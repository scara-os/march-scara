#![allow(irrefutable_let_patterns)]

mod drm;
mod glass_renderer;
mod grabs;
mod handlers;
mod input;
mod lua_bridge;
mod physics;
mod state;
mod winit;

use std::io::{Read, Write};
use std::os::unix::net::UnixListener;
use std::sync::Arc;

use smithay::backend::session::{libseat::LibSeatSession, Session};
use smithay::reexports::{
    calloop::{channel, EventLoop},
    wayland_server::{Display, DisplayHandle},
};
pub use state::Smallvil;
use tracing::{info, warn};

pub struct CalloopData {
    state: Smallvil,
    display_handle: DisplayHandle,
    drm_backend: Option<drm::DrmBackend>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if let Ok(env_filter) = tracing_subscriber::EnvFilter::try_from_default_env() {
        tracing_subscriber::fmt().with_env_filter(env_filter).init();
    } else {
        tracing_subscriber::fmt().init();
    }

    info!("Starting MARCH SCARA Compositor (scara-wm)...");

    let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".into());
    let config_path = format!("{}/.config/march-scara/init.lua", home);
    let config = lua_bridge::load_user_config(&config_path)?;
    info!("Loaded Lua config: {:?}", config.lock().unwrap());

    let backend = std::env::var("SCARA_BACKEND").unwrap_or_else(|_| "winit".into());
    let (seat_name, session) = match backend.as_str() {
        "winit" => ("winit".to_string(), None),
        "drm" => {
            let (session, notifier) = LibSeatSession::new()?;
            let seat_name = session.seat();
            (seat_name, Some((session, notifier)))
        }
        other => {
            return Err(format!("Unsupported SCARA_BACKEND value: {other}").into());
        }
    };

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;
    let (ipc_tx, ipc_rx) = channel::channel::<String>();

    // Terima aksi jendela dari scara-ctl / scara-shell secara real-time di main loop
    event_loop
        .handle()
        .insert_source(ipc_rx, |event, _, data| {
            if let channel::Event::Msg(action) = event {
                data.state.execute_action(&action);
            }
        })?;

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let sock_path = format!("{}/scara.sock", runtime_dir);
    let _ = std::fs::remove_file(&sock_path);
    let ipc_listener = UnixListener::bind(&sock_path)?;
    info!("IPC socket listening at {}", sock_path);

    let cfg_for_ipc = Arc::clone(&config);
    std::thread::spawn(move || {
        for stream in ipc_listener.incoming() {
            if let Ok(mut stream) = stream {
                let mut cmd = String::new();
                stream.read_to_string(&mut cmd).ok();
                let trimmed = cmd.trim();
                let reply = match trimmed {
                    "ping" => "PONG | MARCH SCARA Wayland Compositor Active".to_string(),
                    "reload" => { let _ = ipc_tx.send("config:reload".to_string()); "OK: Reloaded init.lua".to_string() }
                    "config" => format!("{:?}", cfg_for_ipc.lock().unwrap()),
                    action if action.starts_with("window:")
                        || action.starts_with("exec:")
                        || action.starts_with("layout:")
                        || action.starts_with("workspace:")
                        || action.starts_with("movetoworkspace:")
                        || action == "config:reload"
                        || action == "session:quit" =>
                    {
                        let _ = ipc_tx.send(action.to_string());
                        format!("OK: {}", action)
                    }
                    other => format!("Perintah IPC diterima: {}", other),
                };
                stream.write_all(reply.as_bytes()).ok();
            }
        }
    });

    let display: Display<Smallvil> = Display::new()?;
    let display_handle = display.handle();
    let state = Smallvil::new(
        &mut event_loop,
        display,
        Arc::clone(&config),
        config_path,
        &seat_name,
    );

    let mut data = CalloopData {
        state,
        display_handle,
        drm_backend: None,
    };

    match session {
        Some((session, notifier)) => {
            drm::DrmBackend::initialize(&mut event_loop, &mut data, session, notifier)?;
        }
        None => {
            crate::winit::init_winit(&mut event_loop, &mut data)?;
        }
    }

    std::env::set_var("WAYLAND_DISPLAY", &data.state.socket_name);
    update_dbus_activation_environment();

    // Jalankan komponen DE (scara-daemon, scara-shell dengan GSK_RENDERER=gl, dan kitty)
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut children = Vec::new();
    if let Some(dir) = exe_dir {
        if let Ok(daemon) = std::process::Command::new(dir.join("scara-daemon")).spawn() {
            children.push(daemon);
        }
        if let Ok(shell) = std::process::Command::new(dir.join("scara-shell"))
            .env("GSK_RENDERER", "gl")
            .spawn()
        {
            children.push(shell);
        }
    }

    let mut args = std::env::args().skip(1);
    let flag = args.next();
    let arg = args.next();

    match (flag.as_deref(), arg) {
        (Some("-c") | Some("--command"), Some(command)) => {
            if let Ok(c) = std::process::Command::new(command).spawn() {
                children.push(c);
            }
        }
        _ => {
            if let Ok(c) = std::process::Command::new("kitty")
                .args([
                    "-o",
                    "initial_window_width=820",
                    "-o",
                    "initial_window_height=480",
                    "-o",
                    "remember_window_size=no",
                ])
                .spawn()
            {
                children.push(c);
            }
        }
    }

    event_loop.run(None, &mut data, move |_| {})?;

    for mut child in children {
        let _ = child.kill();
        let _ = child.wait();
    }
    let _ = std::fs::remove_file(&sock_path);
    std::process::exit(0);
}

fn update_dbus_activation_environment() {
    match std::process::Command::new("dbus-update-activation-environment")
        .args([
            "--systemd",
            "WAYLAND_DISPLAY",
            "XDG_CURRENT_DESKTOP",
            "XDG_SESSION_DESKTOP",
            "XDG_SESSION_TYPE",
        ])
        .status()
    {
        Ok(status) if status.success() => {}
        Ok(status) => warn!("Failed to update D-Bus activation environment: {status}"),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            warn!("dbus-update-activation-environment is unavailable");
        }
        Err(error) => warn!("Failed to update D-Bus activation environment: {error}"),
    }
}
