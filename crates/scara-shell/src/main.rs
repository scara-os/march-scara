use gtk::prelude::*;
use gtk::{
    glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, ContentFit, CssProvider,
    DropDown, Entry, FileChooserAction, FileChooserNative, Grid, IconTheme, Image, Label,
    MenuButton, Orientation, Picture, Popover, ProgressBar, ResponseType, Scale, ScrolledWindow,
    SearchEntry, Separator, Stack, Switch,
};
use gtk_layer_shell::{Edge, Layer, LayerShell};
use serde::Deserialize;
use std::{
    cell::{Cell, RefCell},
    io::{Read, Write},
    os::unix::{
        fs::PermissionsExt,
        net::{UnixListener, UnixStream},
    },
    path::{Path, PathBuf},
    process::Command,
    rc::Rc,
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

mod preferences;

use preferences::{AccentColor, GlassStyle, MenuBarModules, ThemeMode, UserPreferences};

const TAHOE_CSS: &str = r#"
window,
window.background,
window.transparent-layer,
window.transparent-layer.background {
    background-color: transparent;
    background-image: none;
    border: none;
    box-shadow: none;
    font-family: "SF Pro Text", "Inter", sans-serif;
}

window.settings-window,
window.apps-launcher {
    background-color: #202127;
    color: #f5f5f7;
}

.settings-sidebar {
    background-color: rgba(255, 255, 255, 0.07);
    border-right: 1px solid rgba(255, 255, 255, 0.12);
    padding: 16px 12px;
}

.settings-toolbar {
    background-color: rgba(255, 255, 255, 0.035);
    border-bottom: 1px solid rgba(255, 255, 255, 0.10);
    padding: 10px 18px;
}

.settings-profile-card {
    background-color: rgba(255, 255, 255, 0.09);
    border: 1px solid rgba(255, 255, 255, 0.12);
    border-radius: 12px;
    padding: 10px;
    margin: 10px 0 14px;
}

.settings-avatar {
    background-color: #0a84ff;
    border-radius: 99px;
    color: #ffffff;
    font-size: 16px;
    font-weight: 700;
}

.settings-section-title {
    color: rgba(245, 245, 247, 0.56);
    font-size: 10px;
    font-weight: 700;
    padding: 12px 8px 4px;
}

button.settings-nav-button {
    background: transparent;
    border: none;
    border-radius: 8px;
    box-shadow: none;
    color: rgba(245, 245, 247, 0.88);
    min-height: 32px;
    padding: 4px 9px;
    text-align: left;
}

button.settings-nav-button:hover {
    background-color: rgba(255, 255, 255, 0.09);
}

button.settings-nav-button.selected {
    background-color: #0a84ff;
    color: #ffffff;
}

.settings-page-title {
    font-size: 25px;
    font-weight: 700;
}

.settings-help-button {
    border-radius: 99px;
    min-width: 30px;
    min-height: 30px;
}

.settings-integration-note {
    color: rgba(245, 245, 247, 0.64);
    font-size: 11px;
    padding-top: 2px;
}

window.settings-window.light-theme {
    background-color: #f2f3f7;
    color: #1c1c1e;
}

window.settings-window.light-theme .settings-sidebar {
    background-color: rgba(20, 28, 45, 0.045);
    border-right-color: rgba(20, 28, 45, 0.12);
}

window.settings-window.light-theme .settings-toolbar {
    background-color: rgba(255, 255, 255, 0.65);
    border-bottom-color: rgba(20, 28, 45, 0.10);
}

window.settings-window.light-theme .settings-profile-card,
window.settings-window.light-theme .cc-card {
    background-color: rgba(255, 255, 255, 0.76);
    border-color: rgba(20, 28, 45, 0.10);
}

window.settings-window.light-theme .cc-title {
    color: #1c1c1e;
}

window.settings-window.light-theme .cc-sub,
window.settings-window.light-theme .settings-section-title,
window.settings-window.light-theme .settings-integration-note {
    color: rgba(28, 28, 30, 0.66);
}

window.settings-window.light-theme button.settings-nav-button {
    color: rgba(28, 28, 30, 0.88);
}

window.settings-window.light-theme button.settings-nav-button.selected {
    color: #ffffff;
}

window.settings-window.glass-clear .cc-card,
window.settings-window.glass-clear .settings-profile-card {
    background-color: rgba(255, 255, 255, 0.035);
}

window.settings-window.glass-tinted .cc-card,
window.settings-window.glass-tinted .settings-profile-card {
    background-color: rgba(91, 126, 201, 0.19);
}

window.settings-window.light-theme.glass-clear .cc-card,
window.settings-window.light-theme.glass-clear .settings-profile-card {
    background-color: rgba(255, 255, 255, 0.40);
}

window.settings-window.light-theme.glass-tinted .cc-card,
window.settings-window.light-theme.glass-tinted .settings-profile-card {
    background-color: rgba(110, 142, 205, 0.16);
}

window.settings-window.reduce-transparency .cc-card,
window.settings-window.reduce-transparency .settings-profile-card,
window.settings-window.reduce-transparency .settings-toolbar,
window.settings-window.reduce-transparency .settings-sidebar {
    background-color: #30313a;
}

window.settings-window.light-theme.reduce-transparency .cc-card,
window.settings-window.light-theme.reduce-transparency .settings-profile-card,
window.settings-window.light-theme.reduce-transparency .settings-toolbar,
window.settings-window.light-theme.reduce-transparency .settings-sidebar {
    background-color: #e5e7ee;
}

window.settings-window.accent-purple button.settings-nav-button.selected { background-color: #8e63e6; }
window.settings-window.accent-pink button.settings-nav-button.selected { background-color: #e34f9b; }
window.settings-window.accent-red button.settings-nav-button.selected { background-color: #e5484d; }
window.settings-window.accent-orange button.settings-nav-button.selected { background-color: #e87928; }
window.settings-window.accent-yellow button.settings-nav-button.selected { background-color: #bd9200; }
window.settings-window.accent-green button.settings-nav-button.selected { background-color: #30a46c; }
window.settings-window.accent-graphite button.settings-nav-button.selected { background-color: #64666d; }

.tahoe-wallpaper-fallback {
    background-image:
        radial-gradient(ellipse at 50% 15%, rgba(88, 66, 235, 0.92), transparent 62%),
        radial-gradient(ellipse at 25% 58%, rgba(28, 95, 245, 0.85), transparent 58%),
        radial-gradient(ellipse at 78% 68%, rgba(16, 130, 255, 0.78), transparent 56%),
        radial-gradient(ellipse at 50% 95%, rgba(65, 165, 255, 0.55), transparent 60%),
        linear-gradient(170deg, #271a8a 0%, #1636b8 48%, #1f64e0 100%);
}

.scara-menubar {
    background-color: rgba(18, 22, 48, 0.24);
    border-bottom: 1px solid rgba(255, 255, 255, 0.12);
    padding: 2px 12px;
    color: #ffffff;
    font-size: 13px;
}

button.menubar-item,
menubutton.menubar-item > button {
    background-color: transparent;
    background-image: none;
    border: none;
    box-shadow: none;
    outline: none;
    color: rgba(255, 255, 255, 0.95);
    font-weight: 500;
    font-size: 13px;
    padding: 2px 8px;
    min-height: 18px;
    min-width: 0px;
    border-radius: 5px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.22);
}

menubutton.menubar-brand-btn > button {
    font-weight: 700;
}

button.menubar-item:focus,
button.menubar-item:focus-visible,
menubutton.menubar-item > button:focus,
menubutton.menubar-item > button:focus-visible {
    background-color: transparent;
    outline: none;
    box-shadow: none;
}

button.menubar-item:hover,
menubutton.menubar-item > button:hover,
menubutton.menubar-item > button:checked {
    background-color: rgba(255, 255, 255, 0.20);
    color: #ffffff;
}

.menubar-clock {
    font-weight: 500;
    font-size: 13px;
    color: #ffffff;
    padding: 1px 6px;
    text-shadow: 0 1px 2px rgba(0, 0, 0, 0.22);
}

.menubar-bat {
    font-size: 11px;
    font-weight: 600;
    color: rgba(255, 255, 255, 0.95);
}

.cc-toggle-track {
    min-width: 12px;
    min-height: 3px;
    border-radius: 99px;
    border: 1px solid rgba(255, 255, 255, 0.92);
    background-color: rgba(255, 255, 255, 0.25);
}

popover.tahoe-popover > contents {
    background-color: rgba(26, 30, 52, 0.90);
    background-image: none;
    border: 1px solid rgba(255, 255, 255, 0.26);
    border-radius: 14px;
    padding: 8px;
    color: #ffffff;
    box-shadow: 0 18px 40px rgba(0, 0, 0, 0.50);
}

button.menu-action-btn {
    background-color: transparent;
    background-image: none;
    border: none;
    box-shadow: none;
    color: #f5f5f7;
    font-size: 13px;
    padding: 5px 10px;
    border-radius: 7px;
}

button.menu-action-btn:hover {
    background-color: #0a84ff;
    color: #ffffff;
}

.cc-card {
    background-color: rgba(255, 255, 255, 0.10);
    border: 1px solid rgba(255, 255, 255, 0.16);
    border-radius: 12px;
    padding: 10px 12px;
}
.cc-title {
    font-weight: 700;
    font-size: 13px;
    color: #ffffff;
}
.cc-sub {
    font-size: 11px;
    color: rgba(255, 255, 255, 0.72);
}

.dock-shadow-pad {
    background: transparent;
    padding: 10px 18px 6px 18px;
}

.scara-dock {
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.24) 0%, rgba(255, 255, 255, 0.11) 100%);
    border: 1px solid rgba(255, 255, 255, 0.38);
    border-radius: 21px;
    padding: 6px 12px 4px 12px;
    box-shadow:
        0 10px 24px rgba(0, 0, 0, 0.36),
        inset 0 1px 0 rgba(255, 255, 255, 0.48);
}

button.dock-icon-btn {
    background-color: transparent;
    background-image: none;
    border: none;
    box-shadow: none;
    outline: none;
    padding: 2px 3px;
    margin: 0;
    border-radius: 12px;
    transition: transform 180ms cubic-bezier(0.22, 1.0, 0.36, 1.0);
}

button.dock-icon-btn:hover {
    background-color: transparent;
    background-image: none;
    transform: scale(1.24) translateY(-6px);
}

.dock-dot-active {
    background-color: rgba(255, 255, 255, 0.94);
    border-radius: 99px;
    min-width: 4px;
    min-height: 4px;
}

.dock-dot-idle {
    background-color: transparent;
    min-width: 4px;
    min-height: 4px;
}

.dock-separator {
    background-color: rgba(255, 255, 255, 0.30);
    min-width: 1px;
    margin: 6px 5px 10px 5px;
}
"#;

#[derive(Clone)]
struct DesktopApp {
    name: String,
    exec: String,
    icon: String,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
struct DisplayMode {
    width: u16,
    height: u16,
    refresh_millihz: u32,
}

#[derive(Debug, Deserialize)]
struct DisplayOutputInfo {
    name: String,
    backend: String,
    current: DisplayMode,
    modes: Vec<DisplayMode>,
    scale: i32,
    x: i32,
    y: i32,
}

fn launch_app(cmd: &str) {
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
}

fn launch_program(program: &str, args: &[&str]) -> Result<(), std::io::Error> {
    Command::new(program).args(args).spawn().map(|_| ())
}

fn send_wm_ipc(action: &str) {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            let _ = Command::new(dir.join("scara-ctl")).arg(action).spawn();
            return;
        }
    }
    let _ = Command::new("scara-ctl").arg(action).spawn();
}

fn run_wm_ctl_output(action: &str) -> Result<String, String> {
    let sibling_ctl = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|directory| directory.join("scara-ctl")))
        .filter(|path| path.is_file());
    let program = sibling_ctl.unwrap_or_else(|| PathBuf::from("scara-ctl"));
    let output = Command::new(program)
        .arg(action)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("scara-ctl exited with {}", output.status)
        } else {
            detail
        });
    }

    let response = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if response.starts_with("ERROR:") {
        return Err(response);
    }
    Ok(response)
}

fn set_compositor_reduce_transparency(enabled: bool) -> Result<(), String> {
    let action = if enabled {
        "appearance:reduce-transparency:on"
    } else {
        "appearance:reduce-transparency:off"
    };
    run_wm_ctl_output(action).map(|_| ())
}

fn show_launcher(window: &ApplicationWindow, search: &SearchEntry) {
    window.present();
    search.set_text("");
    search.grab_focus();
    send_wm_ipc("window:focus-launcher");
}

fn start_shell_ipc() -> Receiver<String> {
    let (sender, receiver) = mpsc::channel();
    let runtime_dir = std::env::var("XDG_RUNTIME_DIR").unwrap_or_else(|_| "/tmp".into());
    let socket_path = PathBuf::from(runtime_dir).join("scara-shell.sock");

    if let Ok(mut stream) = UnixStream::connect(&socket_path) {
        let _ = stream.write_all(b"launcher");
        return receiver;
    }

    if let Err(error) = std::fs::remove_file(&socket_path) {
        if error.kind() != std::io::ErrorKind::NotFound {
            eprintln!("Could not remove stale shell socket: {error}");
            return receiver;
        }
    }

    let listener = match UnixListener::bind(&socket_path) {
        Ok(listener) => listener,
        Err(error) => {
            eprintln!("Could not create shell IPC socket: {error}");
            return receiver;
        }
    };

    thread::spawn(move || {
        for connection in listener.incoming() {
            let Ok(mut stream) = connection else {
                eprintln!("Error accepting shell IPC connection");
                continue;
            };
            let mut command = String::new();
            match stream.read_to_string(&mut command) {
                Ok(_) => {
                    if sender.send(command).is_err() {
                        break;
                    }
                }
                Err(error) => eprintln!("Error reading shell IPC command: {error}"),
            }
        }
        let _ = std::fs::remove_file(socket_path);
    });

    receiver
}

fn run_program_output(program: &str, args: &[&str]) -> Result<String, String> {
    let args = args
        .iter()
        .map(|arg| (*arg).to_string())
        .collect::<Vec<_>>();
    run_system_command_output(program, &args)
}

fn run_system_command_output(program: &str, args: &[String]) -> Result<String, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
        return Err(if detail.is_empty() {
            format!("command exited with {}", output.status)
        } else {
            detail
        });
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn find_program_on_path(program: &str) -> Option<PathBuf> {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|path| std::env::split_paths(&path).collect::<Vec<_>>())
        .map(|directory| directory.join(program))
        .find(|path| {
            std::fs::metadata(path).is_ok_and(|metadata| {
                metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
            })
        })
}

fn user_config_file(
    app_directory: &str,
    file_name: &str,
    initial_contents: &str,
) -> Result<PathBuf, String> {
    let config_home = if let Some(path) = std::env::var_os("XDG_CONFIG_HOME") {
        PathBuf::from(path)
    } else {
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| "HOME is not set.".to_string())?)
            .join(".config")
    };
    let directory = config_home.join(app_directory);
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("Could not create {}: {error}", directory.display()))?;
    let path = directory.join(file_name);
    if !path.exists() {
        std::fs::write(&path, initial_contents)
            .map_err(|error| format!("Could not create {}: {error}", path.display()))?;
    }
    Ok(path)
}

fn open_user_config(
    app_directory: &str,
    file_name: &str,
    initial_contents: &str,
) -> Result<PathBuf, String> {
    let path = user_config_file(app_directory, file_name, initial_contents)?;
    Command::new("gnome-text-editor")
        .arg(&path)
        .spawn()
        .map_err(|error| {
            format!(
                "Could not open {} in GNOME Text Editor: {error}",
                path.display()
            )
        })?;
    Ok(path)
}

fn notification_mode_active(output: &str, mode: &str) -> bool {
    output
        .split_whitespace()
        .any(|active_mode| active_mode == mode)
}

fn mako_focus_status() -> Result<bool, String> {
    mako_focus_status_with(run_program_output)
}

fn mako_focus_status_with(
    run: impl FnOnce(&str, &[&str]) -> Result<String, String>,
) -> Result<bool, String> {
    let modes = run("makoctl", &["mode"])?;
    Ok(notification_mode_active(&modes, "do-not-disturb"))
}

fn printer_status() -> Result<String, String> {
    printer_status_with(run_program_output)
}

fn printer_status_with(
    mut run: impl FnMut(&str, &[&str]) -> Result<String, String>,
) -> Result<String, String> {
    let printers = run("lpstat", &["-p"])?;
    let default_printer = run("lpstat", &["-d"])?;
    let queued_jobs = run("lpstat", &["-o"])?;
    let printers = if printers.trim().is_empty() {
        "No printers are configured.".to_string()
    } else {
        printers
    };
    let default_printer = if default_printer.trim().is_empty() {
        "Default printer: not set.".to_string()
    } else {
        default_printer
    };
    let queued_jobs = if queued_jobs.trim().is_empty() {
        "Print queue: empty.".to_string()
    } else {
        format!("Print queue:\n{queued_jobs}")
    };
    Ok(format!("{printers}\n{default_printer}\n{queued_jobs}"))
}

fn refresh_printer_status(status: &Label) {
    status.set_text("Querying the host print service…");
    let status = status.clone();
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(printer_status());
    });
    glib::timeout_add_local(Duration::from_millis(100), move || {
        match receiver.try_recv() {
            Ok(Ok(summary)) => {
                status.set_text(&summary);
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                status.set_text(&format!("Print service unavailable: {error}"));
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                status.set_text("Print service query ended without a result.");
                glib::ControlFlow::Break
            }
        }
    });
}

fn home_storage_categories() -> Result<String, String> {
    let home =
        PathBuf::from(std::env::var_os("HOME").ok_or_else(|| "HOME is not set.".to_string())?);
    let mut categories = Vec::new();
    for (name, directory) in [
        ("Documents", "Documents"),
        ("Downloads", "Downloads"),
        ("Pictures", "Pictures"),
        ("Music", "Music"),
        ("Videos", "Videos"),
    ] {
        let path = home.join(directory);
        if !path.is_dir() {
            continue;
        }
        let output = Command::new("du")
            .args(["-sh", "--"])
            .arg(&path)
            .output()
            .map_err(|error| format!("Could not measure {}: {error}", path.display()))?;
        if !output.status.success() {
            return Err(format!(
                "Could not measure {}: {}",
                path.display(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let size = String::from_utf8_lossy(&output.stdout)
            .split_whitespace()
            .next()
            .ok_or_else(|| format!("No disk usage was returned for {}.", path.display()))?
            .to_string();
        categories.push(format!("{name}: {size}"));
    }
    if categories.is_empty() {
        Ok("No standard user folders were found.".to_string())
    } else {
        Ok(categories.join("\n"))
    }
}

fn input_device_summary(kind: &str) -> Result<String, String> {
    let output = run_program_output("libinput", &["list-devices"])?;
    Ok(parse_libinput_device_summary(&output, kind))
}

fn parse_libinput_device_summary(output: &str, kind: &str) -> String {
    let mut devices = Vec::new();
    let mut block = Vec::new();
    for line in output.lines() {
        if let Some(name) = line.trim().strip_prefix("Device:") {
            if !block.is_empty() {
                append_input_device_summary(&mut devices, &block, kind);
                block.clear();
            }
            block.push(format!("Device: {}", name.trim()));
        } else if !block.is_empty() {
            block.push(line.trim().to_string());
        }
    }
    if !block.is_empty() {
        append_input_device_summary(&mut devices, &block, kind);
    }
    if devices.is_empty() {
        format!("No {kind} devices were reported by libinput.")
    } else {
        devices.join("\n\n")
    }
}

fn append_input_device_summary(devices: &mut Vec<String>, block: &[String], kind: &str) {
    let name = block
        .first()
        .and_then(|line| line.strip_prefix("Device: "))
        .unwrap_or("Unknown input device");
    let name_lower = name.to_ascii_lowercase();
    let is_touchpad = name_lower.contains("touchpad") || name_lower.contains("trackpad");
    if (kind == "touchpad" && !is_touchpad)
        || (kind == "pointer"
            && (is_touchpad
                || !block
                    .iter()
                    .any(|line| line.starts_with("Capabilities:") && line.contains("pointer"))))
    {
        return;
    }
    let mut details = vec![format!("Device: {name}")];
    details.extend(block.iter().filter_map(|line| {
        [
            "Kernel:",
            "Capabilities:",
            "Tap-to-click:",
            "Tap-and-drag:",
            "Natural scrolling:",
            "Scroll methods:",
            "Accel profiles:",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        .then(|| line.to_string())
    }));
    devices.push(details.join("\n"));
}

fn append_home_storage_categories(page: &GtkBox) {
    let card = settings_card(
        "User storage categories",
        "Approximate usage in standard home folders. Folder sizes are computed in the background.",
    );
    let status = Label::new(Some(
        "Scanning Documents, Downloads, Pictures, Music, and Videos…",
    ));
    status.add_css_class("cc-sub");
    status.set_halign(Align::Start);
    status.set_wrap(true);
    card.append(&status);
    page.append(&card);

    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(home_storage_categories());
    });
    glib::timeout_add_local(Duration::from_millis(100), move || {
        match receiver.try_recv() {
            Ok(Ok(summary)) => {
                status.set_text(&summary);
                glib::ControlFlow::Break
            }
            Ok(Err(error)) => {
                status.set_text(&format!("Storage category scan failed: {error}"));
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                status.set_text("Storage category scan ended without a result.");
                glib::ControlFlow::Break
            }
        }
    });
}

fn configured_portal_backend<'a>(configuration: &'a str, interface: &str) -> Option<&'a str> {
    let mut default_backend = None;
    for line in configuration.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') || line.starts_with('[') {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() == interface {
            return Some(value.trim());
        }
        if key.trim() == "default" {
            default_backend = Some(value.trim());
        }
    }
    default_backend
}

fn portal_backend_implements(backend: &str, interface: &str) -> Result<(), String> {
    let descriptor =
        Path::new("/usr/share/xdg-desktop-portal/portals").join(format!("{backend}.portal"));
    let contents = std::fs::read_to_string(&descriptor)
        .map_err(|error| format!("Could not read {}: {error}", descriptor.display()))?;
    let interfaces = contents
        .lines()
        .find_map(|line| line.trim().strip_prefix("Interfaces="))
        .unwrap_or_default();
    if interfaces.split(';').any(|entry| entry.trim() == interface) {
        Ok(())
    } else {
        Err(format!(
            "The {backend} portal backend does not advertise {interface}."
        ))
    }
}

fn dbus_interface_available(
    bus: &str,
    service: &str,
    object_path: &str,
    interface: &str,
) -> Result<(), String> {
    dbus_interface_available_with(bus, service, object_path, interface, run_program_output)
}

fn dbus_interface_available_with(
    bus: &str,
    service: &str,
    object_path: &str,
    interface: &str,
    run: impl FnOnce(&str, &[&str]) -> Result<String, String>,
) -> Result<(), String> {
    run(
        "busctl",
        &[bus, "introspect", service, object_path, interface],
    )
    .map(|_| ())
}

fn secret_service_status(check: impl FnOnce() -> Result<(), String>) -> String {
    check()
        .map(|()| {
            "Secret Service is available. Credentials remain managed by the host keyring."
                .to_string()
        })
        .unwrap_or_else(|error| {
            format!("Secret Service unavailable; check the host keyring: {error}")
        })
}

fn run_system_command_async<F>(
    program: &'static str,
    args: Vec<String>,
    status: Label,
    on_complete: F,
) where
    F: FnOnce(bool) + 'static,
{
    status.set_text("Applying system change...");
    let (sender, receiver) = mpsc::channel();
    thread::spawn(move || {
        let _ = sender.send(run_system_command_output(program, &args));
    });

    let mut on_complete = Some(on_complete);
    glib::timeout_add_local(Duration::from_millis(100), move || {
        match receiver.try_recv() {
            Ok(result) => {
                let succeeded = result.is_ok();
                match result {
                    Ok(_) => status.set_text("System change completed."),
                    Err(error) => status.set_text(&format!("System change failed: {error}")),
                }
                if let Some(on_complete) = on_complete.take() {
                    on_complete(succeeded);
                }
                glib::ControlFlow::Break
            }
            Err(mpsc::TryRecvError::Empty) => glib::ControlFlow::Continue,
            Err(mpsc::TryRecvError::Disconnected) => {
                status.set_text("System operation ended without a result.");
                if let Some(on_complete) = on_complete.take() {
                    on_complete(false);
                }
                glib::ControlFlow::Break
            }
        }
    });
}

fn split_nmcli_terse_row(row: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut field = String::new();
    let mut escaped = false;

    for character in row.chars() {
        if escaped {
            field.push(character);
            escaped = false;
        } else {
            match character {
                '\\' => escaped = true,
                ':' => fields.push(std::mem::take(&mut field)),
                _ => field.push(character),
            }
        }
    }
    if escaped {
        field.push('\\');
    }
    fields.push(field);
    fields
}

#[derive(Debug, PartialEq)]
struct WifiNetwork {
    ssid: String,
    signal: u8,
    security: String,
    in_use: bool,
}

#[derive(Debug, PartialEq)]
struct WifiProfile {
    name: String,
    uuid: String,
    active: bool,
}

#[derive(Debug, PartialEq)]
struct BluetoothDevice {
    address: String,
    name: String,
    paired: bool,
    connected: bool,
}

#[derive(Debug, PartialEq)]
struct AudioDevice {
    name: String,
}

#[derive(Debug, PartialEq)]
struct BatteryHistorySample {
    timestamp: u64,
    percentage: f64,
    state: u32,
}

fn parse_battery_history(output: &str) -> Result<Vec<BatteryHistorySample>, String> {
    let mut fields = output.split_whitespace();
    if fields.next() != Some("a(udu)") {
        return Err("UPower returned an unexpected history format.".to_string());
    }
    let count = fields
        .next()
        .ok_or_else(|| "UPower returned an incomplete history response.".to_string())?
        .parse::<usize>()
        .map_err(|_| "UPower returned an invalid history count.".to_string())?;
    let mut samples = Vec::with_capacity(count);
    for _ in 0..count {
        let timestamp = fields
            .next()
            .ok_or_else(|| "UPower returned a truncated history response.".to_string())?
            .parse::<u64>()
            .map_err(|_| "UPower returned an invalid history timestamp.".to_string())?;
        let percentage = fields
            .next()
            .ok_or_else(|| "UPower returned a truncated history response.".to_string())?
            .parse::<f64>()
            .map_err(|_| "UPower returned an invalid history percentage.".to_string())?;
        let state = fields
            .next()
            .ok_or_else(|| "UPower returned a truncated history response.".to_string())?
            .parse::<u32>()
            .map_err(|_| "UPower returned an invalid history state.".to_string())?;
        if timestamp > 0 && (0.0..=100.0).contains(&percentage) {
            samples.push(BatteryHistorySample {
                timestamp,
                percentage,
                state,
            });
        }
    }
    if fields.next().is_some() {
        return Err("UPower returned more history data than declared.".to_string());
    }
    Ok(samples)
}

fn parse_power_profiles(output: &str) -> Vec<String> {
    output
        .lines()
        .filter_map(|line| {
            let line = line.trim().strip_prefix("* ").unwrap_or(line.trim());
            let profile = line.strip_suffix(':')?;
            (!profile.is_empty()
                && profile
                    .chars()
                    .all(|character| character.is_ascii_lowercase() || character == '-'))
            .then(|| profile.to_string())
        })
        .collect()
}

fn parse_audio_devices(output: &str, include_monitors: bool) -> Vec<AudioDevice> {
    output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split('\t');
            fields.next()?;
            let name = fields.next()?.trim();
            if name.is_empty() || (!include_monitors && name.ends_with(".monitor")) {
                return None;
            }
            Some(AudioDevice {
                name: name.to_string(),
            })
        })
        .collect()
}

fn parse_audio_channel_volumes(output: &str) -> Vec<f64> {
    output
        .split_whitespace()
        .filter(|token| token.contains('%'))
        .filter_map(|token| {
            let value = token
                .trim_matches(|character: char| !character.is_ascii_digit() && character != '.')
                .parse::<f64>()
                .ok()?;
            (0.0..=150.0).contains(&value).then_some(value)
        })
        .collect()
}

fn audio_balance_from_volumes(volumes: &[f64]) -> Option<f64> {
    let [left, right, ..] = volumes else {
        return None;
    };
    let maximum = left.max(*right);
    if maximum <= 0.0 {
        return Some(0.0);
    }
    Some(((right - left) / maximum * 100.0).clamp(-100.0, 100.0))
}

fn parse_bluetooth_powered(output: &str) -> Option<bool> {
    output.lines().find_map(|line| {
        let value = line.trim().strip_prefix("Powered: ")?;
        match value {
            "yes" => Some(true),
            "no" => Some(false),
            _ => None,
        }
    })
}

fn parse_bluetooth_device_rows(output: &str) -> Vec<(String, String)> {
    output
        .lines()
        .filter_map(|line| {
            let line = line
                .trim()
                .strip_prefix("[NEW] ")
                .or_else(|| line.trim().strip_prefix("[CHG] "))
                .unwrap_or_else(|| line.trim());
            let mut fields = line.split_whitespace();
            if fields.next()? != "Device" {
                return None;
            }
            let address = fields.next()?.to_string();
            if !valid_bluetooth_address(&address) {
                return None;
            }
            let name = fields.collect::<Vec<_>>().join(" ");
            Some((address, name))
        })
        .collect()
}

fn valid_bluetooth_address(address: &str) -> bool {
    address.len() == 17
        && address
            .bytes()
            .enumerate()
            .all(|(index, byte)| match index % 3 {
                2 => byte == b':',
                _ => byte.is_ascii_hexdigit(),
            })
}

fn parse_bluetooth_devices(paired: &str, found: &str, connected: &str) -> Vec<BluetoothDevice> {
    let paired_rows = parse_bluetooth_device_rows(paired);
    let found_rows = parse_bluetooth_device_rows(found);
    let connected_addresses = parse_bluetooth_device_rows(connected)
        .into_iter()
        .map(|(address, _)| address)
        .collect::<std::collections::HashSet<_>>();
    let paired_addresses = paired_rows
        .iter()
        .map(|(address, _)| address.clone())
        .collect::<std::collections::HashSet<_>>();

    let mut devices: Vec<BluetoothDevice> = Vec::new();
    for (address, name) in paired_rows.into_iter().chain(found_rows) {
        if let Some(existing) = devices.iter_mut().find(|device| device.address == address) {
            if existing.name.is_empty() && !name.is_empty() {
                existing.name = name;
            }
            continue;
        }
        devices.push(BluetoothDevice {
            paired: paired_addresses.contains(&address),
            connected: connected_addresses.contains(&address),
            address,
            name,
        });
    }
    devices
}

fn refresh_bluetooth_devices(list: &GtkBox, status: &Label) {
    let paired = match run_program_output("bluetoothctl", &["devices", "Paired"]) {
        Ok(output) => output,
        Err(error) => {
            status.set_text(&format!("Could not list paired devices: {error}"));
            return;
        }
    };
    let found = match run_program_output("bluetoothctl", &["devices"]) {
        Ok(output) => output,
        Err(error) => {
            status.set_text(&format!("Could not list nearby devices: {error}"));
            return;
        }
    };
    let connected = match run_program_output("bluetoothctl", &["devices", "Connected"]) {
        Ok(output) => output,
        Err(error) => {
            status.set_text(&format!("Could not read connected devices: {error}"));
            return;
        }
    };

    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let devices = parse_bluetooth_devices(&paired, &found, &connected);
    if devices.is_empty() {
        status.set_text("No Bluetooth devices found. Start a scan to discover nearby devices.");
        return;
    }
    status.set_text(&format!("{} Bluetooth device(s) found.", devices.len()));

    for device in devices {
        let row = GtkBox::new(Orientation::Vertical, 4);
        let title = if device.name.is_empty() {
            device.address.clone()
        } else {
            device.name.clone()
        };
        let name = Label::new(Some(&title));
        name.set_halign(Align::Start);
        name.add_css_class("cc-title");
        let state = match (device.paired, device.connected) {
            (true, true) => "Paired · Connected",
            (true, false) => "Paired · Not connected",
            (false, _) => "Not paired",
        };
        let detail = Label::new(Some(&format!("{} · {}", device.address, state)));
        detail.set_halign(Align::Start);
        detail.add_css_class("cc-sub");
        row.append(&name);
        row.append(&detail);

        let actions = GtkBox::new(Orientation::Horizontal, 6);
        let status_ref = status.clone();
        let list_ref = list.clone();
        if device.paired {
            let action = if device.connected {
                "Disconnect"
            } else {
                "Connect"
            };
            let action_button = Button::with_label(action);
            action_button.add_css_class("flat");
            action_button.add_css_class("menu-action-btn");
            let command = if device.connected {
                "disconnect"
            } else {
                "connect"
            };
            let address = device.address.clone();
            let action_status = status_ref.clone();
            let action_list = list_ref.clone();
            action_button.connect_clicked(move |_| {
                let status_after = action_status.clone();
                let list_after = action_list.clone();
                run_system_command_async(
                    "bluetoothctl",
                    vec![command.to_string(), address.clone()],
                    action_status.clone(),
                    move |success| {
                        if success {
                            refresh_bluetooth_devices(&list_after, &status_after);
                        }
                    },
                );
            });
            actions.append(&action_button);

            let forget_button = Button::with_label("Forget");
            forget_button.add_css_class("flat");
            forget_button.add_css_class("menu-action-btn");
            let address = device.address;
            forget_button.connect_clicked(move |_| {
                let status_after = status_ref.clone();
                let list_after = list_ref.clone();
                run_system_command_async(
                    "bluetoothctl",
                    vec!["remove".to_string(), address.clone()],
                    status_ref.clone(),
                    move |success| {
                        if success {
                            refresh_bluetooth_devices(&list_after, &status_after);
                        }
                    },
                );
            });
            actions.append(&forget_button);
        } else {
            let pair_button = Button::with_label("Pair in Blueman");
            pair_button.add_css_class("flat");
            pair_button.add_css_class("menu-action-btn");
            pair_button.connect_clicked(move |_| match launch_program("blueman-manager", &[]) {
                Ok(()) => status_ref
                    .set_text("Opened Blueman to confirm pairing and device authorization."),
                Err(error) => {
                    status_ref.set_text(&format!("Bluetooth manager unavailable: {error}"))
                }
            });
            actions.append(&pair_button);
        }
        row.append(&actions);
        list.append(&row);
    }
}

fn parse_wifi_profiles(output: &str) -> Vec<WifiProfile> {
    output
        .lines()
        .filter_map(|row| {
            let fields = split_nmcli_terse_row(row);
            if fields.len() != 4
                || !matches!(fields[2].as_str(), "802-11-wireless" | "wifi")
                || fields[0].is_empty()
                || fields[1].is_empty()
            {
                return None;
            }
            Some(WifiProfile {
                name: fields[0].clone(),
                uuid: fields[1].clone(),
                active: !fields[3].is_empty() && fields[3] != "--",
            })
        })
        .collect()
}

fn parse_wifi_networks(output: &str) -> Vec<WifiNetwork> {
    let mut networks: Vec<WifiNetwork> = Vec::new();
    for row in output.lines() {
        let fields = split_nmcli_terse_row(row);
        if fields.len() != 4 || fields[1].trim().is_empty() {
            continue;
        }

        let network = WifiNetwork {
            in_use: fields[0].trim() == "*",
            ssid: fields[1].trim().to_string(),
            signal: fields[2].trim().parse::<u8>().unwrap_or(0).min(100),
            security: match fields[3].trim() {
                "" | "--" => "Open".to_string(),
                security => security.to_string(),
            },
        };
        if let Some(existing) = networks
            .iter_mut()
            .find(|existing| existing.ssid == network.ssid)
        {
            if (network.in_use && !existing.in_use)
                || (network.in_use == existing.in_use && network.signal > existing.signal)
            {
                *existing = network;
            }
        } else {
            networks.push(network);
        }
    }
    networks
}

fn load_desktop_apps() -> Vec<DesktopApp> {
    let mut apps = Vec::new();
    let home = std::env::var("HOME").unwrap_or_default();
    let dirs = [
        PathBuf::from("/usr/share/applications"),
        PathBuf::from(format!("{}/.local/share/applications", home)),
    ];

    for dir in dirs {
        if let Ok(entries) = std::fs::read_dir(dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) != Some("desktop") {
                    continue;
                }
                if let Ok(content) = std::fs::read_to_string(&path) {
                    let mut name = None;
                    let mut exec = None;
                    let mut icon = "application-x-executable".to_string();
                    let mut no_display = false;
                    let mut in_main = false;

                    for line in content.lines() {
                        let line = line.trim();
                        if line.starts_with('[') {
                            in_main = line == "[Desktop Entry]";
                            continue;
                        }
                        if !in_main {
                            continue;
                        }
                        if line == "NoDisplay=true" || line == "Hidden=true" {
                            no_display = true;
                        } else if let Some(v) = line.strip_prefix("Name=") {
                            if name.is_none() {
                                name = Some(v.to_string());
                            }
                        } else if let Some(v) = line.strip_prefix("Exec=") {
                            if exec.is_none() {
                                let cleaned = v
                                    .replace("%u", "")
                                    .replace("%U", "")
                                    .replace("%f", "")
                                    .replace("%F", "")
                                    .replace("%i", "")
                                    .replace("%c", "")
                                    .replace("%k", "");
                                exec = Some(cleaned.trim().to_string());
                            }
                        } else if let Some(v) = line.strip_prefix("Icon=") {
                            icon = v.to_string();
                        }
                    }

                    if !no_display {
                        if let (Some(n), Some(e)) = (name, exec) {
                            apps.push(DesktopApp {
                                name: n,
                                exec: e,
                                icon,
                            });
                        }
                    }
                }
            }
        }
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps.dedup_by(|a, b| a.name == b.name);
    apps
}

fn activate_launcher_entry(
    exec: &str,
    settings_window: &ApplicationWindow,
    settings_navigator: &SettingsNavigator,
) {
    if exec == "__scara_settings__" {
        settings_window.present();
    } else if let Some(panel) = exec.strip_prefix("__scara_settings__:") {
        settings_navigator(panel);
    } else {
        launch_app(exec);
    }
}

fn launcher_entry_matches(query: &str, app: &DesktopApp) -> bool {
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return true;
    }

    if app.exec.starts_with("__scara_settings__:") {
        settings_search_matches(&query, &format!("{} {}", app.name, app.exec))
    } else {
        app.name.to_lowercase().contains(&query) || app.exec.to_lowercase().contains(&query)
    }
}

fn find_wallpaper_path() -> Option<PathBuf> {
    if let Ok(preferences) = preferences::load() {
        if let Some(path) = preferences.wallpaper.filter(|path| path.is_file()) {
            return Some(path);
        }
    }

    let home = std::env::var("HOME").ok()?;
    let candidates = [
        format!("{}/.config/march-scara/wallpaper.jpg", home),
        format!("{}/.config/march-scara/wallpaper.png", home),
        format!("{}/.config/march-scara/wallpaper.webp", home),
    ];
    candidates
        .into_iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

fn discover_wallpapers() -> Result<Vec<PathBuf>, String> {
    let mut directories = vec![PathBuf::from("/usr/share/backgrounds")];
    if let Some(home) = std::env::var_os("HOME") {
        directories.push(PathBuf::from(home).join("Pictures/Wallpapers"));
    }

    let mut wallpapers = Vec::new();
    for directory in directories {
        let entries = match std::fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(format!("Could not read {}: {error}", directory.display()));
            }
        };
        for entry in entries {
            let entry = entry.map_err(|error| {
                format!(
                    "Could not read an entry in {}: {error}",
                    directory.display()
                )
            })?;
            let path = entry.path();
            if !path.is_file()
                || !path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| {
                        matches!(
                            extension.to_ascii_lowercase().as_str(),
                            "jpg" | "jpeg" | "png" | "webp"
                        )
                    })
            {
                continue;
            }
            wallpapers.push(path);
        }
    }
    wallpapers.sort();
    wallpapers.dedup();
    Ok(wallpapers)
}

fn update_preferences(
    current: &Rc<RefCell<UserPreferences>>,
    update: impl FnOnce(&mut UserPreferences),
) -> std::io::Result<()> {
    let mut updated = current.borrow().clone();
    update(&mut updated);
    preferences::save(&updated)?;
    *current.borrow_mut() = updated;
    Ok(())
}

type MenuBarWidgetRegistry = Rc<RefCell<std::collections::HashMap<String, gtk::Widget>>>;
type SettingsNavigator = Rc<dyn Fn(&str)>;

fn update_menu_bar_module(preferences: &mut UserPreferences, name: &str, visible: bool) {
    match name {
        "bluetooth" => preferences.menu_bar.bluetooth = visible,
        "battery" => preferences.menu_bar.battery = visible,
        "wifi" => preferences.menu_bar.wifi = visible,
        "spotlight" => preferences.menu_bar.spotlight = visible,
        "control_center" => preferences.menu_bar.control_center = visible,
        "clock" => preferences.menu_bar.clock = visible,
        "sound" => preferences.menu_bar.sound = visible,
        _ => {}
    }
}

fn menu_bar_module_visible(modules: &MenuBarModules, name: &str) -> bool {
    match name {
        "bluetooth" => modules.bluetooth,
        "battery" => modules.battery,
        "wifi" => modules.wifi,
        "spotlight" => modules.spotlight,
        "control_center" => modules.control_center,
        "clock" => modules.clock,
        "sound" => modules.sound,
        _ => false,
    }
}

fn set_menu_bar_module_visibility(widgets: &MenuBarWidgetRegistry, preferences: &UserPreferences) {
    let modules: &MenuBarModules = &preferences.menu_bar;
    for (name, visible) in [
        ("bluetooth", modules.bluetooth),
        ("battery", modules.battery),
        ("wifi", modules.wifi),
        ("spotlight", modules.spotlight),
        ("control_center", modules.control_center),
        ("clock", modules.clock),
        ("sound", modules.sound),
    ] {
        if let Some(widget) = widgets.borrow().get(name) {
            widget.set_visible(visible);
        }
    }
}

fn update_clock_widget(widgets: &MenuBarWidgetRegistry, preferences: &UserPreferences) {
    if let Some(widget) = widgets.borrow().get("clock") {
        if let Ok(label) = widget.clone().downcast::<Label>() {
            label.set_text(&format_tahoe_clock(preferences.clock_24_hour));
        }
    }
}

fn apply_appearance(window: &ApplicationWindow, preferences: &UserPreferences) {
    for class in [
        "light-theme",
        "glass-clear",
        "glass-tinted",
        "reduce-transparency",
        "accent-blue",
        "accent-purple",
        "accent-pink",
        "accent-red",
        "accent-orange",
        "accent-yellow",
        "accent-green",
        "accent-graphite",
    ] {
        window.remove_css_class(class);
    }

    match preferences.theme {
        ThemeMode::Auto => {}
        ThemeMode::Light => window.add_css_class("light-theme"),
        ThemeMode::Dark => {}
    }
    match preferences.glass {
        GlassStyle::Standard => {}
        GlassStyle::Clear => window.add_css_class("glass-clear"),
        GlassStyle::Tinted => window.add_css_class("glass-tinted"),
    }
    if preferences.reduce_transparency {
        window.add_css_class("reduce-transparency");
    }
    let accent_class = match preferences.accent {
        AccentColor::Blue => "accent-blue",
        AccentColor::Purple => "accent-purple",
        AccentColor::Pink => "accent-pink",
        AccentColor::Red => "accent-red",
        AccentColor::Orange => "accent-orange",
        AccentColor::Yellow => "accent-yellow",
        AccentColor::Green => "accent-green",
        AccentColor::Graphite => "accent-graphite",
    };
    window.add_css_class(accent_class);

    if let Some(settings) = gtk::Settings::default() {
        settings.set_gtk_application_prefer_dark_theme(preferences.theme == ThemeMode::Dark);
    }
}

fn apply_tahoe_icon_theme() {
    let home = std::env::var("HOME").unwrap_or_default();
    let user_theme = PathBuf::from(format!("{}/.local/share/icons/MacTahoe", home));
    let sys_theme = PathBuf::from("/usr/share/icons/MacTahoe");

    if user_theme.exists() || sys_theme.exists() {
        if let Some(settings) = gtk::Settings::default() {
            settings.set_gtk_icon_theme_name(Some("MacTahoe"));
        }
        if let Some(display) = gtk::gdk::Display::default() {
            IconTheme::for_display(&display).set_theme_name(Some("MacTahoe"));
        }
    }
}

fn format_tahoe_clock(use_24_hour: bool) -> String {
    glib::DateTime::now_local()
        .ok()
        .and_then(|dt| dt.format(clock_format_string(use_24_hour)).ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "MARCH SCARA".to_string())
}

fn clock_format_string(use_24_hour: bool) -> &'static str {
    if use_24_hour {
        "%a %b %-d  %H:%M"
    } else {
        "%a %b %-d  %-I:%M %p"
    }
}

fn battery_paths() -> Vec<PathBuf> {
    let mut batteries = std::fs::read_dir("/sys/class/power_supply")
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            let kind = std::fs::read_to_string(path.join("type")).ok()?;
            (kind.trim() == "Battery").then_some(path)
        })
        .collect::<Vec<_>>();
    if batteries.is_empty() {
        batteries.extend(
            ["BAT0", "BAT1"].map(|name| PathBuf::from("/sys/class/power_supply").join(name)),
        );
    }

    batteries
}

fn read_battery_info() -> (String, String) {
    for battery in battery_paths() {
        let cap_path = battery.join("capacity");
        let stat_path = battery.join("status");
        if let Ok(cap) = std::fs::read_to_string(cap_path) {
            let status = std::fs::read_to_string(stat_path)
                .unwrap_or_else(|_| "Status unavailable".into())
                .trim()
                .to_string();
            return (format!("{}%", cap.trim()), status);
        }
    }
    (
        "No battery detected".to_string(),
        "Not available".to_string(),
    )
}

fn read_battery_health() -> (Option<String>, Option<String>) {
    for battery in battery_paths() {
        let energy_pair = ("energy_full", "energy_full_design");
        let charge_pair = ("charge_full", "charge_full_design");
        let capacity = [energy_pair, charge_pair]
            .into_iter()
            .find_map(|(full, design)| {
                let full = std::fs::read_to_string(battery.join(full))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()?;
                let design = std::fs::read_to_string(battery.join(design))
                    .ok()?
                    .trim()
                    .parse::<u64>()
                    .ok()?;
                battery_health_percentage(full, design)
                    .map(|percentage| format!("{percentage:.0}%"))
            });
        let cycles = std::fs::read_to_string(battery.join("cycle_count"))
            .ok()
            .map(|value| format!("{} cycles", value.trim()));
        if capacity.is_some() || cycles.is_some() {
            return (capacity, cycles);
        }
    }
    (None, None)
}

fn read_battery_history() -> Result<Vec<BatteryHistorySample>, String> {
    let devices = run_program_output("upower", &["-e"])?;
    let device = devices
        .lines()
        .find(|path| path.contains("/battery_"))
        .ok_or_else(|| "UPower did not report a battery device.".to_string())?;
    let output = run_program_output(
        "busctl",
        &[
            "--system",
            "--timeout=3s",
            "call",
            "org.freedesktop.UPower",
            device,
            "org.freedesktop.UPower.Device",
            "GetHistory",
            "suu",
            "charge",
            "86400",
            "3600",
        ],
    )?;
    parse_battery_history(&output)
}

fn battery_health_percentage(full: u64, design: u64) -> Option<f64> {
    (design > 0).then(|| (full as f64 / design as f64 * 100.0).clamp(0.0, 100.0))
}

fn os_display_name() -> String {
    std::fs::read_to_string("/etc/os-release")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("PRETTY_NAME=")
                    .map(|value| value.trim_matches('"').to_string())
            })
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| "Linux (version unavailable)".to_string())
}

fn cpu_display_name() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                line.strip_prefix("model name\t: ")
                    .or_else(|| line.strip_prefix("Hardware\t: "))
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| "Not reported by the system".to_string())
}

fn memory_display_size() -> String {
    std::fs::read_to_string("/proc/meminfo")
        .ok()
        .and_then(|contents| {
            contents.lines().find_map(|line| {
                let value = line.strip_prefix("MemTotal:")?.split_whitespace().next()?;
                value.parse::<u64>().ok()
            })
        })
        .map(|kib| format!("{:.1} GiB", kib as f64 / 1_048_576.0))
        .unwrap_or_else(|| "Not reported by the system".to_string())
}

fn parse_storage_row(row: &str) -> Result<(String, f64), String> {
    let columns: Vec<&str> = row.split_whitespace().collect();
    if columns.len() < 6 {
        return Err("Filesystem usage data was incomplete".to_string());
    }
    let used_percentage = columns[4]
        .strip_suffix('%')
        .ok_or_else(|| "Filesystem usage did not include a percentage".to_string())?
        .parse::<f64>()
        .map_err(|error| format!("Invalid filesystem usage percentage: {error}"))?;

    Ok((
        format!(
            "{} total · {} used · {} available · {} used",
            columns[1], columns[2], columns[3], columns[4]
        ),
        (used_percentage / 100.0).clamp(0.0, 1.0),
    ))
}

fn root_storage_display() -> Result<(String, f64), String> {
    let output = Command::new("df")
        .args(["-hP", "--", "/"])
        .output()
        .map_err(|error| format!("Could not run df: {error}"))?;
    if !output.status.success() {
        return Err(format!("df exited with {}", output.status));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let row = stdout
        .lines()
        .skip(1)
        .find(|line| !line.trim().is_empty())
        .ok_or_else(|| "No filesystem usage data was returned".to_string())?;
    parse_storage_row(row)
}

fn build_wallpaper(app: &Application) -> Picture {
    let bg_win = ApplicationWindow::builder()
        .application(app)
        .title("scara-wallpaper")
        .build();

    bg_win.init_layer_shell();
    bg_win.set_layer(Layer::Background);
    bg_win.set_namespace(Some("scara-wallpaper"));
    bg_win.set_exclusive_zone(-1);
    bg_win.set_anchor(Edge::Top, true);
    bg_win.set_anchor(Edge::Bottom, true);
    bg_win.set_anchor(Edge::Left, true);
    bg_win.set_anchor(Edge::Right, true);

    let background = GtkBox::new(Orientation::Vertical, 0);
    background.add_css_class("tahoe-wallpaper-fallback");
    let picture = Picture::new();
    picture.set_content_fit(ContentFit::Cover);
    picture.set_can_shrink(true);
    picture.set_hexpand(true);
    picture.set_vexpand(true);
    if let Some(path) = find_wallpaper_path() {
        if let Some(filename) = path.to_str() {
            picture.set_filename(Some(filename));
        }
    }
    background.append(&picture);
    bg_win.set_child(Some(&background));

    bg_win.present();
    picture
}

fn build_app_launcher(
    app: &Application,
    settings_window: &ApplicationWindow,
    settings_navigator: &SettingsNavigator,
) -> (ApplicationWindow, SearchEntry) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("Applications")
        .default_width(760)
        .default_height(620)
        .build();
    window.set_hide_on_close(true);
    window.add_css_class("apps-launcher");

    let root = GtkBox::new(Orientation::Vertical, 12);
    root.set_margin_top(20);
    root.set_margin_bottom(20);
    root.set_margin_start(24);
    root.set_margin_end(24);

    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search applications"));
    root.append(&search);

    let grid = Grid::new();
    grid.set_row_spacing(12);
    grid.set_column_spacing(12);
    grid.set_column_homogeneous(true);

    let scroll = ScrolledWindow::builder()
        .hexpand(true)
        .vexpand(true)
        .child(&grid)
        .build();
    root.append(&scroll);
    window.set_child(Some(&root));

    let mut apps = load_desktop_apps();
    apps.push(DesktopApp {
        name: "MARCH SCARA Settings".into(),
        exec: "__scara_settings__".into(),
        icon: "preferences-system-symbolic".into(),
    });
    for (name, panel) in [
        ("Wi-Fi Settings", "wifi"),
        ("Bluetooth Settings", "bluetooth"),
        ("Network Settings", "network"),
        ("Night Shift / Layar Kuning", "display"),
        ("Battery Settings", "battery"),
        ("Appearance Settings", "appearance"),
        ("Wallpaper Settings", "wallpaper"),
        ("Privacy & Security", "privacy"),
        ("Keyboard Settings", "keyboard"),
        ("Accessibility Settings", "accessibility"),
    ] {
        apps.push(DesktopApp {
            name: name.into(),
            exec: format!("__scara_settings__:{panel}"),
            icon: "preferences-system-symbolic".into(),
        });
    }
    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    let populate = {
        let grid = grid.clone();
        let apps = apps.clone();
        let window = window.clone();
        let settings_window = settings_window.clone();
        let settings_navigator = settings_navigator.clone();
        move |query: &str| {
            while let Some(child) = grid.first_child() {
                grid.remove(&child);
            }

            let mut column = 0;
            let mut row = 0;
            for app in apps.iter().filter(|app| launcher_entry_matches(query, app)) {
                let icon = Image::from_icon_name(&app.icon);
                icon.set_pixel_size(40);
                let label = Label::new(Some(&app.name));
                label.set_wrap(true);
                label.set_ellipsize(gtk::pango::EllipsizeMode::End);
                label.set_max_width_chars(18);

                let content = GtkBox::new(Orientation::Vertical, 8);
                content.set_halign(Align::Center);
                content.append(&icon);
                content.append(&label);

                let button = Button::builder().child(&content).build();
                button.add_css_class("flat");
                button.add_css_class("menu-action-btn");
                button.set_size_request(118, 100);

                let exec = app.exec.clone();
                let window = window.clone();
                let settings_window = settings_window.clone();
                let settings_navigator = settings_navigator.clone();
                button.connect_clicked(move |_| {
                    window.set_visible(false);
                    activate_launcher_entry(&exec, &settings_window, &settings_navigator);
                });
                grid.attach(&button, column, row, 1, 1);
                column += 1;
                if column == 5 {
                    column = 0;
                    row += 1;
                }
            }
        }
    };

    populate("");
    let populate_search = populate.clone();
    search.connect_search_changed(move |entry| populate_search(&entry.text()));

    let apps_for_enter = apps;
    let window_for_enter = window.clone();
    let settings_for_enter = settings_window.clone();
    let settings_navigator_for_enter = settings_navigator.clone();
    search.connect_activate(move |entry| {
        let query = entry.text();
        if let Some(app) = apps_for_enter
            .iter()
            .find(|app| launcher_entry_matches(&query, app))
        {
            window_for_enter.set_visible(false);
            activate_launcher_entry(
                &app.exec,
                &settings_for_enter,
                &settings_navigator_for_enter,
            );
        }
    });

    (window, search)
}

fn settings_page(title: &str, description: &str) -> GtkBox {
    let page = GtkBox::new(Orientation::Vertical, 14);
    page.set_margin_top(24);
    page.set_margin_bottom(24);
    page.set_margin_start(22);
    page.set_margin_end(22);

    let heading = Label::new(Some(title));
    heading.add_css_class("cc-title");
    heading.add_css_class("settings-page-title");
    heading.set_halign(Align::Start);
    page.append(&heading);

    let detail = Label::new(Some(description));
    detail.add_css_class("cc-sub");
    detail.set_halign(Align::Start);
    detail.set_wrap(true);
    page.append(&detail);
    page
}

fn append_display_output_cards(page: &GtkBox) {
    let outputs = match run_wm_ctl_output("display:list").and_then(|response| {
        serde_json::from_str::<Vec<DisplayOutputInfo>>(&response).map_err(|error| error.to_string())
    }) {
        Ok(outputs) => outputs,
        Err(error) => {
            add_settings_integration_note(
                page,
                "Display settings unavailable",
                &format!("Could not read outputs from the active compositor: {error}"),
            );
            return;
        }
    };

    if outputs.is_empty() {
        add_settings_integration_note(
            page,
            "No displays found",
            "The active compositor did not report any connected display outputs.",
        );
        return;
    }

    for output in outputs {
        let current_refresh = f64::from(output.current.refresh_millihz) / 1000.0;
        let logical_width = i32::from(output.current.width) / output.scale.max(1);
        let logical_height = i32::from(output.current.height) / output.scale.max(1);
        let detail = format!(
            "{} backend · {} × {} at {:.2} Hz · logical {} × {} · scale {}x · position ({}, {})",
            output.backend,
            output.current.width,
            output.current.height,
            current_refresh,
            logical_width,
            logical_height,
            output.scale,
            output.x,
            output.y
        );
        let card = settings_card(&output.name, &detail);
        let mut modes = output.modes;
        if !modes.contains(&output.current) {
            modes.push(output.current.clone());
        }
        modes.sort_by_key(|mode| (mode.width, mode.height, mode.refresh_millihz));
        modes.dedup();
        let mode_labels = modes
            .iter()
            .map(|mode| {
                format!(
                    "{} × {} at {:.2} Hz",
                    mode.width,
                    mode.height,
                    f64::from(mode.refresh_millihz) / 1000.0
                )
            })
            .collect::<Vec<_>>();
        let mode_dropdown =
            DropDown::from_strings(&mode_labels.iter().map(String::as_str).collect::<Vec<_>>());
        let current_mode_index = modes
            .iter()
            .position(|mode| mode == &output.current)
            .unwrap_or(0);
        mode_dropdown.set_selected(current_mode_index as u32);
        card.append(&settings_control_row(
            "Resolution and refresh rate",
            &mode_dropdown,
        ));

        let scale_dropdown =
            DropDown::from_strings(&["100% (1x)", "200% (2x)", "300% (3x)", "400% (4x)"]);
        scale_dropdown.set_selected((output.scale.clamp(1, 4) - 1) as u32);
        card.append(&settings_control_row("Scale", &scale_dropdown));

        let x_position = gtk::SpinButton::with_range(-16384.0, 16384.0, 1.0);
        x_position.set_value(f64::from(output.x));
        let y_position = gtk::SpinButton::with_range(-16384.0, 16384.0, 1.0);
        y_position.set_value(f64::from(output.y));
        card.append(&settings_control_row("Horizontal position", &x_position));
        card.append(&settings_control_row("Vertical position", &y_position));

        let actions = GtkBox::new(Orientation::Horizontal, 8);
        let apply = Button::with_label("Preview changes");
        apply.add_css_class("flat");
        apply.add_css_class("menu-action-btn");
        let confirm = Button::with_label("Keep");
        confirm.add_css_class("flat");
        confirm.add_css_class("menu-action-btn");
        let cancel = Button::with_label("Revert");
        cancel.add_css_class("flat");
        cancel.add_css_class("menu-action-btn");
        actions.append(&apply);
        actions.append(&confirm);
        actions.append(&cancel);
        card.append(&actions);

        let status = Label::new(Some(
            "Changes are previewed for 15 seconds before automatic revert.",
        ));
        status.add_css_class("cc-sub");
        status.set_halign(Align::Start);
        status.set_wrap(true);
        card.append(&status);

        let output_name = output.name;
        let modes_for_apply = modes.clone();
        let mode_dropdown_for_apply = mode_dropdown.clone();
        let scale_dropdown_for_apply = scale_dropdown.clone();
        let x_for_apply = x_position.clone();
        let y_for_apply = y_position.clone();
        let status_for_apply = status.clone();
        apply.connect_clicked(move |_| {
            let selected_mode = mode_dropdown_for_apply.selected() as usize;
            let Some(mode) = modes_for_apply.get(selected_mode) else {
                status_for_apply.set_text("Select a supported display mode first.");
                return;
            };
            let scale = scale_dropdown_for_apply.selected().saturating_add(1);
            let command = format!(
                "display:preview:{}:{}:{}:{}:{}:{}:{}",
                output_name,
                mode.width,
                mode.height,
                mode.refresh_millihz,
                scale,
                x_for_apply.value_as_int(),
                y_for_apply.value_as_int()
            );
            match run_wm_ctl_output(&command) {
                Ok(response) => status_for_apply.set_text(&response),
                Err(error) => status_for_apply.set_text(&error),
            }
        });

        let status_for_confirm = status.clone();
        confirm.connect_clicked(move |_| match run_wm_ctl_output("display:confirm") {
            Ok(response) => status_for_confirm.set_text(&response),
            Err(error) => status_for_confirm.set_text(&error),
        });
        let status_for_cancel = status.clone();
        cancel.connect_clicked(move |_| match run_wm_ctl_output("display:cancel") {
            Ok(response) => status_for_cancel.set_text(&response),
            Err(error) => status_for_cancel.set_text(&error),
        });

        page.append(&card);
    }
}

fn append_sound_settings(page: &GtkBox) {
    let server_info = match run_program_output("pactl", &["info"]) {
        Ok(info) => info,
        Err(error) => {
            add_settings_integration_note(
                page,
                "PipeWire audio unavailable",
                &format!(
                    "Could not connect to the audio server through PipeWire's PulseAudio interface: {error}"
                ),
            );
            add_settings_tool(page, "Open audio mixer", "pavucontrol", &[]);
            return;
        }
    };
    let server_name = server_info
        .lines()
        .find_map(|line| line.strip_prefix("Server Name:").map(str::trim))
        .unwrap_or("Unknown audio server");
    page.append(&settings_card("Audio server", server_name));

    let output_devices = match run_program_output("pactl", &["list", "short", "sinks"]) {
        Ok(output) => parse_audio_devices(&output, true),
        Err(error) => {
            add_settings_integration_note(
                page,
                "Output devices unavailable",
                &format!("Could not enumerate audio outputs: {error}"),
            );
            Vec::new()
        }
    };

    if output_devices.is_empty() {
        add_settings_integration_note(
            page,
            "No audio outputs found",
            "The audio server did not report any output devices.",
        );
    } else {
        let default_output_result = run_program_output("pactl", &["get-default-sink"]);
        let default_output = default_output_result.as_ref().ok().cloned();
        let default_output_found = default_output
            .as_ref()
            .is_some_and(|name| output_devices.iter().any(|device| &device.name == name));
        let initial_output_index = default_output
            .as_ref()
            .and_then(|name| {
                output_devices
                    .iter()
                    .position(|device| &device.name == name)
            })
            .unwrap_or(0);
        let output_labels = output_devices
            .iter()
            .map(|device| device.name.replace('_', " "))
            .collect::<Vec<_>>();
        let output_dropdown =
            DropDown::from_strings(&output_labels.iter().map(String::as_str).collect::<Vec<_>>());
        output_dropdown.set_selected(initial_output_index as u32);
        let output_card = settings_card("Output", "Choose the default speaker or headphone.");
        output_card.append(&settings_control_row("Default output", &output_dropdown));

        let initial_output_name = output_devices[initial_output_index].name.clone();
        let output_volume = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        let output_channels = Rc::new(RefCell::new(Vec::<f64>::new()));
        let mut output_initial_status = match &default_output_result {
            Err(error) => Some(format!("Could not identify the default output: {error}")),
            Ok(name) if !default_output_found => Some(format!(
                "Default output {name} was not in the enumerated device list; selecting a device will set it."
            )),
            Ok(_) => None,
        };
        match run_program_output("pactl", &["get-sink-volume", &initial_output_name]) {
            Ok(volume_text) => {
                let volumes = parse_audio_channel_volumes(&volume_text);
                if let Some(volume) = volumes.iter().copied().reduce(f64::max) {
                    output_volume.set_value(volume);
                } else {
                    output_initial_status =
                        Some("The audio server did not report output volume channels.".into());
                }
                output_channels.replace(volumes);
            }
            Err(error) => {
                output_initial_status = Some(format!("Could not read output volume: {error}"));
            }
        }
        output_card.append(&settings_control_row("Output volume", &output_volume));

        let output_balance = Scale::with_range(Orientation::Horizontal, -100.0, 100.0, 1.0);
        output_balance
            .set_value(audio_balance_from_volumes(&output_channels.borrow()).unwrap_or_default());
        output_balance.set_sensitive(output_channels.borrow().len() == 2);
        output_card.append(&settings_control_row(
            "Balance (left to right)",
            &output_balance,
        ));

        let output_initial_status = output_initial_status
            .unwrap_or_else(|| "Changes are applied through the active audio server.".into());
        let output_status = Label::new(Some(&output_initial_status));
        output_status.add_css_class("cc-sub");
        output_status.set_halign(Align::Start);
        output_status.set_wrap(true);
        output_card.append(&output_status);

        let active_output = Rc::new(RefCell::new(initial_output_name));
        let updating_output_controls = Rc::new(Cell::new(false));
        let output_name_for_volume = active_output.clone();
        let channels_for_volume = output_channels.clone();
        let balance_for_volume = output_balance.clone();
        let updating_for_volume = updating_output_controls.clone();
        let status_for_volume = output_status.clone();
        output_volume.connect_value_changed(move |slider| {
            if updating_for_volume.get() {
                return;
            }
            let name = output_name_for_volume.borrow().clone();
            let volume = slider.value().round();
            let balance = balance_for_volume.value() / 100.0;
            let result = if channels_for_volume.borrow().len() == 2 {
                let left = volume * if balance > 0.0 { 1.0 - balance } else { 1.0 };
                let right = volume * if balance < 0.0 { 1.0 + balance } else { 1.0 };
                run_program_output(
                    "pactl",
                    &[
                        "set-sink-volume",
                        &name,
                        &format!("{left:.0}%"),
                        &format!("{right:.0}%"),
                    ],
                )
            } else {
                run_program_output(
                    "pactl",
                    &["set-sink-volume", &name, &format!("{volume:.0}%")],
                )
            };
            match result {
                Ok(_) => status_for_volume.set_text("Output volume applied."),
                Err(error) => {
                    status_for_volume.set_text(&format!("Could not set output volume: {error}"))
                }
            }
        });

        let output_name_for_balance = active_output.clone();
        let volume_for_balance = output_volume.clone();
        let channels_for_balance = output_channels.clone();
        let updating_for_balance = updating_output_controls.clone();
        let status_for_balance = output_status.clone();
        output_balance.connect_value_changed(move |slider| {
            if updating_for_balance.get() {
                return;
            }
            if channels_for_balance.borrow().len() != 2 {
                status_for_balance.set_text("Balance is supported only for stereo output devices.");
                return;
            }
            let name = output_name_for_balance.borrow().clone();
            let volume = volume_for_balance.value().round();
            let balance = slider.value() / 100.0;
            let left = volume * if balance > 0.0 { 1.0 - balance } else { 1.0 };
            let right = volume * if balance < 0.0 { 1.0 + balance } else { 1.0 };
            match run_program_output(
                "pactl",
                &[
                    "set-sink-volume",
                    &name,
                    &format!("{left:.0}%"),
                    &format!("{right:.0}%"),
                ],
            ) {
                Ok(_) => status_for_balance.set_text("Output balance applied."),
                Err(error) => {
                    status_for_balance.set_text(&format!("Could not set output balance: {error}"))
                }
            }
        });

        let devices_for_select = output_devices;
        let active_for_select = active_output.clone();
        let channels_for_select = output_channels.clone();
        let volume_for_select = output_volume.clone();
        let balance_for_select = output_balance.clone();
        let updating_for_select = updating_output_controls.clone();
        let status_for_select = output_status.clone();
        output_dropdown.connect_selected_notify(move |dropdown| {
            let selected = dropdown.selected() as usize;
            let Some(device) = devices_for_select.get(selected) else {
                return;
            };
            match run_program_output("pactl", &["set-default-sink", &device.name]) {
                Ok(_) => {
                    active_for_select.replace(device.name.clone());
                    match run_program_output("pactl", &["get-sink-volume", &device.name]) {
                        Ok(volume_text) => {
                            let volumes = parse_audio_channel_volumes(&volume_text);
                            channels_for_select.replace(volumes.clone());
                            updating_for_select.set(true);
                            balance_for_select.set_sensitive(volumes.len() == 2);
                            if let Some(balance) = audio_balance_from_volumes(&volumes) {
                                balance_for_select.set_value(balance);
                            }
                            if let Some(volume) = volumes.iter().copied().reduce(f64::max) {
                                volume_for_select.set_value(volume);
                            }
                            updating_for_select.set(false);
                            status_for_select.set_text("Default output changed.");
                        }
                        Err(error) => status_for_select.set_text(&format!(
                            "Default output changed, but its volume could not be read: {error}"
                        )),
                    }
                }
                Err(error) => {
                    status_for_select.set_text(&format!("Could not select default output: {error}"))
                }
            }
        });
        page.append(&output_card);
    }

    let input_devices = match run_program_output("pactl", &["list", "short", "sources"]) {
        Ok(output) => parse_audio_devices(&output, false),
        Err(error) => {
            add_settings_integration_note(
                page,
                "Input devices unavailable",
                &format!("Could not enumerate microphones: {error}"),
            );
            Vec::new()
        }
    };

    if input_devices.is_empty() {
        add_settings_integration_note(
            page,
            "No microphones found",
            "The audio server did not report any microphone input devices.",
        );
    } else {
        let default_input_result = run_program_output("pactl", &["get-default-source"]);
        let default_input = default_input_result.as_ref().ok().cloned();
        let default_input_found = default_input
            .as_ref()
            .is_some_and(|name| input_devices.iter().any(|device| &device.name == name));
        let initial_input_index = default_input
            .as_ref()
            .and_then(|name| input_devices.iter().position(|device| &device.name == name))
            .unwrap_or(0);
        let input_labels = input_devices
            .iter()
            .map(|device| device.name.replace('_', " "))
            .collect::<Vec<_>>();
        let input_dropdown =
            DropDown::from_strings(&input_labels.iter().map(String::as_str).collect::<Vec<_>>());
        input_dropdown.set_selected(initial_input_index as u32);
        let input_card = settings_card("Input", "Choose and adjust the default microphone.");
        input_card.append(&settings_control_row("Default input", &input_dropdown));

        let initial_input_name = input_devices[initial_input_index].name.clone();
        let input_volume = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 1.0);
        let mut input_initial_status = match &default_input_result {
            Err(error) => Some(format!("Could not identify the default input: {error}")),
            Ok(name) if !default_input_found => Some(format!(
                "Default input {name} was not in the enumerated device list; selecting a device will set it."
            )),
            Ok(_) => None,
        };
        match run_program_output("pactl", &["get-source-volume", &initial_input_name]) {
            Ok(volume_text) => {
                if let Some(volume) = parse_audio_channel_volumes(&volume_text)
                    .iter()
                    .copied()
                    .reduce(f64::max)
                {
                    input_volume.set_value(volume);
                } else {
                    input_initial_status =
                        Some("The audio server did not report microphone volume channels.".into());
                }
            }
            Err(error) => {
                input_initial_status = Some(format!("Could not read microphone level: {error}"));
            }
        }
        input_card.append(&settings_control_row("Microphone level", &input_volume));

        let input_muted_result =
            run_program_output("pactl", &["get-source-mute", &initial_input_name]);
        let input_muted = input_muted_result
            .as_ref()
            .is_ok_and(|result| result.trim() == "Mute: yes");
        if let Err(error) = &input_muted_result {
            input_initial_status = Some(format!("Could not read microphone mute state: {error}"));
        }
        let mute_switch = Switch::new();
        mute_switch.set_active(input_muted);
        input_card.append(&settings_control_row("Mute microphone", &mute_switch));

        let input_initial_status = input_initial_status
            .unwrap_or_else(|| "Changes are applied through the active audio server.".into());
        let input_status = Label::new(Some(&input_initial_status));
        input_status.add_css_class("cc-sub");
        input_status.set_halign(Align::Start);
        input_status.set_wrap(true);
        input_card.append(&input_status);

        let active_input = Rc::new(RefCell::new(initial_input_name));
        let input_for_volume = active_input.clone();
        let input_status_for_volume = input_status.clone();
        input_volume.connect_value_changed(move |slider| {
            let name = input_for_volume.borrow().clone();
            match run_program_output(
                "pactl",
                &[
                    "set-source-volume",
                    &name,
                    &format!("{:.0}%", slider.value()),
                ],
            ) {
                Ok(_) => input_status_for_volume.set_text("Microphone level applied."),
                Err(error) => input_status_for_volume
                    .set_text(&format!("Could not set microphone level: {error}")),
            }
        });

        let input_for_mute = active_input.clone();
        let input_status_for_mute = input_status.clone();
        mute_switch.connect_active_notify(move |switch| {
            let name = input_for_mute.borrow().clone();
            let value = if switch.is_active() { "1" } else { "0" };
            match run_program_output("pactl", &["set-source-mute", &name, value]) {
                Ok(_) => input_status_for_mute.set_text(if switch.is_active() {
                    "Microphone muted."
                } else {
                    "Microphone unmuted."
                }),
                Err(error) => input_status_for_mute
                    .set_text(&format!("Could not change microphone mute: {error}")),
            }
        });

        let devices_for_select = input_devices;
        let active_for_select = active_input.clone();
        let volume_for_select = input_volume.clone();
        let mute_for_select = mute_switch.clone();
        let status_for_select = input_status.clone();
        input_dropdown.connect_selected_notify(move |dropdown| {
            let selected = dropdown.selected() as usize;
            let Some(device) = devices_for_select.get(selected) else {
                return;
            };
            match run_program_output("pactl", &["set-default-source", &device.name]) {
                Ok(_) => {
                    active_for_select.replace(device.name.clone());
                    if let Ok(volume_text) =
                        run_program_output("pactl", &["get-source-volume", &device.name])
                    {
                        if let Some(volume) = parse_audio_channel_volumes(&volume_text)
                            .iter()
                            .copied()
                            .reduce(f64::max)
                        {
                            volume_for_select.set_value(volume);
                        }
                    }
                    let mute_result =
                        run_program_output("pactl", &["get-source-mute", &device.name]);
                    match mute_result {
                        Ok(result) => {
                            mute_for_select.set_active(result.trim() == "Mute: yes");
                            status_for_select.set_text("Default input changed.");
                        }
                        Err(error) => status_for_select.set_text(&format!(
                            "Default input changed, but mute state could not be read: {error}"
                        )),
                    }
                }
                Err(error) => {
                    status_for_select.set_text(&format!("Could not select default input: {error}"))
                }
            }
        });
        page.append(&input_card);
    }

    add_settings_tool(page, "Open audio mixer", "pavucontrol", &[]);
}

fn append_battery_settings(page: &GtkBox) {
    let (capacity, state) = read_battery_info();
    page.append(&settings_card(
        "Battery",
        &format!("Charge: {capacity} · State: {state}"),
    ));

    let (health, cycles) = read_battery_health();
    page.append(&settings_card(
        "Battery Health",
        &format!(
            "Maximum capacity: {}\nCycle count: {}",
            health.unwrap_or_else(|| "Not reported by hardware".to_string()),
            cycles.unwrap_or_else(|| "Not reported by hardware".to_string())
        ),
    ));

    match read_battery_history() {
        Ok(mut history) if !history.is_empty() => {
            history.sort_by_key(|sample| sample.timestamp);
            let card = settings_card(
                "Charge history",
                "UPower-reported battery charge samples from the last 24 hours.",
            );
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let sample_count = history.len().min(12);
            for sample_index in 0..sample_count {
                let history_index = if sample_count == 1 {
                    history.len() - 1
                } else {
                    sample_index * (history.len() - 1) / (sample_count - 1)
                };
                let sample = &history[history_index];
                let hours_ago = now.saturating_sub(sample.timestamp) / 3600;
                let state = match sample.state {
                    1 => "charging",
                    2 => "discharging",
                    3 => "empty",
                    4 => "fully charged",
                    5 => "charge pending",
                    6 => "discharge pending",
                    _ => "state unavailable",
                };
                let bar = ProgressBar::new();
                bar.set_fraction(sample.percentage / 100.0);
                bar.set_show_text(true);
                bar.set_text(Some(&format!(
                    "{hours_ago}h ago · {:.0}% · {state}",
                    sample.percentage
                )));
                card.append(&bar);
            }
            page.append(&card);
        }
        Ok(_) => add_settings_integration_note(
            page,
            "Charge history unavailable",
            "UPower reported no stored battery history samples.",
        ),
        Err(error) => add_settings_integration_note(
            page,
            "Charge history unavailable",
            &format!("Could not read history from UPower: {error}"),
        ),
    }

    match run_program_output("powerprofilesctl", &["list"]) {
        Ok(output) => {
            let profiles = parse_power_profiles(&output);
            if profiles.is_empty() {
                add_settings_integration_note(
                    page,
                    "Power modes unavailable",
                    "The power service did not report any selectable profiles.",
                );
            } else {
                let current_result = run_program_output("powerprofilesctl", &["get"]);
                let current = current_result.as_ref().ok().cloned();
                let current_index = current
                    .as_ref()
                    .and_then(|current| profiles.iter().position(|profile| profile == current))
                    .unwrap_or(0);
                let profile_dropdown = DropDown::from_strings(
                    &profiles.iter().map(String::as_str).collect::<Vec<_>>(),
                );
                profile_dropdown.set_selected(current_index as u32);
                let card = settings_card(
                    "Power mode",
                    "Set a mode supported by the host power-profiles service.",
                );
                card.append(&settings_control_row("Profile", &profile_dropdown));
                let profile_status = match current_result {
                    Ok(profile) if profiles.iter().any(|available| available == &profile) => {
                        format!("Current profile: {profile}")
                    }
                    Ok(profile) => format!(
                        "Current profile {profile} is not in the service's reported profile list."
                    ),
                    Err(error) => format!("Could not read the current power profile: {error}"),
                };
                let status = Label::new(Some(&profile_status));
                status.add_css_class("cc-sub");
                status.set_halign(Align::Start);
                card.append(&status);

                let profiles_for_select = profiles;
                let status_for_select = status.clone();
                profile_dropdown.connect_selected_notify(move |dropdown| {
                    let selected = dropdown.selected() as usize;
                    let Some(profile) = profiles_for_select.get(selected) else {
                        return;
                    };
                    match run_program_output("powerprofilesctl", &["set", profile]) {
                        Ok(_) => {
                            status_for_select.set_text(&format!("Power profile set to {profile}."))
                        }
                        Err(error) => status_for_select
                            .set_text(&format!("Could not set power profile: {error}")),
                    }
                });
                page.append(&card);
            }
        }
        Err(error) => add_settings_integration_note(
            page,
            "Power modes unavailable",
            &format!("power-profiles-daemon is not available: {error}"),
        ),
    }

    add_settings_tool(
        page,
        "Open power settings",
        "gnome-control-center",
        &["power"],
    );
}

fn settings_search_matches(query: &str, searchable_text: &str) -> bool {
    let normalized_query = query
        .trim()
        .to_lowercase()
        .replace("layar kuning", "night shift")
        .replace("resolusi", "resolution")
        .replace("jaringan", "network")
        .replace("baterai", "battery")
        .replace("tampilan", "display")
        .replace("suara", "sound")
        .replace("sandi", "password");
    let ignored_words = [
        "aktifkan", "ubah", "ganti", "matikan", "buka", "atur", "enable", "disable", "turn", "on",
        "off", "the", "to",
    ];
    let query_terms: Vec<&str> = normalized_query
        .split_whitespace()
        .filter(|term| !ignored_words.contains(term))
        .collect();
    if query_terms.is_empty() {
        return true;
    }

    let candidates: Vec<&str> = searchable_text.split_whitespace().collect();
    query_terms.iter().all(|term| {
        candidates
            .iter()
            .any(|candidate| candidate.contains(term) || one_edit_away(term, candidate))
    })
}

fn settings_panel_from_uri(uri: &str) -> Option<&'static str> {
    let path = uri.strip_prefix("mactahoe://settings/")?;
    let path = path
        .split(|character| matches!(character, '?' | '#'))
        .next()?;
    let mut segments = path.split('/');
    let panel = segments.next()?;
    let subpanel = segments.next();

    match (panel, subpanel) {
        ("network", Some("wifi" | "wi-fi")) => Some("wifi"),
        ("display", _) | ("displays", _) => Some("display"),
        ("appearance", _) => Some("appearance"),
        ("privacy", _) => Some("privacy"),
        ("battery", _) | ("power", _) => Some("battery"),
        ("sound", _) | ("audio", _) => Some("sound"),
        ("bluetooth", _) => Some("bluetooth"),
        ("wifi", _) | ("wi-fi", _) => Some("wifi"),
        ("network", _) => Some("network"),
        ("wallpaper", _) => Some("wallpaper"),
        ("general", _) | ("system", _) => Some("general"),
        ("keyboard", _) => Some("keyboard"),
        ("trackpad", _) => Some("trackpad"),
        ("mouse", _) => Some("mouse"),
        ("accessibility", _) => Some("accessibility"),
        ("notifications", _) | ("focus", _) => Some("notifications"),
        ("account", _) | ("profile", _) => Some("account"),
        ("dock", _) | ("desktop", _) => Some("desktop_dock"),
        ("control-center", _) | ("menu-bar", _) => Some("control_center"),
        ("lock-screen", _) => Some("lock_screen"),
        ("passwords", _) | ("passkeys", _) => Some("passwords"),
        ("printers", _) => Some("printers"),
        _ => None,
    }
}

fn one_edit_away(left: &str, right: &str) -> bool {
    let left: Vec<char> = left.chars().collect();
    let right: Vec<char> = right.chars().collect();
    if left.len().abs_diff(right.len()) > 1 {
        return false;
    }

    let (mut left_index, mut right_index, mut edits) = (0, 0, 0);
    while left_index < left.len() && right_index < right.len() {
        if left[left_index] == right[right_index] {
            left_index += 1;
            right_index += 1;
        } else {
            edits += 1;
            if edits > 1 {
                return false;
            }
            if left.len() > right.len() {
                left_index += 1;
            } else if right.len() > left.len() {
                right_index += 1;
            } else {
                left_index += 1;
                right_index += 1;
            }
        }
    }

    edits + usize::from(left_index < left.len() || right_index < right.len()) <= 1
}

fn settings_card(title: &str, description: &str) -> GtkBox {
    let card = GtkBox::new(Orientation::Vertical, 8);
    card.add_css_class("cc-card");

    let heading = Label::new(Some(title));
    heading.add_css_class("cc-title");
    heading.set_halign(Align::Start);

    let detail = Label::new(Some(description));
    detail.add_css_class("cc-sub");
    detail.set_halign(Align::Start);
    detail.set_wrap(true);

    card.append(&heading);
    card.append(&detail);
    card
}

fn settings_control_row(label: &str, control: &impl gtk::prelude::IsA<gtk::Widget>) -> GtkBox {
    let row = GtkBox::new(Orientation::Horizontal, 12);
    let title = Label::new(Some(label));
    title.add_css_class("cc-sub");
    title.set_halign(Align::Start);
    title.set_hexpand(true);
    row.append(&title);
    row.append(control);
    row
}

fn add_settings_integration_note(page: &GtkBox, title: &str, detail: &str) {
    let card = settings_card(title, detail);
    let note = Label::new(Some(
        "MARCH SCARA does not manage this setting yet. No system change will be made here.",
    ));
    note.add_css_class("settings-integration-note");
    note.set_halign(Align::Start);
    note.set_wrap(true);
    card.append(&note);
    page.append(&card);
}

fn add_settings_tool(
    page: &GtkBox,
    label: &str,
    program: &'static str,
    args: &'static [&'static str],
) {
    let row = GtkBox::new(Orientation::Horizontal, 10);
    let button = Button::with_label(label);
    button.add_css_class("flat");
    button.add_css_class("menu-action-btn");
    let status = Label::new(None);
    status.add_css_class("cc-sub");
    status.set_halign(Align::Start);
    let status_ref = status.clone();
    button.connect_clicked(move |_| match launch_program(program, args) {
        Ok(()) => status_ref.set_text("Opened system utility."),
        Err(error) => status_ref.set_text(&format!("{program} is unavailable: {error}")),
    });
    row.append(&button);
    row.append(&status);
    page.append(&row);
}

fn append_host_integration_diagnostics(page: &GtkBox) {
    let portal_config_path = Path::new("/usr/share/xdg-desktop-portal/march-scara-portals.conf");
    let portal_config = std::fs::read_to_string(portal_config_path);
    let mut portal_status = Vec::new();
    for (name, implementation, public_interface) in [
        (
            "File chooser",
            "org.freedesktop.impl.portal.FileChooser",
            "org.freedesktop.portal.FileChooser",
        ),
        (
            "Portal settings",
            "org.freedesktop.impl.portal.Settings",
            "org.freedesktop.portal.Settings",
        ),
        (
            "Screenshot",
            "org.freedesktop.impl.portal.Screenshot",
            "org.freedesktop.portal.Screenshot",
        ),
        (
            "Screen sharing",
            "org.freedesktop.impl.portal.ScreenCast",
            "org.freedesktop.portal.ScreenCast",
        ),
    ] {
        let result = match &portal_config {
            Ok(contents) => match configured_portal_backend(contents, implementation) {
                Some(backend) => {
                    portal_backend_implements(backend, implementation).and_then(|()| {
                        dbus_interface_available(
                            "--user",
                            "org.freedesktop.portal.Desktop",
                            "/org/freedesktop/portal/desktop",
                            public_interface,
                        )
                        .map(|()| format!("{name}: {backend} backend available"))
                    })
                }
                None => Err(format!("No backend is configured for {implementation}.")),
            },
            Err(error) => Err(format!(
                "Could not read {}: {error}",
                portal_config_path.display()
            )),
        };
        portal_status.push(match result {
            Ok(status) => status,
            Err(error) => format!("{name}: unavailable — {error}"),
        });
    }
    page.append(&settings_card(
        "XDG Desktop Portal backends",
        &portal_status.join("\n"),
    ));

    let permission_store_status = dbus_interface_available(
        "--user",
        "org.freedesktop.impl.portal.PermissionStore",
        "/org/freedesktop/impl/portal/PermissionStore",
        "org.freedesktop.impl.portal.PermissionStore",
    )
    .map(|()| {
        "PermissionStore API is available. App access requests remain owned by the portals."
            .to_string()
    })
    .unwrap_or_else(|error| {
        format!("PermissionStore unavailable; check xdg-desktop-portal: {error}")
    });
    let polkit_status = dbus_interface_available(
        "--system",
        "org.freedesktop.PolicyKit1",
        "/org/freedesktop/PolicyKit1/Authority",
        "org.freedesktop.PolicyKit1.Authority",
    )
    .map(|()| "Polkit authorization service is available.".to_string())
    .unwrap_or_else(|error| format!("Polkit service unavailable: {error}"));
    let agent_path = Path::new("/usr/lib/polkit-gnome/polkit-gnome-authentication-agent-1");
    let agent_status = if agent_path.is_file() {
        format!(
            "Polkit authentication agent found at {}.",
            agent_path.display()
        )
    } else {
        "Polkit authentication agent missing; install polkit-gnome.".to_string()
    };
    page.append(&settings_card(
        "Permission storage and administrator authorization",
        &format!("{permission_store_status}\n{polkit_status}\n{agent_status}\nMARCH SCARA does not request, store, or forward administrator passwords."),
    ));

    let utilities = [
        (
            "GNOME Control Center",
            "gnome-control-center",
            "gnome-control-center",
        ),
        ("NetworkManager CLI", "nmcli", "networkmanager"),
        ("Bluetooth CLI", "bluetoothctl", "bluez-utils"),
        ("PipeWire PulseAudio client", "pactl", "pipewire-pulse"),
        (
            "Power profile controls",
            "powerprofilesctl",
            "power-profiles-daemon",
        ),
        ("Credential manager", "seahorse", "seahorse"),
        (
            "Printer settings",
            "system-config-printer",
            "system-config-printer",
        ),
    ];
    let utility_status = utilities
        .iter()
        .map(
            |(name, program, package)| match find_program_on_path(program) {
                Some(path) => format!("{name}: available ({})", path.display()),
                None => format!("{name}: missing; install package {package}."),
            },
        )
        .collect::<Vec<_>>()
        .join("\n");
    page.append(&settings_card(
        "GNOME and system utilities",
        &utility_status,
    ));
}

fn persist_settings_change(
    preferences: &Rc<RefCell<UserPreferences>>,
    window: &ApplicationWindow,
    status: &Label,
    update: impl FnOnce(&mut UserPreferences),
) -> bool {
    match update_preferences(preferences, update) {
        Ok(()) => {
            apply_appearance(window, &preferences.borrow());
            status.set_text("Saved. Changes are applied immediately.");
            true
        }
        Err(error) => {
            status.set_text(&format!("Could not save settings: {error}"));
            false
        }
    }
}

fn build_settings_window(
    app: &Application,
    wallpaper_picture: &Picture,
    preferences: Rc<RefCell<UserPreferences>>,
    preferences_error: Option<String>,
    menu_bar_widgets: MenuBarWidgetRegistry,
) -> (ApplicationWindow, SettingsNavigator) {
    let window = ApplicationWindow::builder()
        .application(app)
        .title("MARCH SCARA Settings")
        .default_width(780)
        .default_height(720)
        .build();
    window.add_css_class("settings-window");
    apply_appearance(&window, &preferences.borrow());

    let layout = GtkBox::new(Orientation::Horizontal, 0);
    let stack = Stack::new();
    stack.set_hexpand(true);
    stack.set_vexpand(true);
    stack.set_transition_type(gtk::StackTransitionType::Crossfade);

    let sidebar = GtkBox::new(Orientation::Vertical, 6);
    sidebar.set_size_request(230, -1);
    sidebar.add_css_class("settings-sidebar");

    let search = SearchEntry::new();
    search.set_placeholder_text(Some("Search settings"));
    sidebar.append(&search);

    let profile = GtkBox::new(Orientation::Horizontal, 10);
    profile.add_css_class("settings-profile-card");
    let avatar = Image::from_icon_name("avatar-default-symbolic");
    avatar.set_size_request(40, 40);
    avatar.set_pixel_size(40);
    avatar.add_css_class("settings-avatar");
    let profile_details = GtkBox::new(Orientation::Vertical, 3);
    let username = std::env::var("USER").unwrap_or_else(|_| "Local user".to_string());
    let initial_name = preferences
        .borrow()
        .display_name
        .clone()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or(username);
    let profile_name = Label::new(Some(&initial_name));
    profile_name.add_css_class("cc-title");
    profile_name.set_halign(Align::Start);
    let profile_id = Label::new(Some("Local account"));
    profile_id.add_css_class("cc-sub");
    profile_id.set_halign(Align::Start);
    profile_details.append(&profile_name);
    profile_details.append(&profile_id);
    profile.append(&avatar);
    profile.append(&profile_details);
    sidebar.append(&profile);
    if let Some(path) = preferences
        .borrow()
        .avatar
        .as_ref()
        .filter(|path| path.is_file())
        .and_then(|path| path.to_str())
    {
        avatar.set_from_file(Some(path));
    }

    let nav_scroll = ScrolledWindow::new();
    nav_scroll.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
    nav_scroll.set_vexpand(true);
    let nav_content = GtkBox::new(Orientation::Vertical, 2);
    nav_scroll.set_child(Some(&nav_content));
    let no_results = Label::new(Some("No settings found"));
    no_results.add_css_class("cc-sub");
    no_results.set_halign(Align::Start);
    no_results.set_margin_top(12);
    no_results.set_margin_start(8);
    no_results.set_visible(false);
    nav_content.append(&no_results);
    sidebar.append(&nav_scroll);

    let content = GtkBox::new(Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);

    let page_title = Label::new(Some("User Profile"));
    page_title.add_css_class("cc-title");
    page_title.set_halign(Align::Center);
    page_title.set_hexpand(true);
    let toolbar = GtkBox::new(Orientation::Horizontal, 8);
    toolbar.add_css_class("settings-toolbar");
    let back = Button::with_label("‹");
    back.set_tooltip_text(Some("Back"));
    back.set_sensitive(false);
    let forward = Button::with_label("›");
    forward.set_tooltip_text(Some("Forward"));
    forward.set_sensitive(false);
    let help = Button::with_label("?");
    help.add_css_class("settings-help-button");
    help.set_tooltip_text(Some(
        "Supported changes apply immediately. Application permissions follow host desktop portals, and administrator authorization is handled by system services such as Polkit.",
    ));
    toolbar.append(&back);
    toolbar.append(&forward);
    toolbar.append(&page_title);
    toolbar.append(&help);

    content.append(&toolbar);
    content.append(&stack);

    let page_groups: [(&str, &[(&str, &str, &str, &str)]); 6] = [
        (
            "LOCAL USER",
            &[(
                "account",
                "User Profile",
                "Manage the local display name and profile photo.",
                "profile local user display name avatar photo",
            )],
        ),
        (
            "CONNECTIVITY",
            &[
                (
                    "wifi",
                    "Wi-Fi",
                    "Manage wireless connections and radio state.",
                    "wireless internet network dns known networks wifi wi-fi",
                ),
                (
                    "bluetooth",
                    "Bluetooth",
                    "Manage Bluetooth power and connected devices.",
                    "bluetooth paired headphones discoverable",
                ),
                (
                    "network",
                    "Network",
                    "Review network connections, VPN, and wired settings.",
                    "ethernet vpn proxy firewall network",
                ),
            ],
        ),
        (
            "PERSONALIZATION & WORKSPACE",
            &[
                (
                    "appearance",
                    "Appearance",
                    "Theme, accent color, and Liquid Glass appearance.",
                    "light dark theme accent transparency liquid glass tinted",
                ),
                (
                    "display",
                    "Displays",
                    "Configure resolution, refresh rate, and monitor layout.",
                    "screen resolution refresh rate monitor night shift brightness",
                ),
                (
                    "wallpaper",
                    "Wallpaper & Screen Saver",
                    "Choose a desktop background and screen saver.",
                    "wallpaper background aerial screensaver screen saver",
                ),
                (
                    "desktop_dock",
                    "Desktop & Dock",
                    "Configure the Dock, window tiling, and hot corners.",
                    "dock desktop windows stage manager tiling hot corners",
                ),
                (
                    "multitasking",
                    "Multitasking",
                    "Configure workspaces, tiling, and window behavior.",
                    "multitasking workspace floating window layout",
                ),
                (
                    "control_center",
                    "Control Center & Menu Bar",
                    "Choose which controls appear in the menu bar.",
                    "control center menu bar menubar icons modules",
                ),
            ],
        ),
        (
            "SYSTEM",
            &[
                (
                    "general",
                    "General",
                    "About this system, updates, storage, and startup items.",
                    "about update software storage login items date language reset",
                ),
                (
                    "sound",
                    "Sound",
                    "Choose audio devices and control output, balance, and microphone level.",
                    "audio sound speaker microphone volume balance",
                ),
                (
                    "notifications",
                    "Notifications & Focus",
                    "Mako notifications and notification preferences.",
                    "notifications focus do not disturb banners alerts",
                ),
                (
                    "battery",
                    "Battery",
                    "Review reported battery history and supported power modes.",
                    "battery power energy low power charging",
                ),
                (
                    "apps",
                    "Apps & Defaults",
                    "Browse applications and manage default apps.",
                    "apps applications defaults launcher installed software",
                ),
                (
                    "search",
                    "App Search",
                    "Search and launch installed desktop applications.",
                    "search apps launcher spotlight",
                ),
            ],
        ),
        (
            "PRIVACY & SECURITY",
            &[
                (
                    "privacy",
                    "Privacy & Security",
                    "Review privacy controls and system security.",
                    "privacy permissions camera microphone filevault firewall security",
                ),
                (
                    "lock_screen",
                    "Lock Screen",
                    "Configure screen locking and sleep behavior.",
                    "lock screen password sleep screen saver",
                ),
                (
                    "passwords",
                    "Passwords & Passkeys",
                    "Open the system credential manager.",
                    "passwords passkeys credentials vault keychain",
                ),
            ],
        ),
        (
            "DEVICES & ACCESSIBILITY",
            &[
                (
                    "keyboard",
                    "Keyboard",
                    "Review keyboard layout and MARCH SCARA shortcuts.",
                    "keyboard key repeat input sources shortcuts",
                ),
                (
                    "trackpad",
                    "Trackpad",
                    "Configure touchpad gestures and scrolling.",
                    "trackpad touchpad gestures natural scrolling tap click",
                ),
                (
                    "mouse",
                    "Mouse",
                    "Pointer speed, scrolling, and mouse controls.",
                    "mouse pointer cursor double click",
                ),
                (
                    "printers",
                    "Printers & Scanners",
                    "Review connected printers and scanners.",
                    "printer scanner print queue",
                ),
                (
                    "accessibility",
                    "Accessibility",
                    "Assistive features for vision, hearing, and motor access.",
                    "accessibility voiceover zoom captions voice control contrast",
                ),
            ],
        ),
    ];

    let active_buttons = Rc::new(RefCell::new(Vec::<Button>::new()));
    let search_groups = Rc::new(RefCell::new(Vec::<(GtkBox, Vec<(Button, String)>)>::new()));
    let page_names = Rc::new(
        page_groups
            .iter()
            .flat_map(|(_, pages)| pages.iter())
            .map(|(id, title, _, _)| ((*id).to_string(), (*title).to_string()))
            .collect::<std::collections::HashMap<_, _>>(),
    );
    let history = Rc::new(RefCell::new(vec!["account".to_string()]));
    let history_index = Rc::new(Cell::new(0_usize));

    for (_, pages) in &page_groups {
        for (id, title, description, _) in *pages {
            let page = settings_page(title, description);
            match *id {
                "wifi" => {
                    let card = settings_card("Wireless", "Toggle Wi-Fi using NetworkManager.");
                    let controls = GtkBox::new(Orientation::Horizontal, 10);
                    let radio_state = run_program_output("nmcli", &["radio", "wifi"]);
                    let is_enabled = radio_state.as_deref() == Ok("enabled");
                    let status_text = match &radio_state {
                        Ok(state) => format!("Wi-Fi radio: {state}"),
                        Err(error) => format!("NetworkManager unavailable: {error}"),
                    };
                    let status = Label::new(Some(&status_text));
                    status.add_css_class("cc-sub");
                    let toggle = Switch::new();
                    toggle.set_active(is_enabled);
                    toggle.set_sensitive(radio_state.is_ok());
                    let status_ref = status.clone();
                    let updating = Rc::new(Cell::new(false));
                    let updating_ref = updating.clone();
                    toggle.connect_active_notify(move |switch| {
                        if updating_ref.get() {
                            return;
                        }
                        let requested = switch.is_active();
                        let action = if requested { "on" } else { "off" };
                        match run_program_output("nmcli", &["radio", "wifi", action]) {
                            Ok(_) => status_ref.set_text(if requested {
                                "Wi-Fi radio: enabled"
                            } else {
                                "Wi-Fi radio: disabled"
                            }),
                            Err(error) => {
                                status_ref.set_text(&format!("Could not change Wi-Fi: {error}"));
                                updating_ref.set(true);
                                switch.set_active(!requested);
                                updating_ref.set(false);
                            }
                        }
                    });
                    controls.append(&status);
                    controls.append(&toggle);
                    card.append(&controls);
                    page.append(&card);
                    add_settings_tool(&page, "Open connection editor", "nm-connection-editor", &[]);

                    let profiles_card = settings_card(
                        "Saved Wi-Fi Networks",
                        "Connect to saved profiles or remove profiles from NetworkManager.",
                    );
                    let profiles_status = Label::new(None);
                    profiles_status.add_css_class("cc-sub");
                    profiles_status.set_halign(Align::Start);
                    profiles_card.append(&profiles_status);
                    let profiles_list = GtkBox::new(Orientation::Vertical, 6);
                    profiles_card.append(&profiles_list);
                    let refresh_profiles = Button::with_label("Refresh Saved Networks");
                    refresh_profiles.add_css_class("flat");
                    refresh_profiles.add_css_class("menu-action-btn");
                    let profiles_list_ref = profiles_list.clone();
                    let profiles_status_ref = profiles_status.clone();
                    refresh_profiles.connect_clicked(move |_| {
                        refresh_saved_wifi_profiles(&profiles_list_ref, &profiles_status_ref);
                    });
                    profiles_card.append(&refresh_profiles);
                    refresh_saved_wifi_profiles(&profiles_list, &profiles_status);
                    page.append(&profiles_card);
                }
                "bluetooth" => {
                    let card =
                        settings_card("Bluetooth radio", "Power the Bluetooth adapter on or off.");
                    let controls = GtkBox::new(Orientation::Horizontal, 10);
                    let powered = run_program_output("bluetoothctl", &["show"])
                        .and_then(|output| {
                            parse_bluetooth_powered(&output)
                                .ok_or_else(|| "BlueZ did not report adapter power state.".into())
                        });
                    let status_text = match &powered {
                        Ok(true) => "Bluetooth is on".to_string(),
                        Ok(false) => "Bluetooth is off".to_string(),
                        Err(error) => format!("Bluetooth unavailable: {error}"),
                    };
                    let status = Label::new(Some(&status_text));
                    status.add_css_class("cc-sub");
                    let toggle = Switch::new();
                    toggle.set_active(powered.as_ref() == Ok(&true));
                    toggle.set_sensitive(powered.is_ok());
                    let status_ref = status.clone();
                    let updating = Rc::new(Cell::new(false));
                    let updating_ref = updating.clone();
                    toggle.connect_active_notify(move |switch| {
                        if updating_ref.get() {
                            return;
                        }
                        let requested = switch.is_active();
                        let args = vec![
                            "power".to_string(),
                            if requested { "on" } else { "off" }.to_string(),
                        ];
                        let switch_ref = switch.clone();
                        let updating_after = updating_ref.clone();
                        let status_after = status_ref.clone();
                        run_system_command_async(
                            "bluetoothctl",
                            args,
                            status_ref.clone(),
                            move |success| {
                                if !success {
                                    updating_after.set(true);
                                    switch_ref.set_active(!requested);
                                    updating_after.set(false);
                                } else {
                                    status_after.set_text(if requested {
                                        "Bluetooth is on"
                                    } else {
                                        "Bluetooth is off"
                                    });
                                }
                            },
                        );
                    });
                    controls.append(&status);
                    controls.append(&toggle);
                    card.append(&controls);
                    page.append(&card);

                    let devices_card = settings_card(
                        "Bluetooth Devices",
                        "Paired devices and devices known to BlueZ. Scan to discover nearby devices.",
                    );
                    let devices_status = Label::new(None);
                    devices_status.add_css_class("cc-sub");
                    devices_status.set_halign(Align::Start);
                    devices_card.append(&devices_status);
                    let devices_list = GtkBox::new(Orientation::Vertical, 6);
                    devices_card.append(&devices_list);

                    let scan_button = Button::with_label("Scan for Devices");
                    scan_button.add_css_class("flat");
                    scan_button.add_css_class("menu-action-btn");
                    let scan_status = devices_status.clone();
                    let scan_list = devices_list.clone();
                    scan_button.connect_clicked(move |_| {
                        let status_after = scan_status.clone();
                        let list_after = scan_list.clone();
                        run_system_command_async(
                            "bluetoothctl",
                            vec![
                                "--timeout".to_string(),
                                "12".to_string(),
                                "scan".to_string(),
                                "on".to_string(),
                            ],
                            scan_status.clone(),
                            move |success| {
                                if success {
                                    refresh_bluetooth_devices(&list_after, &status_after);
                                }
                            },
                        );
                        scan_status.set_text("Scanning for nearby devices...");
                    });
                    devices_card.append(&scan_button);
                    let manager_button = Button::with_label("Open Bluetooth Manager");
                    manager_button.add_css_class("flat");
                    manager_button.add_css_class("menu-action-btn");
                    let manager_status = devices_status.clone();
                    manager_button.connect_clicked(move |_| {
                        match launch_program("blueman-manager", &[]) {
                            Ok(()) => manager_status.set_text(
                                "Opened Blueman to confirm pairing and device authorization.",
                            ),
                            Err(error) => manager_status
                                .set_text(&format!("Bluetooth manager unavailable: {error}")),
                        }
                    });
                    devices_card.append(&manager_button);
                    refresh_bluetooth_devices(&devices_list, &devices_status);
                    page.append(&devices_card);
                }
                "sound" => append_sound_settings(&page),
                "battery" => append_battery_settings(&page),
                "general" => {
                    let hostname = std::fs::read_to_string("/etc/hostname")
                        .map(|name| name.trim().to_string())
                        .ok()
                        .filter(|name| !name.is_empty())
                        .unwrap_or_else(|| "Not reported by the system".to_string());
                    page.append(&settings_card(
                        "About This Device",
                        &format!(
                            "{} · {hostname}\nProcessor: {}\nMemory: {}\nHardware serial: Not exposed by the user session",
                            os_display_name(),
                            cpu_display_name(),
                            memory_display_size()
                        ),
                    ));
                    let clock_card = settings_card(
                        "Date & Time",
                        "Choose how the MARCH SCARA menu bar displays the local time.",
                    );
                    let clock_24_hour = Switch::new();
                    clock_24_hour.set_active(preferences.borrow().clock_24_hour);
                    clock_card.append(&settings_control_row(
                        "24-hour clock",
                        &clock_24_hour,
                    ));
                    let clock_status = Label::new(Some("Uses the system's current local time zone."));
                    clock_status.add_css_class("cc-sub");
                    clock_status.set_halign(Align::Start);
                    clock_card.append(&clock_status);
                    let clock_preferences = preferences.clone();
                    let clock_window = window.clone();
                    let clock_widgets = menu_bar_widgets.clone();
                    let clock_status_ref = clock_status.clone();
                    clock_24_hour.connect_active_notify(move |switch| {
                        let use_24_hour = switch.is_active();
                        if persist_settings_change(
                            &clock_preferences,
                            &clock_window,
                            &clock_status_ref,
                            |settings| settings.clock_24_hour = use_24_hour,
                        ) {
                            update_clock_widget(&clock_widgets, &clock_preferences.borrow());
                        }
                    });
                    page.append(&clock_card);
                    match root_storage_display() {
                        Ok((summary, used_fraction)) => {
                            let storage_card = settings_card("Storage", &summary);
                            let usage_bar = ProgressBar::new();
                            usage_bar.set_fraction(used_fraction);
                            usage_bar.set_show_text(true);
                            usage_bar.set_text(Some(&format!(
                                "{:.0}% of the root filesystem is used",
                                used_fraction * 100.0
                            )));
                            storage_card.append(&usage_bar);
                            page.append(&storage_card);
                        }
                        Err(error) => add_settings_integration_note(
                            &page,
                            "Storage",
                            &format!("Usage data is unavailable: {error}"),
                        ),
                    }
                    append_home_storage_categories(&page);
                    add_settings_tool(
                        &page,
                        "Open system information",
                        "gnome-control-center",
                        &["about"],
                    );
                    add_settings_tool(
                        &page,
                        "Open software updates",
                        "gnome-software",
                        &[],
                    );
                    add_settings_tool(
                        &page,
                        "Open Date & Time settings",
                        "gnome-control-center",
                        &["datetime"],
                    );
                    add_settings_tool(
                        &page,
                        "Open Language & Region settings",
                        "gnome-control-center",
                        &["region"],
                    );
                    add_settings_integration_note(
                        &page,
                        "Updates, storage, and startup items",
                        "The storage view reports the root filesystem and standard home-folder sizes. Software updates and time/region preferences open host settings when installed. MARCH SCARA does not yet manage login items or categorize all application/system data.",
                    );
                }
                "keyboard" => {
                    let shortcuts = settings_card(
                        "MARCH SCARA shortcuts",
                        "Super+Return: terminal · Super+E: files · Super+Q: close · Super+1–5: workspaces · Super+Shift+1–5: move window · Super alone: app launcher.",
                    );
                    let shortcut_status = Label::new(Some(
                        "Edit the user init.lua to customize bindings, then reload them here.",
                    ));
                    shortcut_status.add_css_class("cc-sub");
                    shortcut_status.set_halign(Align::Start);
                    shortcuts.append(&shortcut_status);
                    let actions = GtkBox::new(Orientation::Horizontal, 8);
                    let edit_shortcuts = Button::with_label("Edit shortcuts");
                    edit_shortcuts.add_css_class("flat");
                    edit_shortcuts.add_css_class("menu-action-btn");
                    let shortcut_status_ref = shortcut_status.clone();
                    edit_shortcuts.connect_clicked(move |_| {
                        match open_user_config(
                            "march-scara",
                            "init.lua",
                            include_str!("../../../data/init.lua"),
                        ) {
                            Ok(path) => shortcut_status_ref
                                .set_text(&format!("Editing {}.", path.display())),
                            Err(error) => shortcut_status_ref.set_text(&error),
                        }
                    });
                    actions.append(&edit_shortcuts);
                    let reload_shortcuts = Button::with_label("Reload shortcuts");
                    reload_shortcuts.add_css_class("flat");
                    reload_shortcuts.add_css_class("menu-action-btn");
                    let shortcut_status_ref = shortcut_status.clone();
                    reload_shortcuts.connect_clicked(move |_| {
                        match run_wm_ctl_output("reload") {
                            Ok(response) => shortcut_status_ref.set_text(&response),
                            Err(error) => shortcut_status_ref
                                .set_text(&format!("Could not reload shortcuts: {error}")),
                        }
                    });
                    actions.append(&reload_shortcuts);
                    shortcuts.append(&actions);
                    page.append(&shortcuts);
                    add_settings_integration_note(
                        &page,
                        "Keyboard input",
                        "Key repeat and input-source changes are not currently exposed by the compositor. The host keyboard panel is available when installed.",
                    );
                    add_settings_tool(&page, "Open host keyboard settings", "gnome-control-center", &["keyboard"]);
                }
                "mouse" => {
                    let pointer_summary = input_device_summary("pointer")
                        .unwrap_or_else(|error| format!("Could not enumerate libinput devices: {error}"));
                    page.append(&settings_card("Libinput pointer devices", &pointer_summary));
                    add_settings_integration_note(
                        &page,
                        "Pointer settings",
                        "This panel reports devices exposed by libinput. Per-device acceleration and button mappings are not yet configurable by MARCH SCARA.",
                    );
                    add_settings_tool(
                        &page,
                        "Open host mouse settings",
                        "gnome-control-center",
                        &["mouse"],
                    );
                }
                "display" => append_display_output_cards(&page),
                "appearance" => {
                    let card = settings_card(
                        "Personalize your desktop",
                        "These preferences are saved for your user and applied immediately to MARCH SCARA Settings.",
                    );
                    let theme = DropDown::from_strings(&["Automatic", "Light", "Dark"]);
                    theme.set_selected(match preferences.borrow().theme {
                        ThemeMode::Auto => 0,
                        ThemeMode::Light => 1,
                        ThemeMode::Dark => 2,
                    });
                    card.append(&settings_control_row("Appearance", &theme));

                    let accent = DropDown::from_strings(&[
                        "Blue", "Purple", "Pink", "Red", "Orange", "Yellow", "Green", "Graphite",
                    ]);
                    accent.set_selected(match preferences.borrow().accent {
                        AccentColor::Blue => 0,
                        AccentColor::Purple => 1,
                        AccentColor::Pink => 2,
                        AccentColor::Red => 3,
                        AccentColor::Orange => 4,
                        AccentColor::Yellow => 5,
                        AccentColor::Green => 6,
                        AccentColor::Graphite => 7,
                    });
                    card.append(&settings_control_row("Accent color", &accent));

                    let glass = DropDown::from_strings(&["Standard Glass", "Clear", "Tinted"]);
                    glass.set_selected(match preferences.borrow().glass {
                        GlassStyle::Standard => 0,
                        GlassStyle::Clear => 1,
                        GlassStyle::Tinted => 2,
                    });
                    card.append(&settings_control_row("Glass material", &glass));

                    let reduce_transparency = Switch::new();
                    reduce_transparency
                        .set_active(preferences.borrow().reduce_transparency);
                    card.append(&settings_control_row(
                        "Reduce transparency",
                        &reduce_transparency,
                    ));

                    let status = Label::new(Some(
                        preferences_error
                            .as_deref()
                            .unwrap_or("Changes apply immediately."),
                    ));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);
                    status.set_wrap(true);
                    card.append(&status);
                    page.append(&card);

                    let preferences_ref = preferences.clone();
                    let window_ref = window.clone();
                    let status_ref = status.clone();
                    theme.connect_selected_notify(move |dropdown| {
                        let value = match dropdown.selected() {
                            0 => ThemeMode::Auto,
                            1 => ThemeMode::Light,
                            2 => ThemeMode::Dark,
                            _ => return,
                        };
                        persist_settings_change(
                            &preferences_ref,
                            &window_ref,
                            &status_ref,
                            |settings| settings.theme = value,
                        );
                    });

                    let preferences_ref = preferences.clone();
                    let window_ref = window.clone();
                    let status_ref = status.clone();
                    accent.connect_selected_notify(move |dropdown| {
                        let value = match dropdown.selected() {
                            0 => AccentColor::Blue,
                            1 => AccentColor::Purple,
                            2 => AccentColor::Pink,
                            3 => AccentColor::Red,
                            4 => AccentColor::Orange,
                            5 => AccentColor::Yellow,
                            6 => AccentColor::Green,
                            7 => AccentColor::Graphite,
                            _ => return,
                        };
                        persist_settings_change(
                            &preferences_ref,
                            &window_ref,
                            &status_ref,
                            |settings| settings.accent = value,
                        );
                    });

                    let preferences_ref = preferences.clone();
                    let window_ref = window.clone();
                    let status_ref = status.clone();
                    glass.connect_selected_notify(move |dropdown| {
                        let value = match dropdown.selected() {
                            0 => GlassStyle::Standard,
                            1 => GlassStyle::Clear,
                            2 => GlassStyle::Tinted,
                            _ => return,
                        };
                        persist_settings_change(
                            &preferences_ref,
                            &window_ref,
                            &status_ref,
                            |settings| settings.glass = value,
                        );
                    });

                    let preferences_ref = preferences.clone();
                    let window_ref = window.clone();
                    let status_ref = status.clone();
                    reduce_transparency.connect_active_notify(move |switch| {
                        let value = switch.is_active();
                        if persist_settings_change(
                            &preferences_ref,
                            &window_ref,
                            &status_ref,
                            |settings| settings.reduce_transparency = value,
                        ) {
                            match set_compositor_reduce_transparency(value) {
                                Ok(()) => status_ref.set_text(
                                    "Saved and applied to the compositor immediately.",
                                ),
                                Err(error) => status_ref.set_text(&format!(
                                    "Preference saved, but compositor update failed: {error}"
                                )),
                            }
                        }
                    });
                    add_settings_tool(
                        &page,
                        "Open appearance settings",
                        "gnome-control-center",
                        &["appearance"],
                    );
                }
                "wallpaper" => {
                    let current_wallpaper = find_wallpaper_path()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "Built-in MARCH SCARA background".to_string());
                    let card = settings_card(
                        "Desktop wallpaper",
                        &format!("Current image: {current_wallpaper}"),
                    );
                    let choose_wallpaper = Button::with_label("Choose Image…");
                    choose_wallpaper.add_css_class("flat");
                    choose_wallpaper.add_css_class("menu-action-btn");
                    let status = Label::new(Some(
                        "Select a JPG, PNG, or WebP image to apply it to the desktop.",
                    ));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);
                    status.set_wrap(true);

                    let settings_window = window.clone();
                    let chooser_wallpaper_picture = wallpaper_picture.clone();
                    let preferences_ref = preferences.clone();
                    let status_ref = status.clone();
                    choose_wallpaper.connect_clicked(move |_| {
                        let chooser = FileChooserNative::new(
                            Some("Choose Desktop Wallpaper"),
                            Some(&settings_window),
                            FileChooserAction::Open,
                            Some("Select"),
                            Some("Cancel"),
                        );
                        let picture = chooser_wallpaper_picture.clone();
                        let preferences = preferences_ref.clone();
                        let status = status_ref.clone();
                        chooser.connect_response(move |dialog, response| {
                            if response == ResponseType::Accept {
                                let selected_path = dialog.file().and_then(|file| file.path());
                                match selected_path {
                                    Some(path) => match preferences::validate_wallpaper(&path) {
                                        Ok(()) => {
                                            let saved_path = path.clone();
                                            match update_preferences(&preferences, |settings| {
                                                settings.wallpaper = Some(saved_path);
                                            }) {
                                                Ok(()) => {
                                                    if let Some(filename) = path.to_str() {
                                                        picture.set_filename(Some(filename));
                                                        status.set_text("Wallpaper updated and saved.");
                                                    } else {
                                                        status.set_text(
                                                            "The selected file path is not valid UTF-8.",
                                                        );
                                                    }
                                                }
                                                Err(error) => status.set_text(&format!(
                                                    "Could not save wallpaper setting: {error}"
                                                )),
                                            }
                                        }
                                        Err(error) => {
                                            status.set_text(&format!("Cannot use image: {error}"))
                                        }
                                    },
                                    None => status.set_text(
                                        "Could not read the selected image path.",
                                    ),
                                }
                            }
                            dialog.destroy();
                        });
                        chooser.show();
                    });
                    card.append(&choose_wallpaper);
                    card.append(&status);
                    page.append(&card);
                    match discover_wallpapers() {
                        Ok(wallpapers) if !wallpapers.is_empty() => {
                            let labels = wallpapers
                                .iter()
                                .map(|path| {
                                    let directory = path
                                        .parent()
                                        .and_then(Path::file_name)
                                        .map(|name| name.to_string_lossy())
                                        .unwrap_or_default();
                                    let file = path
                                        .file_name()
                                        .map(|name| name.to_string_lossy())
                                        .unwrap_or_default();
                                    format!("{directory} · {file}")
                                })
                                .collect::<Vec<_>>();
                            let gallery = settings_card(
                                "Wallpaper gallery",
                                "Images found in /usr/share/backgrounds and ~/Pictures/Wallpapers.",
                            );
                            let selection = DropDown::from_strings(
                                &labels.iter().map(String::as_str).collect::<Vec<_>>(),
                            );
                            gallery.append(&settings_control_row("Available image", &selection));
                            let gallery_status = Label::new(Some(""));
                            gallery_status.add_css_class("cc-sub");
                            gallery_status.set_halign(Align::Start);
                            let apply = Button::with_label("Apply selected wallpaper");
                            apply.add_css_class("flat");
                            apply.add_css_class("menu-action-btn");
                            let gallery_paths = wallpapers;
                            let selected_wallpaper = selection.clone();
                            let gallery_picture = wallpaper_picture.clone();
                            let gallery_preferences = preferences.clone();
                            let gallery_status_ref = gallery_status.clone();
                            apply.connect_clicked(move |_| {
                                let Some(path) =
                                    gallery_paths.get(selected_wallpaper.selected() as usize)
                                else {
                                    gallery_status_ref.set_text("Select a wallpaper first.");
                                    return;
                                };
                                let Some(filename) = path.to_str() else {
                                    gallery_status_ref
                                        .set_text("The selected file path is not valid UTF-8.");
                                    return;
                                };
                                match update_preferences(&gallery_preferences, |settings| {
                                    settings.wallpaper = Some(path.clone());
                                }) {
                                    Ok(()) => {
                                        gallery_picture.set_filename(Some(filename));
                                        gallery_status_ref.set_text("Wallpaper updated and saved.");
                                    }
                                    Err(error) => gallery_status_ref
                                        .set_text(&format!("Could not save wallpaper setting: {error}")),
                                }
                            });
                            gallery.append(&apply);
                            gallery.append(&gallery_status);
                            page.append(&gallery);
                        }
                        Ok(_) => add_settings_integration_note(
                            &page,
                            "Wallpaper gallery",
                            "No system or user wallpaper images were found. You can still choose an image file above.",
                        ),
                        Err(error) => add_settings_integration_note(
                            &page,
                            "Wallpaper gallery unavailable",
                            &error,
                        ),
                    }
                    add_settings_integration_note(
                        &page,
                        "Screen saver",
                        "Screen-saver scheduling and lock-screen clock options are not yet managed here.",
                    );
                }
                "network" => add_settings_tool(
                    &page,
                    "Open network connections",
                    "nm-connection-editor",
                    &[],
                ),
                "notifications" => {
                    page.append(&settings_card(
                        "Notifications",
                        "Mako is started with the MARCH SCARA session and receives standard Wayland desktop notifications.",
                    ));
                    let focus_card = settings_card(
                        "Focus",
                        "Toggle Mako's Do Not Disturb mode for this session.",
                    );
                    let focus_status_text = match mako_focus_status() {
                        Ok(true) => "Do Not Disturb is active.".to_string(),
                        Ok(false) => "Do Not Disturb is inactive.".to_string(),
                        Err(error) => format!("Mako control is unavailable: {error}"),
                    };
                    let focus_status = Label::new(Some(&focus_status_text));
                    focus_status.add_css_class("cc-sub");
                    focus_status.set_halign(Align::Start);
                    focus_card.append(&focus_status);
                    let focus_button = Button::with_label("Toggle Do Not Disturb");
                    focus_button.add_css_class("flat");
                    focus_button.add_css_class("menu-action-btn");
                    let focus_status_ref = focus_status.clone();
                    focus_button.connect_clicked(move |_| {
                        match run_program_output(
                            "makoctl",
                            &["mode", "-t", "do-not-disturb"],
                        ) {
                            Ok(_) => match mako_focus_status() {
                                Ok(true) => focus_status_ref.set_text("Do Not Disturb is active."),
                                Ok(false) => {
                                    focus_status_ref.set_text("Do Not Disturb is inactive.")
                                }
                                Err(error) => focus_status_ref.set_text(&format!(
                                    "Mode changed, but Mako status could not be read: {error}"
                                )),
                            },
                            Err(error) => focus_status_ref
                                .set_text(&format!("Could not toggle Focus mode: {error}")),
                        }
                    });
                    focus_card.append(&focus_button);
                    page.append(&focus_card);

                    let rules_card = settings_card(
                        "Per-application notification rules",
                        "Edit Mako's user configuration to persist app-specific notification rules. Reload after saving.",
                    );
                    let rules_status = Label::new(Some(
                        "Rules are stored in the user's Mako configuration and owned by Mako.",
                    ));
                    rules_status.add_css_class("cc-sub");
                    rules_status.set_halign(Align::Start);
                    rules_status.set_wrap(true);
                    rules_card.append(&rules_status);
                    let rules_actions = GtkBox::new(Orientation::Horizontal, 8);
                    let edit_rules = Button::with_label("Edit notification rules");
                    edit_rules.add_css_class("flat");
                    edit_rules.add_css_class("menu-action-btn");
                    let rules_status_ref = rules_status.clone();
                    edit_rules.connect_clicked(move |_| {
                        match open_user_config(
                            "mako",
                            "config",
                            "# Per-application Mako rules can be added here.\n",
                        ) {
                            Ok(path) => {
                                rules_status_ref.set_text(&format!("Editing {}.", path.display()))
                            }
                            Err(error) => rules_status_ref.set_text(&error),
                        }
                    });
                    rules_actions.append(&edit_rules);
                    let reload_rules = Button::with_label("Reload rules");
                    reload_rules.add_css_class("flat");
                    reload_rules.add_css_class("menu-action-btn");
                    let rules_status_ref = rules_status.clone();
                    reload_rules.connect_clicked(move |_| {
                        match run_program_output("makoctl", &["reload"]) {
                            Ok(_) => rules_status_ref.set_text("Mako notification rules reloaded."),
                            Err(error) => rules_status_ref
                                .set_text(&format!("Could not reload Mako rules: {error}")),
                        }
                    });
                    rules_actions.append(&reload_rules);
                    rules_card.append(&rules_actions);
                    page.append(&rules_card);
                }
                "accessibility" => {
                    let card = settings_card(
                        "Display",
                        "Reduce transparency is applied to MARCH SCARA's Liquid Glass surfaces.",
                    );
                    let reduce_transparency = Switch::new();
                    reduce_transparency.set_active(preferences.borrow().reduce_transparency);
                    card.append(&settings_control_row(
                        "Reduce transparency",
                        &reduce_transparency,
                    ));
                    let status = Label::new(Some("Changes apply immediately."));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);
                    card.append(&status);
                    let preferences_ref = preferences.clone();
                    let window_ref = window.clone();
                    let status_ref = status.clone();
                    reduce_transparency.connect_active_notify(move |switch| {
                        let value = switch.is_active();
                        if persist_settings_change(
                            &preferences_ref,
                            &window_ref,
                            &status_ref,
                            |settings| settings.reduce_transparency = value,
                        ) {
                            match set_compositor_reduce_transparency(value) {
                                Ok(()) => status_ref.set_text(
                                    "Saved and applied to the compositor immediately.",
                                ),
                                Err(error) => status_ref.set_text(&format!(
                                    "Preference saved, but compositor update failed: {error}"
                                )),
                            }
                        }
                    });
                    page.append(&card);
                    add_settings_tool(
                        &page,
                        "Open accessibility settings",
                        "gnome-control-center",
                        &["universal-access"],
                    );
                }
                "privacy" => {
                    page.append(&settings_card(
                        "Application permissions",
                        "File and wallpaper/avatar selection use GTK native choosers with GTK_USE_PORTAL enabled. Supported app access prompts and remembered permissions are handled by XDG Desktop Portal; MARCH SCARA does not keep a parallel permission database or edit another app's permissions.",
                    ));
                    page.append(&settings_card(
                        "Administrator actions",
                        "NetworkManager, BlueZ, power-profile, and other host services perform privileged changes. Polkit handles authorization; MARCH SCARA never requests or stores administrator passwords.",
                    ));
                    append_host_integration_diagnostics(&page);
                    add_settings_tool(
                        &page,
                        "Open GNOME privacy settings",
                        "gnome-control-center",
                        &["privacy"],
                    );
                }
                "apps" | "search" => {
                    let card = settings_card(
                        "Installed applications",
                        "Use the MARCH SCARA application grid to search and launch desktop apps.",
                    );
                    let button = Button::with_label("Open application grid");
                    button.add_css_class("flat");
                    button.add_css_class("menu-action-btn");
                    let app = app.clone();
                    button.connect_clicked(move |_| {
                        if let Some(window) = app
                            .windows()
                            .into_iter()
                            .find(|window| window.title().as_deref() == Some("Applications"))
                        {
                            window.present();
                        }
                    });
                    card.append(&button);
                    page.append(&card);
                    add_settings_tool(
                        &page,
                        "Default application settings",
                        "gnome-control-center",
                        &["applications"],
                    );
                }
                "account" => {
                    let card = settings_card(
                        "Local Profile",
                        "The display name and avatar are stored in this user account.",
                    );
                    let display_name = Entry::new();
                    display_name.set_placeholder_text(Some("Display name"));
                    display_name.set_text(profile_name.label().as_str());
                    display_name.set_hexpand(true);
                    card.append(&settings_control_row("Display name", &display_name));

                    let choose_avatar = Button::with_label("Choose Profile Photo…");
                    choose_avatar.add_css_class("flat");
                    choose_avatar.add_css_class("menu-action-btn");
                    card.append(&choose_avatar);

                    let status = Label::new(Some(
                        "Press Enter in the name field to apply the display name.",
                    ));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);
                    status.set_wrap(true);
                    card.append(&status);
                    page.append(&card);

                    let preferences_ref = preferences.clone();
                    let status_ref = status.clone();
                    let profile_name_ref = profile_name.clone();
                    display_name.connect_activate(move |entry| {
                        let name = entry.text().trim().to_string();
                        if name.is_empty() {
                            status_ref.set_text("Display name cannot be empty.");
                            return;
                        }
                        match update_preferences(&preferences_ref, |settings| {
                            settings.display_name = Some(name.clone());
                        }) {
                            Ok(()) => {
                                profile_name_ref.set_text(&name);
                                status_ref.set_text("Display name updated.");
                            }
                            Err(error) => {
                                status_ref.set_text(&format!("Could not save profile: {error}"))
                            }
                        }
                    });

                    let preferences_ref = preferences.clone();
                    let status_ref = status.clone();
                    let avatar_ref = avatar.clone();
                    let settings_window = window.clone();
                    choose_avatar.connect_clicked(move |_| {
                        let chooser = FileChooserNative::new(
                            Some("Choose Profile Photo"),
                            Some(&settings_window),
                            FileChooserAction::Open,
                            Some("Select"),
                            Some("Cancel"),
                        );
                        let preferences = preferences_ref.clone();
                        let status = status_ref.clone();
                        let avatar = avatar_ref.clone();
                        chooser.connect_response(move |dialog, response| {
                            if response == ResponseType::Accept {
                                let path = dialog.file().and_then(|file| file.path());
                                match path {
                                    Some(path) => match preferences::validate_wallpaper(&path) {
                                        Ok(()) => {
                                            let saved_path = path.clone();
                                            match update_preferences(&preferences, |settings| {
                                                settings.avatar = Some(saved_path);
                                            }) {
                                                Ok(()) => {
                                                    if let Some(filename) = path.to_str() {
                                                        avatar.set_from_file(Some(filename));
                                                        status.set_text("Profile photo updated.");
                                                    } else {
                                                        status.set_text(
                                                            "The selected file path is not valid UTF-8.",
                                                        );
                                                    }
                                                }
                                                Err(error) => status.set_text(&format!(
                                                    "Could not save profile photo: {error}"
                                                )),
                                            }
                                        }
                                        Err(error) => {
                                            status.set_text(&format!("Cannot use image: {error}"))
                                        }
                                    },
                                    None => status.set_text(
                                        "Could not read the selected image path.",
                                    ),
                                }
                            }
                            dialog.destroy();
                        });
                        chooser.show();
                    });

                    page.append(&settings_card(
                        "Sign-in and password",
                        "Linux account authentication is managed by the host operating system. MARCH SCARA never stores account passwords.",
                    ));
                    add_settings_tool(
                        &page,
                        "Open GNOME user settings",
                        "gnome-control-center",
                        &["users"],
                    );
                }
                "desktop_dock" => {
                    page.append(&settings_card(
                        "Workspaces and windows",
                        "Use Super+1–5 to switch workspaces, Super+Shift+1–5 to move a window, Super+F to toggle floating, and Super+T to toggle layout.",
                    ));
                    add_settings_integration_note(
                        &page,
                        "Dock & window behavior",
                        "Dock placement, auto-hide, magnification, hot corners, and Stage Manager controls are not yet configurable here.",
                    );
                }
                "control_center" => {
                    let card = settings_card(
                        "Menu bar modules",
                        "Choose which MARCH SCARA controls remain visible. Changes apply immediately.",
                    );
                    let status = Label::new(Some(
                        "Preferences are stored for your local user account.",
                    ));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);

                    for (title, module) in [
                        ("Bluetooth", "bluetooth"),
                        ("Battery", "battery"),
                        ("Wi-Fi", "wifi"),
                        ("Spotlight", "spotlight"),
                        ("Control Center", "control_center"),
                        ("Clock", "clock"),
                        ("Sound in Control Center", "sound"),
                    ] {
                        let toggle = Switch::new();
                        toggle.set_active(menu_bar_module_visible(
                            &preferences.borrow().menu_bar,
                            module,
                        ));
                        card.append(&settings_control_row(title, &toggle));

                        let preferences_ref = preferences.clone();
                        let window_ref = window.clone();
                        let status_ref = status.clone();
                        let widgets_ref = menu_bar_widgets.clone();
                        let module = module.to_string();
                        toggle.connect_active_notify(move |switch| {
                            let visible = switch.is_active();
                            if persist_settings_change(
                                &preferences_ref,
                                &window_ref,
                                &status_ref,
                                |settings| {
                                    update_menu_bar_module(settings, &module, visible);
                                },
                            ) {
                                set_menu_bar_module_visibility(
                                    &widgets_ref,
                                    &preferences_ref.borrow(),
                                );
                            }
                        });
                    }
                    card.append(&status);
                    page.append(&card);
                }
                "lock_screen" => add_settings_integration_note(
                    &page,
                    "Lock screen",
                    "Lock-screen messages, idle timers, and sleep authentication are managed by the host system, not MARCH SCARA.",
                ),
                "passwords" => {
                    let secret_service_status = secret_service_status(|| {
                        dbus_interface_available(
                            "--user",
                            "org.freedesktop.secrets",
                            "/org/freedesktop/secrets",
                            "org.freedesktop.Secret.Service",
                        )
                    });
                    page.append(&settings_card(
                        "System credential service",
                        &secret_service_status,
                    ));
                    add_settings_tool(&page, "Open password manager", "seahorse", &[]);
                }
                "trackpad" => {
                    let touchpad_summary = input_device_summary("touchpad")
                        .unwrap_or_else(|error| format!("Could not enumerate libinput devices: {error}"));
                    page.append(&settings_card("Libinput touchpad devices", &touchpad_summary));
                    add_settings_integration_note(
                        &page,
                        "Trackpad settings",
                        "The compositor enables tap-to-click on supported devices. Natural scrolling, pointer speed, and gesture configuration are not yet configurable here.",
                    );
                    add_settings_tool(
                        &page,
                        "Open host touchpad settings",
                        "gnome-control-center",
                        &["mouse"],
                    );
                }
                "printers" => {
                    let status = Label::new(Some("Checking CUPS printers and queues…"));
                    status.add_css_class("cc-sub");
                    status.set_halign(Align::Start);
                    status.set_wrap(true);
                    let card = settings_card(
                        "CUPS printers and queues",
                        "Read-only status from the host print service. Adding or removing printers remains a host-authorized action.",
                    );
                    card.append(&status);
                    let refresh = Button::with_label("Refresh printers and queue");
                    refresh.add_css_class("flat");
                    refresh.add_css_class("menu-action-btn");
                    let status_ref = status.clone();
                    refresh.connect_clicked(move |_| {
                        refresh_printer_status(&status_ref);
                    });
                    card.append(&refresh);
                    page.append(&card);
                    refresh_printer_status(&status);
                    add_settings_tool(
                        &page,
                        "Open printer settings",
                        "system-config-printer",
                        &[],
                    );
                }
                "multitasking" => page.append(&settings_card(
                    "Workspaces and windows",
                    "Use Super+1–5 to switch workspaces, Super+Shift+1–5 to move a window, Super+F to toggle floating, and Super+T to toggle layout.",
                )),
                _ => add_settings_integration_note(
                    &page,
                    "Integration status",
                    "This panel does not yet have a MARCH SCARA system integration.",
                ),
            }

            let scrolled_page = ScrolledWindow::new();
            scrolled_page.set_policy(gtk::PolicyType::Never, gtk::PolicyType::Automatic);
            scrolled_page.set_child(Some(&page));
            stack.add_titled(&scrolled_page, Some(id), title);
        }
    }

    for (group_title, pages) in &page_groups {
        let group = GtkBox::new(Orientation::Vertical, 2);
        let heading = Label::new(Some(group_title));
        heading.add_css_class("settings-section-title");
        heading.set_halign(Align::Start);
        group.append(&heading);

        let mut rows = Vec::new();
        for (id, title, _, keywords) in *pages {
            let button = Button::with_label(title);
            button.add_css_class("settings-nav-button");
            let stack = stack.clone();
            let page_title = page_title.clone();
            let history = history.clone();
            let history_index = history_index.clone();
            let buttons_for_selection = active_buttons.clone();
            let back = back.clone();
            let forward = forward.clone();
            let id = (*id).to_string();
            let title = (*title).to_string();
            let searchable_text = format!(
                "{} {} {}",
                title.to_lowercase(),
                keywords.to_lowercase(),
                id
            );
            button.connect_clicked(move |selected| {
                let mut entries = history.borrow_mut();
                let index = history_index.get();
                entries.truncate(index + 1);
                if entries.last() != Some(&id) {
                    entries.push(id.clone());
                }
                let new_index = entries.len().saturating_sub(1);
                history_index.set(new_index);
                stack.set_visible_child_name(&id);
                page_title.set_text(&title);
                back.set_sensitive(new_index > 0);
                forward.set_sensitive(false);
                for item in buttons_for_selection.borrow().iter() {
                    item.remove_css_class("selected");
                }
                selected.add_css_class("selected");
            });
            rows.push((button.clone(), searchable_text));
            active_buttons.borrow_mut().push(button.clone());
            group.append(&button);
        }
        nav_content.append(&group);
        search_groups.borrow_mut().push((group, rows));
    }

    if let Some(first) = active_buttons.borrow().first() {
        first.add_css_class("selected");
    }

    let search_groups_ref = search_groups.clone();
    let no_results_ref = no_results.clone();
    search.connect_search_changed(move |entry| {
        let query = entry.text().trim().to_lowercase();
        let mut any_group_visible = false;
        for (group, rows) in search_groups_ref.borrow().iter() {
            let mut group_visible = false;
            for (button, searchable_text) in rows {
                let is_match = settings_search_matches(&query, searchable_text);
                button.set_visible(is_match);
                group_visible |= is_match;
            }
            group.set_visible(group_visible);
            any_group_visible |= group_visible;
        }
        no_results_ref.set_visible(!query.is_empty() && !any_group_visible);
    });

    let history_ref = history.clone();
    let history_index_ref = history_index.clone();
    let stack_ref = stack.clone();
    let title_ref = page_title.clone();
    let back_ref = back.clone();
    let forward_ref = forward.clone();
    let active_buttons_ref = active_buttons.clone();
    let page_names_ref = page_names.clone();
    back.connect_clicked(move |_| {
        let current = history_index_ref.get();
        if current == 0 {
            return;
        }
        let next = current - 1;
        history_index_ref.set(next);
        if let Some(id) = history_ref.borrow().get(next) {
            stack_ref.set_visible_child_name(id);
            if let Some(title) = page_names_ref.get(id) {
                title_ref.set_text(title);
            }
            for button in active_buttons_ref.borrow().iter() {
                button.remove_css_class("selected");
                if button.label().as_deref() == page_names_ref.get(id).map(String::as_str) {
                    button.add_css_class("selected");
                }
            }
        }
        back_ref.set_sensitive(next > 0);
        forward_ref.set_sensitive(true);
    });

    let history_ref = history.clone();
    let history_index_ref = history_index.clone();
    let stack_ref = stack.clone();
    let title_ref = page_title.clone();
    let back_ref = back.clone();
    let forward_ref = forward.clone();
    let active_buttons_ref = active_buttons.clone();
    let page_names_ref = page_names.clone();
    forward.connect_clicked(move |_| {
        let next = history_index_ref.get() + 1;
        let entries = history_ref.borrow();
        if next >= entries.len() {
            return;
        }
        history_index_ref.set(next);
        if let Some(id) = entries.get(next) {
            stack_ref.set_visible_child_name(id);
            if let Some(title) = page_names_ref.get(id) {
                title_ref.set_text(title);
            }
            for button in active_buttons_ref.borrow().iter() {
                button.remove_css_class("selected");
                if button.label().as_deref() == page_names_ref.get(id).map(String::as_str) {
                    button.add_css_class("selected");
                }
            }
        }
        back_ref.set_sensitive(true);
        forward_ref.set_sensitive(next + 1 < entries.len());
    });

    for button in [back.clone(), forward.clone()] {
        button.add_css_class("flat");
    }
    layout.append(&sidebar);
    layout.append(&content);
    window.set_child(Some(&layout));

    let navigator = {
        let window = window.clone();
        let stack = stack.clone();
        let search = search.clone();
        let page_title = page_title.clone();
        let page_names = page_names.clone();
        let history = history.clone();
        let history_index = history_index.clone();
        let back = back.clone();
        let forward = forward.clone();
        let active_buttons = active_buttons.clone();
        Rc::new(move |panel: &str| {
            let Some(title) = page_names.get(panel) else {
                return;
            };
            let mut entries = history.borrow_mut();
            let index = history_index.get();
            entries.truncate(index + 1);
            if entries.last().is_none_or(|current| current != panel) {
                entries.push(panel.to_string());
            }
            let current_index = entries.len().saturating_sub(1);
            history_index.set(current_index);
            stack.set_visible_child_name(panel);
            page_title.set_text(title);
            search.set_text("");
            for button in active_buttons.borrow().iter() {
                button.remove_css_class("selected");
                if button.label().as_deref() == Some(title.as_str()) {
                    button.add_css_class("selected");
                }
            }
            back.set_sensitive(current_index > 0);
            forward.set_sensitive(false);
            window.present();
        }) as SettingsNavigator
    };

    (window, navigator)
}

#[cfg(test)]
mod settings_search_tests {
    use super::{
        audio_balance_from_volumes, battery_health_percentage, clock_format_string,
        configured_portal_backend, dbus_interface_available_with, launcher_entry_matches,
        mako_focus_status_with, menu_bar_module_visible, notification_mode_active,
        parse_audio_channel_volumes, parse_audio_devices, parse_battery_history,
        parse_bluetooth_devices, parse_bluetooth_powered, parse_libinput_device_summary,
        parse_power_profiles, parse_storage_row, parse_wifi_networks, parse_wifi_profiles,
        printer_status_with, secret_service_status, settings_panel_from_uri,
        settings_search_matches, update_menu_bar_module, DesktopApp,
    };
    use crate::preferences::UserPreferences;

    #[test]
    fn search_supports_common_indonesian_terms_and_actions() {
        assert!(settings_search_matches(
            "layar kuning",
            "displays resolution refresh rate night shift"
        ));
        assert!(settings_search_matches(
            "matikan bluetooth",
            "bluetooth paired headphones discoverable"
        ));
        assert!(settings_search_matches(
            "ubah resolusi",
            "displays resolution refresh rate"
        ));
    }

    #[test]
    fn search_tolerates_a_single_typo() {
        assert!(settings_search_matches(
            "bluetooh",
            "bluetooth paired headphones"
        ));
        assert!(!settings_search_matches(
            "bluetooh",
            "wireless internet network dns"
        ));
    }

    #[test]
    fn deep_links_resolve_panels_and_subpanels() {
        assert_eq!(
            settings_panel_from_uri("mactahoe://settings/privacy/camera"),
            Some("privacy")
        );
        assert_eq!(
            settings_panel_from_uri("mactahoe://settings/network/wifi?auto-join=true"),
            Some("wifi")
        );
        assert_eq!(
            settings_panel_from_uri("https://example.com/settings"),
            None
        );
        assert_eq!(
            settings_panel_from_uri("mactahoe://settings/not-a-panel"),
            None
        );
        assert_eq!(
            settings_panel_from_uri("mactahoe://settings/continuity"),
            None
        );
        assert_eq!(
            settings_panel_from_uri("mactahoe://settings/assistant"),
            None
        );
    }

    #[test]
    fn desktop_entry_registers_uri_launch_argument() {
        let desktop_entry = include_str!("../../../data/march-scara-settings-handler.desktop");
        assert!(desktop_entry
            .lines()
            .any(|line| line == "Exec=/usr/bin/scara-shell %u"));
        assert!(desktop_entry
            .lines()
            .any(|line| line == "MimeType=x-scheme-handler/mactahoe;"));
    }

    #[test]
    fn menu_bar_module_preferences_can_be_updated_independently() {
        let mut preferences = UserPreferences::default();
        assert!(menu_bar_module_visible(
            &preferences.menu_bar,
            "control_center"
        ));

        update_menu_bar_module(&mut preferences, "wifi", false);
        assert!(!menu_bar_module_visible(&preferences.menu_bar, "wifi"));
        assert!(menu_bar_module_visible(&preferences.menu_bar, "bluetooth"));
    }

    #[test]
    fn storage_usage_rows_are_parsed_and_bounded() {
        let (summary, usage) =
            parse_storage_row("/dev/root 100G 75G 25G 75% /").expect("valid df row");
        assert!(summary.contains("100G total"));
        assert_eq!(usage, 0.75);
        assert!(parse_storage_row("not a df row").is_err());
    }

    #[test]
    fn wifi_scan_rows_decode_escaped_ssids_and_keep_active_network() {
        let networks = parse_wifi_networks(
            "*:Cafe\\:Bar:42:WPA2\n:Cafe\\:Bar:93:WPA2\n:Open network:0:--\n::55:WPA3",
        );

        assert_eq!(networks.len(), 2);
        assert_eq!(networks[0].ssid, "Cafe:Bar");
        assert_eq!(networks[0].signal, 42);
        assert!(networks[0].in_use);
        assert_eq!(networks[1].ssid, "Open network");
        assert_eq!(networks[1].security, "Open");
    }

    #[test]
    fn saved_wifi_profiles_are_filtered_and_escape_profile_names() {
        let profiles = parse_wifi_profiles(
            "Home\\: 5GHz:abc-123:802-11-wireless:wlan0\nWired:xyz-789:802-3-ethernet:eth0",
        );

        assert_eq!(profiles.len(), 1);
        assert_eq!(profiles[0].name, "Home: 5GHz");
        assert_eq!(profiles[0].uuid, "abc-123");
        assert!(profiles[0].active);
    }

    #[test]
    fn bluetooth_devices_merge_paired_and_connected_state() {
        let devices = parse_bluetooth_devices(
            "Device AA:BB:CC:DD:EE:FF Headphones",
            "Device AA:BB:CC:DD:EE:FF Headphones\nDevice 00:11:22:33:44:55 Keyboard",
            "Device AA:BB:CC:DD:EE:FF Headphones",
        );

        assert_eq!(devices.len(), 2);
        assert_eq!(devices[0].name, "Headphones");
        assert!(devices[0].paired);
        assert!(devices[0].connected);
        assert!(!devices[1].paired);
        assert!(!devices[1].connected);
    }

    #[test]
    fn bluetooth_power_state_parser_requires_known_value() {
        assert_eq!(parse_bluetooth_powered("Powered: yes\n"), Some(true));
        assert_eq!(parse_bluetooth_powered("Powered: no\n"), Some(false));
        assert_eq!(parse_bluetooth_powered("No controller available"), None);
    }

    #[test]
    fn audio_device_parser_excludes_monitor_sources() {
        let sources = parse_audio_devices(
            "42\talsa_input.usb-microphone\tmodule\t...\tRUNNING\n43\talsa_output.monitor\tmodule\t...\tIDLE",
            false,
        );
        assert_eq!(
            sources,
            vec![super::AudioDevice {
                name: "alsa_input.usb-microphone".to_string()
            }]
        );
    }

    #[test]
    fn audio_balance_uses_stereo_channel_volumes_only() {
        let volumes = parse_audio_channel_volumes(
            "Volume: front-left: 65536 / 80% / -1.94 dB, front-right: 65536 / 100% / 0.00 dB",
        );
        assert_eq!(volumes, vec![80.0, 100.0]);
        assert_eq!(audio_balance_from_volumes(&volumes), Some(20.0));
        assert_eq!(audio_balance_from_volumes(&[100.0]), None);
    }

    #[test]
    fn upower_history_parser_checks_declared_sample_count() {
        let history = parse_battery_history("a(udu) 2 1700000000 79.5 2 1700003600 81 1")
            .expect("valid UPower history");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].percentage, 79.5);
        assert_eq!(history[0].state, 2);
        assert!(parse_battery_history("a(udu) 2 1700000000 79.5 2").is_err());
    }

    #[test]
    fn power_profile_parser_ignores_profile_details() {
        assert_eq!(
            parse_power_profiles(
                "performance:\n  CpuDriver: intel_pstate\n* balanced:\n  PlatformDriver: platform_profile\npower-saver:\n"
            ),
            vec!["performance", "balanced", "power-saver"]
        );
    }

    #[test]
    fn march_portal_configuration_routes_interfaces_to_supported_backends() {
        let configuration = include_str!("../../../data/march-scara-portals.conf");
        assert_eq!(
            configured_portal_backend(configuration, "org.freedesktop.impl.portal.ScreenCast"),
            Some("wlr")
        );
        assert_eq!(
            configured_portal_backend(configuration, "org.freedesktop.impl.portal.Screenshot"),
            Some("wlr")
        );
        assert_eq!(
            configured_portal_backend(configuration, "org.freedesktop.impl.portal.FileChooser"),
            Some("gtk")
        );
        assert_eq!(
            configured_portal_backend(configuration, "org.freedesktop.impl.portal.Settings"),
            Some("gtk")
        );
    }

    #[test]
    fn mako_focus_mode_is_detected_from_active_modes() {
        assert!(notification_mode_active(
            "do-not-disturb\npresentation",
            "do-not-disturb"
        ));
        assert!(!notification_mode_active(
            "presentation\n",
            "do-not-disturb"
        ));
        assert!(!notification_mode_active(
            "not-do-not-disturb",
            "do-not-disturb"
        ));
    }

    #[test]
    fn mako_status_and_bus_permission_failures_are_reported() {
        assert_eq!(
            mako_focus_status_with(|_, _| Err("Access denied".to_string())),
            Err("Access denied".to_string())
        );
        let status = secret_service_status(|| Err("Access denied".to_string()));
        assert!(status.contains("Access denied"));
    }

    #[test]
    fn dbus_service_probe_uses_requested_bus_and_surfaces_denial() {
        let result = dbus_interface_available_with(
            "--user",
            "org.freedesktop.secrets",
            "/org/freedesktop/secrets",
            "org.freedesktop.Secret.Service",
            |program, args| {
                assert_eq!(program, "busctl");
                assert_eq!(
                    args,
                    [
                        "--user",
                        "introspect",
                        "org.freedesktop.secrets",
                        "/org/freedesktop/secrets",
                        "org.freedesktop.Secret.Service"
                    ]
                );
                Err("Access denied".to_string())
            },
        );
        assert_eq!(result, Err("Access denied".to_string()));
    }

    #[test]
    fn cups_service_status_includes_printers_default_and_queue() {
        let status = printer_status_with(|program, args| {
            assert_eq!(program, "lpstat");
            match args {
                ["-p"] => Ok("printer Office is idle. enabled since today".to_string()),
                ["-d"] => Ok("system default destination: Office".to_string()),
                ["-o"] => Ok("Office-17 user 1024 bytes".to_string()),
                _ => Err("Unexpected lpstat arguments".to_string()),
            }
        })
        .expect("CUPS status should be combined");
        assert!(status.contains("printer Office"));
        assert!(status.contains("default destination: Office"));
        assert!(status.contains("Office-17"));
    }

    #[test]
    fn cups_permission_errors_are_not_misreported_as_empty_queues() {
        let result = printer_status_with(|_, args| {
            if args == ["-p"] {
                Err("Not authorized to query print service".to_string())
            } else {
                Ok(String::new())
            }
        });
        assert_eq!(
            result,
            Err("Not authorized to query print service".to_string())
        );
    }

    #[test]
    fn libinput_device_listing_filters_pointer_and_touchpad_devices() {
        let output = "Device: USB Mouse\nKernel: /dev/input/event1\nCapabilities: pointer\n\nDevice: ELAN Touchpad\nKernel: /dev/input/event2\nCapabilities: pointer\nTap-to-click: enabled\n";
        let pointers = parse_libinput_device_summary(output, "pointer");
        assert!(pointers.contains("USB Mouse"));
        assert!(!pointers.contains("ELAN Touchpad"));
        let touchpads = parse_libinput_device_summary(output, "touchpad");
        assert!(touchpads.contains("ELAN Touchpad"));
        assert!(touchpads.contains("Tap-to-click: enabled"));
        assert!(!touchpads.contains("USB Mouse"));
    }

    #[test]
    fn launcher_search_keeps_application_matching_and_settings_synonyms() {
        let terminal = DesktopApp {
            name: "Terminal".to_string(),
            exec: "kitty".to_string(),
            icon: "terminal".to_string(),
        };
        assert!(launcher_entry_matches("terminal", &terminal));

        let bluetooth_settings = DesktopApp {
            name: "Bluetooth Settings".to_string(),
            exec: "__scara_settings__:bluetooth".to_string(),
            icon: "settings".to_string(),
        };
        assert!(launcher_entry_matches(
            "matikan bluetooth",
            &bluetooth_settings
        ));
    }

    #[test]
    fn clock_format_follows_the_selected_hour_cycle() {
        assert!(clock_format_string(true).contains("%H"));
        assert!(clock_format_string(false).contains("%p"));
    }

    #[test]
    fn battery_health_is_only_reported_for_valid_design_capacity() {
        assert_eq!(battery_health_percentage(80, 100), Some(80.0));
        assert_eq!(battery_health_percentage(120, 100), Some(100.0));
        assert_eq!(battery_health_percentage(80, 0), None);
    }
}

enum MenuAction {
    Exec(&'static str),
    Ipc(&'static str),
    Apps,
    Settings,
    Separator,
}

fn make_dropdown_menu(
    label: Option<&str>,
    icon: Option<&str>,
    bold: bool,
    items: &[(&str, MenuAction)],
    launcher: &ApplicationWindow,
    launcher_search: &SearchEntry,
    settings: &ApplicationWindow,
) -> MenuButton {
    let mb = MenuButton::new();
    mb.add_css_class("flat");
    mb.add_css_class("menubar-item");
    if bold {
        mb.add_css_class("menubar-brand-btn");
    }

    if let Some(icon_name) = icon {
        let img = Image::from_icon_name(icon_name);
        img.set_pixel_size(14);
        mb.set_child(Some(&img));
    } else if let Some(text) = label {
        let lbl = Label::new(Some(text));
        mb.set_child(Some(&lbl));
    }

    let pop = Popover::new();
    pop.set_has_arrow(false);
    pop.set_offset(0, 4);
    pop.set_has_arrow(false);
    pop.set_offset(0, 4);
    pop.add_css_class("tahoe-popover");

    let box_col = GtkBox::new(Orientation::Vertical, 2);
    box_col.set_width_request(195);

    for (title, action) in items {
        match action {
            MenuAction::Separator => {
                let sep = Separator::new(Orientation::Horizontal);
                sep.set_margin_top(4);
                sep.set_margin_bottom(4);
                box_col.append(&sep);
            }
            MenuAction::Exec(cmd) => {
                let b = Button::with_label(title);
                b.add_css_class("flat");
                b.add_css_class("menu-action-btn");
                if let Some(lbl) = b.child().and_then(|c| c.downcast::<Label>().ok()) {
                    lbl.set_halign(Align::Start);
                }
                let pop_ref = pop.clone();
                let cmd_str = cmd.to_string();
                b.connect_clicked(move |_| {
                    pop_ref.popdown();
                    launch_app(&cmd_str);
                });
                box_col.append(&b);
            }
            MenuAction::Ipc(ipc_cmd) => {
                let b = Button::with_label(title);
                b.add_css_class("flat");
                b.add_css_class("menu-action-btn");
                if let Some(lbl) = b.child().and_then(|c| c.downcast::<Label>().ok()) {
                    lbl.set_halign(Align::Start);
                }
                let pop_ref = pop.clone();
                let ipc_str = ipc_cmd.to_string();
                b.connect_clicked(move |_| {
                    pop_ref.popdown();
                    send_wm_ipc(&ipc_str);
                });
                box_col.append(&b);
            }
            MenuAction::Apps | MenuAction::Settings => {
                let b = Button::with_label(title);
                b.add_css_class("flat");
                b.add_css_class("menu-action-btn");
                if let Some(lbl) = b.child().and_then(|c| c.downcast::<Label>().ok()) {
                    lbl.set_halign(Align::Start);
                }
                let pop_ref = pop.clone();
                if matches!(action, MenuAction::Apps) {
                    let launcher = launcher.clone();
                    let search = launcher_search.clone();
                    b.connect_clicked(move |_| {
                        pop_ref.popdown();
                        show_launcher(&launcher, &search);
                    });
                } else {
                    let settings = settings.clone();
                    b.connect_clicked(move |_| {
                        pop_ref.popdown();
                        settings.present();
                    });
                }
                box_col.append(&b);
            }
        }
    }

    pop.set_child(Some(&box_col));
    mb.set_popover(Some(&pop));
    mb
}

fn refresh_wifi_popover(status: &Label, list: &GtkBox) {
    let radio = match run_program_output("nmcli", &["radio", "wifi"]) {
        Ok(radio) => radio,
        Err(error) => {
            status.set_text(&format!("NetworkManager unavailable: {error}"));
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            return;
        }
    };
    if radio != "enabled" {
        status.set_text("Wi-Fi is off");
        while let Some(child) = list.first_child() {
            list.remove(&child);
        }
        return;
    }

    let output = match run_program_output(
        "nmcli",
        &[
            "-t",
            "--escape",
            "yes",
            "--fields",
            "IN-USE,SSID,SIGNAL,SECURITY",
            "device",
            "wifi",
            "list",
        ],
    ) {
        Ok(output) => output,
        Err(error) => {
            status.set_text(&format!("Could not list Wi-Fi networks: {error}"));
            while let Some(child) = list.first_child() {
                list.remove(&child);
            }
            return;
        }
    };

    let networks = parse_wifi_networks(&output);
    if let Some(active) = networks.iter().find(|network| network.in_use) {
        status.set_text(&format!("Connected: {}", active.ssid));
    } else {
        status.set_text("Not connected");
    }
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    for network in networks {
        let label = format!(
            "{} · {}% · {}",
            network.ssid, network.signal, network.security
        );
        if network.in_use {
            let connected = Label::new(Some(&format!("Connected · {label}")));
            connected.set_halign(Align::Start);
            connected.add_css_class("cc-sub");
            list.append(&connected);
            continue;
        }

        let button = Button::with_label(&label);
        button.add_css_class("flat");
        button.add_css_class("menu-action-btn");
        let ssid = network.ssid;
        let status_ref = status.clone();
        button.connect_clicked(move |_| {
            match Command::new("kitty")
                .arg("--")
                .arg("nmcli")
                .arg("--ask")
                .args(["device", "wifi", "connect"])
                .arg(&ssid)
                .spawn()
            {
                Ok(_) => status_ref.set_text("Connection prompt opened in Kitty."),
                Err(error) => status_ref.set_text(&format!("Could not open Kitty: {error}")),
            }
        });
        list.append(&button);
    }
}

fn refresh_saved_wifi_profiles(list: &GtkBox, status: &Label) {
    while let Some(child) = list.first_child() {
        list.remove(&child);
    }

    let output = match run_program_output(
        "nmcli",
        &[
            "-t",
            "--escape",
            "yes",
            "--fields",
            "NAME,UUID,TYPE,DEVICE",
            "connection",
            "show",
        ],
    ) {
        Ok(output) => output,
        Err(error) => {
            status.set_text(&format!("Could not list saved networks: {error}"));
            return;
        }
    };

    let profiles = parse_wifi_profiles(&output);
    if profiles.is_empty() {
        status.set_text("No saved Wi-Fi networks.");
        return;
    }
    status.set_text(&format!("{} saved Wi-Fi network(s).", profiles.len()));

    for profile in profiles {
        let row = GtkBox::new(Orientation::Horizontal, 8);
        let name = Label::new(Some(&profile.name));
        name.set_halign(Align::Start);
        name.set_hexpand(true);
        row.append(&name);

        let action = if profile.active {
            "Disconnect"
        } else {
            "Connect"
        };
        let action_button = Button::with_label(action);
        action_button.add_css_class("flat");
        action_button.add_css_class("menu-action-btn");
        let args = if profile.active {
            vec![
                "connection".to_string(),
                "down".to_string(),
                "uuid".to_string(),
                profile.uuid.clone(),
            ]
        } else {
            vec![
                "--wait".to_string(),
                "15".to_string(),
                "connection".to_string(),
                "up".to_string(),
                "uuid".to_string(),
                profile.uuid.clone(),
            ]
        };
        let status_ref = status.clone();
        let list_ref = list.clone();
        action_button.connect_clicked(move |_| {
            let status_after = status_ref.clone();
            let list_after = list_ref.clone();
            run_system_command_async("nmcli", args.clone(), status_ref.clone(), move |success| {
                if success {
                    refresh_saved_wifi_profiles(&list_after, &status_after);
                }
            });
        });
        row.append(&action_button);

        let forget_button = Button::with_label("Forget");
        forget_button.add_css_class("flat");
        forget_button.add_css_class("menu-action-btn");
        let uuid = profile.uuid;
        let status_ref = status.clone();
        let list_ref = list.clone();
        forget_button.connect_clicked(move |_| {
            let status_after = status_ref.clone();
            let list_after = list_ref.clone();
            run_system_command_async(
                "nmcli",
                vec![
                    "connection".to_string(),
                    "delete".to_string(),
                    "uuid".to_string(),
                    uuid.clone(),
                ],
                status_ref.clone(),
                move |success| {
                    if success {
                        refresh_saved_wifi_profiles(&list_after, &status_after);
                    }
                },
            );
        });
        row.append(&forget_button);
        list.append(&row);
    }
}

fn build_wifi_button() -> MenuButton {
    let mb = MenuButton::new();
    mb.add_css_class("flat");
    mb.add_css_class("menubar-item");
    let img = Image::from_icon_name("network-wireless-signal-excellent-symbolic");
    img.set_pixel_size(14);
    mb.set_child(Some(&img));
    mb.set_tooltip_text(Some("Wi-Fi"));

    let pop = Popover::new();
    pop.set_has_arrow(false);
    pop.set_offset(0, 4);
    pop.add_css_class("tahoe-popover");

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_width_request(240);

    let title = Label::new(Some("Wi-Fi"));
    title.add_css_class("cc-title");
    title.set_halign(Align::Start);
    root.append(&title);

    let status_lbl = Label::new(Some("Memeriksa jaringan..."));
    status_lbl.add_css_class("cc-sub");
    status_lbl.set_halign(Align::Start);
    root.append(&status_lbl);

    let list_box = GtkBox::new(Orientation::Vertical, 2);
    let network_scroll = ScrolledWindow::new();
    network_scroll.set_min_content_height(80);
    network_scroll.set_max_content_height(220);
    network_scroll.set_child(Some(&list_box));
    root.append(&network_scroll);

    let toggle_btn = Button::with_label("Toggle Wi-Fi On/Off");
    toggle_btn.add_css_class("flat");
    toggle_btn.add_css_class("menu-action-btn");
    let status_ref2 = status_lbl.clone();
    let list_ref2 = list_box.clone();
    toggle_btn.connect_clicked(move |_| {
        let state = run_program_output("nmcli", &["radio", "wifi"]);
        let action = match state {
            Ok(state) if state == "enabled" => "off",
            Ok(_) => "on",
            Err(error) => {
                status_ref2.set_text(&format!("Could not read Wi-Fi state: {error}"));
                return;
            }
        };
        match run_program_output("nmcli", &["radio", "wifi", action]) {
            Ok(_) => refresh_wifi_popover(&status_ref2, &list_ref2),
            Err(error) => status_ref2.set_text(&format!("Could not change Wi-Fi: {error}")),
        }
    });
    root.append(&toggle_btn);

    let refresh_btn = Button::with_label("Scan for Networks");
    refresh_btn.add_css_class("flat");
    refresh_btn.add_css_class("menu-action-btn");
    let status_ref = status_lbl.clone();
    let list_ref = list_box.clone();
    refresh_btn.connect_clicked(move |_| {
        match run_program_output("nmcli", &["device", "wifi", "rescan"]) {
            Ok(_) => {
                status_ref.set_text("Scanning for networks...");
                let status_after_scan = status_ref.clone();
                let list_after_scan = list_ref.clone();
                glib::timeout_add_seconds_local(2, move || {
                    refresh_wifi_popover(&status_after_scan, &list_after_scan);
                    glib::ControlFlow::Break
                });
            }
            Err(error) => status_ref.set_text(&format!("Could not scan for networks: {error}")),
        }
    });
    root.append(&refresh_btn);

    let status_ref = status_lbl.clone();
    let list_ref = list_box.clone();
    pop.connect_show(move |_| {
        refresh_wifi_popover(&status_ref, &list_ref);
    });

    let editor_btn = Button::with_label("Open Connection Editor");
    editor_btn.add_css_class("flat");
    editor_btn.add_css_class("menu-action-btn");
    let status_ref = status_lbl.clone();
    editor_btn.connect_clicked(move |_| match launch_program("nm-connection-editor", &[]) {
        Ok(()) => status_ref.set_text("Opened NetworkManager connection editor."),
        Err(error) => status_ref.set_text(&format!("Connection editor unavailable: {error}")),
    });
    root.append(&editor_btn);

    pop.set_child(Some(&root));
    mb.set_popover(Some(&pop));
    mb
}

fn build_bluetooth_button() -> MenuButton {
    let mb = MenuButton::new();
    mb.add_css_class("flat");
    mb.add_css_class("menubar-item");
    let img = Image::from_icon_name("bluetooth-active-symbolic");
    img.set_pixel_size(14);
    mb.set_child(Some(&img));
    mb.set_tooltip_text(Some("Bluetooth"));

    let pop = Popover::new();
    pop.set_has_arrow(false);
    pop.set_offset(0, 4);
    pop.add_css_class("tahoe-popover");

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_width_request(220);

    let title = Label::new(Some("Bluetooth"));
    title.add_css_class("cc-title");
    title.set_halign(Align::Start);
    root.append(&title);

    let status_lbl = Label::new(Some("Checking..."));
    status_lbl.add_css_class("cc-sub");
    status_lbl.set_halign(Align::Start);
    root.append(&status_lbl);

    let toggle_btn = Button::with_label("Toggle Bluetooth On/Off");
    toggle_btn.add_css_class("flat");
    toggle_btn.add_css_class("menu-action-btn");
    let s_ref = status_lbl.clone();
    toggle_btn.connect_clicked(move |_| {
        let powered = match run_program_output("bluetoothctl", &["show"]).and_then(|output| {
            parse_bluetooth_powered(&output)
                .ok_or_else(|| "BlueZ did not report adapter power state.".to_string())
        }) {
            Ok(powered) => powered,
            Err(error) => {
                s_ref.set_text(&format!("Bluetooth unavailable: {error}"));
                return;
            }
        };
        let action = if powered { "off" } else { "on" };
        match run_program_output("bluetoothctl", &["power", action]) {
            Ok(_) => s_ref.set_text(if powered { "Status: Off" } else { "Status: On" }),
            Err(error) => s_ref.set_text(&format!("Could not change Bluetooth: {error}")),
        }
    });
    root.append(&toggle_btn);

    let manager_btn = Button::with_label("Open Bluetooth Manager");
    manager_btn.add_css_class("flat");
    manager_btn.add_css_class("menu-action-btn");
    let manager_status = status_lbl.clone();
    manager_btn.connect_clicked(move |_| match launch_program("blueman-manager", &[]) {
        Ok(()) => manager_status.set_text("Opened Blueman."),
        Err(error) => manager_status.set_text(&format!("Bluetooth manager unavailable: {error}")),
    });
    root.append(&manager_btn);

    let s_show = status_lbl.clone();
    pop.connect_show(move |_| {
        match run_program_output("bluetoothctl", &["show"]).and_then(|output| {
            parse_bluetooth_powered(&output)
                .ok_or_else(|| "BlueZ did not report adapter power state.".to_string())
        }) {
            Ok(true) => s_show.set_text("Status: Powered On"),
            Ok(false) => s_show.set_text("Status: Off"),
            Err(error) => s_show.set_text(&format!("Bluetooth unavailable: {error}")),
        }
    });

    pop.set_child(Some(&root));
    mb.set_popover(Some(&pop));
    mb
}

fn build_spotlight_button() -> MenuButton {
    let mb = MenuButton::new();
    mb.add_css_class("flat");
    mb.add_css_class("menubar-item");
    let img = Image::from_icon_name("system-search-symbolic");
    img.set_pixel_size(14);
    mb.set_child(Some(&img));
    mb.set_tooltip_text(Some("Spotlight Search"));

    let pop = Popover::new();
    pop.set_has_arrow(false);
    pop.set_offset(0, 4);
    pop.add_css_class("tahoe-popover");

    let root = GtkBox::new(Orientation::Vertical, 8);
    root.set_width_request(320);

    let entry = SearchEntry::new();
    entry.set_placeholder_text(Some("Spotlight Search (ketik nama app)..."));
    root.append(&entry);

    let scroll = ScrolledWindow::builder().min_content_height(210).build();
    let results_box = GtkBox::new(Orientation::Vertical, 2);
    scroll.set_child(Some(&results_box));
    root.append(&scroll);

    let apps = load_desktop_apps();
    let apps_clone = apps.clone();
    let results_ref = results_box.clone();
    let pop_ref = pop.clone();

    let populate = move |query: &str| {
        while let Some(child) = results_ref.first_child() {
            results_ref.remove(&child);
        }
        let q = query.to_lowercase();
        let mut count = 0;
        for app in &apps_clone {
            if !q.is_empty() && !app.name.to_lowercase().contains(&q) {
                continue;
            }
            if count >= 12 {
                break;
            }
            count += 1;

            let row = GtkBox::new(Orientation::Horizontal, 8);
            let icon = Image::from_icon_name(&app.icon);
            icon.set_pixel_size(20);
            let lbl = Label::new(Some(&app.name));
            lbl.set_halign(Align::Start);
            row.append(&icon);
            row.append(&lbl);

            let btn = Button::builder().child(&row).build();
            btn.add_css_class("flat");
            btn.add_css_class("menu-action-btn");
            let exec_str = app.exec.clone();
            let p_close = pop_ref.clone();
            btn.connect_clicked(move |_| {
                p_close.popdown();
                launch_app(&exec_str);
            });
            results_ref.append(&btn);
        }
    };

    let pop_enter = pop.clone();
    let apps_enter = apps.clone();
    entry.connect_activate(move |e| {
        let q = e.text().to_lowercase();
        if let Some(first) = apps_enter
            .iter()
            .find(|a| q.is_empty() || a.name.to_lowercase().contains(&q))
        {
            pop_enter.popdown();
            launch_app(&first.exec);
        }
    });

    let populate_change = populate.clone();
    entry.connect_search_changed(move |e| {
        populate_change(&e.text());
    });

    let entry_focus = entry.clone();
    pop.connect_show(move |_| {
        entry_focus.set_text("");
        populate("");
        entry_focus.grab_focus();
    });

    pop.set_child(Some(&root));
    mb.set_popover(Some(&pop));
    mb
}

fn build_control_center_popover(
    settings: &ApplicationWindow,
    menu_bar_widgets: &MenuBarWidgetRegistry,
) -> Popover {
    let popover = Popover::new();
    popover.set_has_arrow(false);
    popover.set_offset(0, 4);
    popover.add_css_class("tahoe-popover");

    let root = GtkBox::new(Orientation::Vertical, 10);
    root.set_width_request(260);

    let header = Label::new(Some("Control Center"));
    header.add_css_class("cc-title");
    header.set_halign(Align::Start);
    root.append(&header);

    let net_card = GtkBox::new(Orientation::Vertical, 4);
    net_card.add_css_class("cc-card");
    let wifi_lbl = Label::new(Some("Wi-Fi  •  Active"));
    wifi_lbl.add_css_class("cc-title");
    wifi_lbl.set_halign(Align::Start);
    let bt_lbl = Label::new(Some("MARCH SCARA Native Shell"));
    bt_lbl.add_css_class("cc-sub");
    bt_lbl.set_halign(Align::Start);
    net_card.append(&wifi_lbl);
    net_card.append(&bt_lbl);
    root.append(&net_card);

    let vol_card = GtkBox::new(Orientation::Vertical, 6);
    vol_card.add_css_class("cc-card");
    let vol_lbl = Label::new(Some("Sound"));
    vol_lbl.add_css_class("cc-title");
    vol_lbl.set_halign(Align::Start);
    let slider = Scale::with_range(Orientation::Horizontal, 0.0, 100.0, 5.0);
    slider.set_value(65.0);
    slider.connect_value_changed(|s| {
        let v = s.value() as u32;
        launch_app(&format!("pamixer --set-volume {}", v));
    });
    vol_card.append(&vol_lbl);
    vol_card.append(&slider);
    root.append(&vol_card);
    menu_bar_widgets
        .borrow_mut()
        .insert("sound".to_string(), vol_card.clone().upcast());

    let settings_btn = Button::with_label("System Settings...");
    settings_btn.add_css_class("flat");
    settings_btn.add_css_class("menu-action-btn");
    let pop_ref = popover.clone();
    let settings = settings.clone();
    settings_btn.connect_clicked(move |_| {
        pop_ref.popdown();
        settings.present();
    });
    root.append(&settings_btn);

    popover.set_child(Some(&root));
    popover
}

fn build_cc_toggle_icon() -> GtkBox {
    let col = GtkBox::new(Orientation::Vertical, 2);
    col.set_valign(Align::Center);
    col.set_halign(Align::Center);
    col.set_hexpand(false);
    for _ in 0..2 {
        let track = GtkBox::new(Orientation::Horizontal, 0);
        track.set_halign(Align::Center);
        track.set_hexpand(false);
        track.set_size_request(13, 4);
        track.add_css_class("cc-toggle-track");
        col.append(&track);
    }
    col
}

fn build_menubar(
    app: &Application,
    launcher: &ApplicationWindow,
    launcher_search: &SearchEntry,
    settings: &ApplicationWindow,
    preferences: &Rc<RefCell<UserPreferences>>,
    menu_bar_widgets: &MenuBarWidgetRegistry,
) {
    let bar_win = ApplicationWindow::builder()
        .application(app)
        .title("scara-menubar")
        .build();

    bar_win.remove_css_class("background");
    bar_win.add_css_class("transparent-layer");
    bar_win.set_decorated(false);
    bar_win.init_layer_shell();
    bar_win.set_layer(Layer::Top);
    bar_win.set_namespace(Some("scara-menubar"));
    bar_win.auto_exclusive_zone_enable();
    bar_win.set_anchor(Edge::Top, true);
    bar_win.set_anchor(Edge::Left, true);
    bar_win.set_anchor(Edge::Right, true);

    let row = GtkBox::new(Orientation::Horizontal, 2);
    row.add_css_class("scara-menubar");

    // 1. Logo Apple / MARCH SCARA
    row.append(&make_dropdown_menu(
        None,
        Some("start-here-symbolic"),
        false,
        &[
            (
                "About MARCH SCARA",
                MenuAction::Exec("notify-send 'MARCH SCARA' 'Wayland Desktop Environment v0.1'"),
            ),
            ("---", MenuAction::Separator),
            ("System Settings...", MenuAction::Settings),
            ("Open Terminal", MenuAction::Exec("kitty")),
            ("---", MenuAction::Separator),
            ("Quit MARCH SCARA", MenuAction::Ipc("session:quit")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    // 2. Menu Bar Interaktif ala macOS Tahoe
    row.append(&make_dropdown_menu(
        Some("Finder"),
        None,
        true,
        &[
            ("Home Folder", MenuAction::Exec("thunar ~")),
            ("Applications", MenuAction::Apps),
            ("Trash", MenuAction::Exec("thunar trash:///")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("File"),
        None,
        false,
        &[
            ("New Terminal Window", MenuAction::Exec("kitty")),
            ("New File Manager Window", MenuAction::Exec("thunar")),
            ("New Text Note", MenuAction::Exec("gnome-text-editor")),
            ("---", MenuAction::Separator),
            ("Close Active Window", MenuAction::Ipc("window:close")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("Edit"),
        None,
        false,
        &[
            ("Copy (Clipboard)", MenuAction::Exec("wl-copy")),
            (
                "Take Screenshot",
                MenuAction::Exec("grim -g \"$(slurp)\" - | swappy -f -"),
            ),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("View"),
        None,
        false,
        &[
            (
                "Toggle Full / Floating",
                MenuAction::Ipc("window:toggle_floating"),
            ),
            ("Center Window", MenuAction::Ipc("window:center")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("Go"),
        None,
        false,
        &[
            ("Home", MenuAction::Exec("thunar ~")),
            ("Documents", MenuAction::Exec("thunar ~/Documents")),
            ("Downloads", MenuAction::Exec("thunar ~/Downloads")),
            ("MARCH SCARA Repo", MenuAction::Exec("thunar ~/march-scara")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("Window"),
        None,
        false,
        &[
            ("Tile Left Half", MenuAction::Ipc("window:tile_left")),
            ("Tile Right Half", MenuAction::Ipc("window:tile_right")),
            ("Center Floating", MenuAction::Ipc("window:center")),
            ("Toggle Maximize", MenuAction::Ipc("window:toggle_floating")),
            ("---", MenuAction::Separator),
            ("Close Window", MenuAction::Ipc("window:close")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    row.append(&make_dropdown_menu(
        Some("Help"),
        None,
        false,
        &[
            ("Super+Return : Terminal", MenuAction::Exec("kitty")),
            (
                "Super+F : Maximize/Float",
                MenuAction::Ipc("window:toggle_floating"),
            ),
            ("Super+Q : Close Window", MenuAction::Ipc("window:close")),
        ],
        launcher,
        launcher_search,
        settings,
    ));

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    row.append(&spacer);

    // 3. Tray Kanan: Bluetooth, Battery, Wi-Fi, Spotlight, Control Center, Clock
    let bluetooth_button = build_bluetooth_button();
    menu_bar_widgets
        .borrow_mut()
        .insert("bluetooth".to_string(), bluetooth_button.clone().upcast());
    row.append(&bluetooth_button);

    let (bat_pct, bat_stat) = read_battery_info();
    let bat_box = GtkBox::new(Orientation::Horizontal, 4);
    let bat_lbl = Label::new(Some(&bat_pct));
    bat_lbl.add_css_class("menubar-bat");
    let bat_icon = Image::from_icon_name("battery-good-symbolic");
    bat_icon.set_pixel_size(14);
    bat_box.append(&bat_lbl);
    bat_box.append(&bat_icon);

    let bat_btn = MenuButton::new();
    bat_btn.add_css_class("flat");
    bat_btn.add_css_class("menubar-item");
    bat_btn.set_child(Some(&bat_box));
    let bat_pop = Popover::new();
    bat_pop.set_has_arrow(false);
    bat_pop.set_offset(0, 4);
    bat_pop.add_css_class("tahoe-popover");
    let bat_info_lbl = Label::new(Some(&format!("Battery: {} ({})", bat_pct, bat_stat)));
    bat_pop.set_child(Some(&bat_info_lbl));
    bat_btn.set_popover(Some(&bat_pop));
    menu_bar_widgets
        .borrow_mut()
        .insert("battery".to_string(), bat_btn.clone().upcast());
    row.append(&bat_btn);

    let wifi_button = build_wifi_button();
    menu_bar_widgets
        .borrow_mut()
        .insert("wifi".to_string(), wifi_button.clone().upcast());
    row.append(&wifi_button);

    let spotlight_button = build_spotlight_button();
    menu_bar_widgets
        .borrow_mut()
        .insert("spotlight".to_string(), spotlight_button.clone().upcast());
    row.append(&spotlight_button);

    let cc_btn = MenuButton::new();
    cc_btn.add_css_class("flat");
    cc_btn.add_css_class("menubar-item");
    cc_btn.set_child(Some(&build_cc_toggle_icon()));
    cc_btn.set_popover(Some(&build_control_center_popover(
        settings,
        menu_bar_widgets,
    )));
    cc_btn.set_tooltip_text(Some("Control Center"));
    menu_bar_widgets
        .borrow_mut()
        .insert("control_center".to_string(), cc_btn.clone().upcast());
    row.append(&cc_btn);

    let clock_lbl = Label::new(Some(&format_tahoe_clock(
        preferences.borrow().clock_24_hour,
    )));
    clock_lbl.add_css_class("menubar-clock");
    menu_bar_widgets
        .borrow_mut()
        .insert("clock".to_string(), clock_lbl.clone().upcast());
    row.append(&clock_lbl);

    let clock_ref = clock_lbl.clone();
    let bat_ref = bat_lbl.clone();
    let bat_info_ref = bat_info_lbl.clone();
    let preferences_ref = preferences.clone();
    glib::timeout_add_seconds_local(5, move || {
        clock_ref.set_text(&format_tahoe_clock(preferences_ref.borrow().clock_24_hour));
        let (pct, stat) = read_battery_info();
        bat_ref.set_text(&pct);
        bat_info_ref.set_text(&format!("Battery: {} ({})", pct, stat));
        glib::ControlFlow::Continue
    });

    set_menu_bar_module_visibility(menu_bar_widgets, &preferences.borrow());
    bar_win.set_child(Some(&row));
    bar_win.present();
}

fn make_dock_item(
    label: &str,
    icon_candidates: &[&str],
    cmd: &str,
    active: bool,
    launcher: &ApplicationWindow,
    launcher_search: &SearchEntry,
    settings: &ApplicationWindow,
) -> GtkBox {
    let icon_theme = gtk::gdk::Display::default().map(|d| IconTheme::for_display(&d));
    let chosen_icon = icon_candidates
        .iter()
        .find(|&&name| icon_theme.as_ref().is_some_and(|t| t.has_icon(name)))
        .copied()
        .unwrap_or(icon_candidates[0]);

    let img = Image::from_icon_name(chosen_icon);
    img.set_pixel_size(42);

    let btn = Button::builder().child(&img).build();
    btn.add_css_class("flat");
    btn.add_css_class("dock-icon-btn");
    btn.set_tooltip_text(Some(label));

    let cmd_owned = cmd.to_string();
    let launcher = launcher.clone();
    let launcher_search = launcher_search.clone();
    let settings = settings.clone();
    let label_owned = label.to_string();
    btn.connect_clicked(move |_| match label_owned.as_str() {
        "Launchpad" => {
            show_launcher(&launcher, &launcher_search);
        }
        "System Settings" => settings.present(),
        _ => launch_app(&cmd_owned),
    });

    let dot = GtkBox::new(Orientation::Horizontal, 0);
    dot.set_halign(Align::Center);
    dot.add_css_class(if active {
        "dock-dot-active"
    } else {
        "dock-dot-idle"
    });

    let wrapper = GtkBox::new(Orientation::Vertical, 2);
    wrapper.set_halign(Align::Center);
    wrapper.append(&btn);
    wrapper.append(&dot);
    wrapper
}

fn build_dock(
    app: &Application,
    launcher: &ApplicationWindow,
    launcher_search: &SearchEntry,
    settings: &ApplicationWindow,
) {
    let dock_win = ApplicationWindow::builder()
        .application(app)
        .title("scara-dock")
        .build();

    dock_win.remove_css_class("background");
    dock_win.add_css_class("transparent-layer");
    dock_win.set_decorated(false);
    dock_win.init_layer_shell();
    dock_win.set_layer(Layer::Top);
    dock_win.set_namespace(Some("scara-dock"));
    dock_win.set_anchor(Edge::Bottom, true);
    dock_win.set_margin(Edge::Bottom, 0);

    let shadow_pad = GtkBox::new(Orientation::Horizontal, 0);
    shadow_pad.add_css_class("dock-shadow-pad");
    shadow_pad.set_halign(Align::Center);

    let dock_box = GtkBox::new(Orientation::Horizontal, 6);
    dock_box.add_css_class("scara-dock");
    dock_box.set_halign(Align::Center);

    let apps: &[(&str, &[&str], &str, bool)] = &[
        (
            "Finder",
            &["system-file-manager", "org.xfce.thunar"],
            "thunar",
            true,
        ),
        (
            "Launchpad",
            &["applications-all", "view-app-grid-symbolic"],
            "thunar /usr/share/applications",
            false,
        ),
        (
            "Browser",
            &["google-chrome", "firefox", "web-browser"],
            "google-chrome-stable || firefox",
            false,
        ),
        ("Terminal", &["kitty", "utilities-terminal"], "kitty", true),
        (
            "Editor",
            &["visual-studio-code", "code", "accessories-text-editor"],
            "code || gnome-text-editor",
            false,
        ),
        (
            "Notes",
            &[
                "accessories-notes",
                "org.gnome.TextEditor",
                "accessories-text-editor",
            ],
            "gnome-text-editor",
            false,
        ),
        (
            "Calendar",
            &["x-office-calendar", "org.gnome.Calendar"],
            "gnome-calendar",
            false,
        ),
        (
            "Videos",
            &["mpv", "video-x-generic", "multimedia-player"],
            "mpv --idle=yes --force-window=yes",
            false,
        ),
        (
            "Calculator",
            &["accessories-calculator", "org.gnome.Calculator"],
            "gnome-calculator",
            false,
        ),
        (
            "System Settings",
            &["preferences-system", "org.gnome.Settings"],
            "gnome-control-center",
            false,
        ),
    ];

    for (name, icons, exec_cmd, active) in apps {
        dock_box.append(&make_dock_item(
            name,
            icons,
            exec_cmd,
            *active,
            launcher,
            launcher_search,
            settings,
        ));
    }

    let sep = Separator::new(Orientation::Vertical);
    sep.add_css_class("dock-separator");
    dock_box.append(&sep);

    dock_box.append(&make_dock_item(
        "Documents",
        &["folder-documents", "folder"],
        "thunar ~/Documents",
        false,
        launcher,
        launcher_search,
        settings,
    ));
    dock_box.append(&make_dock_item(
        "Downloads",
        &["folder-download", "folder"],
        "thunar ~/Downloads",
        false,
        launcher,
        launcher_search,
        settings,
    ));
    dock_box.append(&make_dock_item(
        "Trash",
        &["user-trash-full", "user-trash"],
        "thunar trash:///",
        false,
        launcher,
        launcher_search,
        settings,
    ));

    shadow_pad.append(&dock_box);
    dock_win.set_child(Some(&shadow_pad));
    dock_win.present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.marchscara.Shell")
        .flags(gtk::gio::ApplicationFlags::HANDLES_OPEN)
        .build();
    let settings_navigator = Rc::new(RefCell::new(None::<SettingsNavigator>));

    let settings_navigator_ref = settings_navigator.clone();
    app.connect_activate(move |app| {
        apply_tahoe_icon_theme();

        let provider = CssProvider::new();
        provider.load_from_data(TAHOE_CSS);
        if let Some(display) = gtk::gdk::Display::default() {
            gtk::style_context_add_provider_for_display(
                &display,
                &provider,
                gtk::STYLE_PROVIDER_PRIORITY_USER + 100,
            );
        }

        let wallpaper_picture = build_wallpaper(app);
        let (initial_preferences, preferences_error) = match preferences::load() {
            Ok(preferences) => (preferences, None),
            Err(error) => (UserPreferences::default(), Some(error.to_string())),
        };
        let is_march_session = std::env::var("XDG_CURRENT_DESKTOP")
            .is_ok_and(|desktop| desktop.split(':').any(|name| name == "MARCH-SCARA"));
        if is_march_session {
            if let Err(error) =
                set_compositor_reduce_transparency(initial_preferences.reduce_transparency)
            {
                eprintln!("Could not restore compositor transparency preference: {error}");
            }
        }
        let preferences = Rc::new(RefCell::new(initial_preferences));
        let menu_bar_widgets = Rc::new(RefCell::new(std::collections::HashMap::new()));
        let (settings_window, navigator) = build_settings_window(
            app,
            &wallpaper_picture,
            preferences.clone(),
            preferences_error,
            menu_bar_widgets.clone(),
        );
        *settings_navigator_ref.borrow_mut() = Some(navigator.clone());
        let (launcher_window, launcher_search) =
            build_app_launcher(app, &settings_window, &navigator);
        build_menubar(
            app,
            &launcher_window,
            &launcher_search,
            &settings_window,
            &preferences,
            &menu_bar_widgets,
        );
        build_dock(app, &launcher_window, &launcher_search, &settings_window);

        let shell_commands = start_shell_ipc();
        let launcher = launcher_window.clone();
        let launcher_search = launcher_search.clone();
        let settings = settings_window.clone();
        glib::timeout_add_local(Duration::from_millis(100), move || {
            for command in shell_commands.try_iter() {
                match command.trim() {
                    "launcher" => {
                        show_launcher(&launcher, &launcher_search);
                    }
                    "settings" => settings.present(),
                    _ => eprintln!("Unknown shell IPC command: {}", command.trim()),
                }
            }
            glib::ControlFlow::Continue
        });
    });

    let settings_navigator_ref = settings_navigator.clone();
    app.connect_open(move |application, files, _hint| {
        let target = files.iter().find_map(|file| {
            let uri = file.uri();
            settings_panel_from_uri(uri.as_str())
        });
        let Some(target) = target else {
            eprintln!("No supported MARCH SCARA Settings deep link was provided.");
            return;
        };

        if settings_navigator_ref.borrow().is_none() {
            application.activate();
        }
        if let Some(navigate) = settings_navigator_ref.borrow().as_ref() {
            navigate(target);
        } else {
            eprintln!("MARCH SCARA Settings did not initialize.");
        }
    });

    app.run()
}
