#![allow(irrefutable_let_patterns)]

mod display;
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
use std::sync::{mpsc, Arc};

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
    winit_display_tx: Option<mpsc::Sender<display::DisplayIpcRequest>>,
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
    let (display_tx, display_rx) = channel::channel::<display::DisplayIpcRequest>();

    // Terima aksi jendela dari scara-ctl / scara-shell secara real-time di main loop
    event_loop
        .handle()
        .insert_source(ipc_rx, |event, _, data| {
            if let channel::Event::Msg(action) = event {
                data.state.execute_action(&action);
            }
        })?;

    event_loop
        .handle()
        .insert_source(display_rx, |event, _, data| {
            let channel::Event::Msg(request) = event else {
                return;
            };
            if let Some(backend) = data.drm_backend.as_mut() {
                let result = backend.handle_display_request(&request.command, &mut data.state);
                let _ = request.reply.send(result);
            } else if let Some(sender) = &data.winit_display_tx {
                if let Err(error) = sender.send(request) {
                    let _ = error
                        .0
                        .reply
                        .send("ERROR: Winit display handler stopped.".into());
                }
            } else {
                let _ = request
                    .reply
                    .send("ERROR: No display backend is available.".into());
            }
        })?;

    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let sock_path = format!("{}/scara.sock", runtime_dir);
    let _ = std::fs::remove_file(&sock_path);
    let ipc_listener = UnixListener::bind(&sock_path)?;
    info!("IPC socket listening at {}", sock_path);

    let cfg_for_ipc = Arc::clone(&config);
    std::thread::spawn(move || {
        for connection in ipc_listener.incoming() {
            let mut stream = match connection {
                Ok(stream) => stream,
                Err(error) => {
                    warn!(%error, "Failed to accept compositor IPC connection");
                    continue;
                }
            };
            let mut cmd = String::new();
            if let Err(error) = stream.read_to_string(&mut cmd) {
                warn!(%error, "Failed to read compositor IPC request");
                continue;
            }
            let trimmed = cmd.trim();
            let reply = if trimmed.starts_with("display:") {
                let (reply_tx, reply_rx) = mpsc::channel();
                match display_tx.send(display::DisplayIpcRequest {
                    command: trimmed.to_string(),
                    reply: reply_tx,
                }) {
                    Ok(()) => reply_rx
                        .recv_timeout(std::time::Duration::from_secs(5))
                        .unwrap_or_else(|error| {
                            format!("ERROR: Display request did not complete: {error}")
                        }),
                    Err(error) => {
                        format!("ERROR: Display request queue is unavailable: {error}")
                    }
                }
            } else {
                match trimmed {
                    "ping" => "PONG | MARCH SCARA Wayland Compositor Active".to_string(),
                    "reload" => match ipc_tx.send("config:reload".to_string()) {
                        Ok(()) => "OK: Reloaded init.lua".to_string(),
                        Err(error) => {
                            format!("ERROR: Could not queue configuration reload: {error}")
                        }
                    },
                    "config" => format!("{:?}", cfg_for_ipc.lock().unwrap()),
                    action
                        if action.starts_with("window:")
                            || action.starts_with("exec:")
                            || action.starts_with("layout:")
                            || action.starts_with("workspace:")
                            || action.starts_with("movetoworkspace:")
                            || matches!(
                                action,
                                "appearance:reduce-transparency:on"
                                    | "appearance:reduce-transparency:off"
                            )
                            || action == "config:reload"
                            || action == "session:quit" =>
                    {
                        match ipc_tx.send(action.to_string()) {
                            Ok(()) => format!("OK: {}", action),
                            Err(error) => format!("ERROR: Could not queue action: {error}"),
                        }
                    }
                    other => format!("Perintah IPC diterima: {}", other),
                }
            };
            if let Err(error) = stream.write_all(reply.as_bytes()) {
                warn!(%error, "Failed to send compositor IPC response");
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
        winit_display_tx: None,
    };

    match session {
        Some((session, notifier)) => {
            drm::DrmBackend::initialize(&mut event_loop, &mut data, session, notifier)?;
        }
        None => {
            let (sender, receiver) = mpsc::channel();
            data.winit_display_tx = Some(sender);
            crate::winit::init_winit(&mut event_loop, &mut data, receiver)?;
        }
    }

    std::env::set_var("WAYLAND_DISPLAY", &data.state.socket_name);
    update_dbus_activation_environment();

    // Start the shell's required session services and applications.
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()));

    let mut children = Vec::new();
    match std::process::Command::new("mako").spawn() {
        Ok(notification_daemon) => children.push(notification_daemon),
        Err(error) => warn!(%error, "Failed to start the Mako notification daemon"),
    }
    match std::process::Command::new("/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1")
        .spawn()
    {
        Ok(authentication_agent) => children.push(authentication_agent),
        Err(error) => warn!(%error, "Failed to start the Polkit authentication agent"),
    }

    if let Some(dir) = exe_dir {
        if let Ok(daemon) = std::process::Command::new(dir.join("scara-daemon")).spawn() {
            children.push(daemon);
        }
        if let Ok(shell) = std::process::Command::new(dir.join("scara-shell"))
            .env("GSK_RENDERER", "gl")
            .env("GTK_USE_PORTAL", "1")
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
