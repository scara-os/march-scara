use gtk::prelude::*;
use gtk::{
    glib, Align, Application, ApplicationWindow, Box as GtkBox, Button, ContentFit, CssProvider,
    IconTheme, Image, Label, MenuButton, Orientation, Picture, Popover, Scale, ScrolledWindow,
    SearchEntry, Separator,
};
use gtk_layer_shell::{Edge, Layer, LayerShell};
use std::path::PathBuf;
use std::process::Command;

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

fn launch_app(cmd: &str) {
    let _ = Command::new("sh").arg("-c").arg(cmd).spawn();
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

fn run_cmd_output(cmd: &str) -> String {
    Command::new("sh")
        .arg("-c")
        .arg(cmd)
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
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
                            apps.push(DesktopApp { name: n, exec: e, icon });
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

fn find_wallpaper_path() -> Option<PathBuf> {
    let home = std::env::var("HOME").ok()?;
    let candidates = [
        format!("{}/.config/march-scara/wallpaper.jpg", home),
        format!("{}/.config/march-scara/wallpaper.png", home),
        format!("{}/.config/march-scara/wallpaper.webp", home),
    ];
    candidates.into_iter().map(PathBuf::from).find(|p| p.exists())
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

fn format_tahoe_clock() -> String {
    glib::DateTime::now_local()
        .ok()
        .and_then(|dt| dt.format("%a %b %-d  %-I:%M %p").ok())
        .map(|s| s.to_string())
        .unwrap_or_else(|| "MARCH SCARA".to_string())
}

fn read_battery_info() -> (String, String) {
    for bat in ["BAT0", "BAT1"] {
        let cap_path = format!("/sys/class/power_supply/{}/capacity", bat);
        let stat_path = format!("/sys/class/power_supply/{}/status", bat);
        if let Ok(cap) = std::fs::read_to_string(cap_path) {
            let status = std::fs::read_to_string(stat_path)
                .unwrap_or_else(|_| "Unknown".into())
                .trim()
                .to_string();
            return (format!("{}%", cap.trim()), status);
        }
    }
    ("100%".to_string(), "AC Power".to_string())
}

fn build_wallpaper(app: &Application) {
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

    if let Some(path) = find_wallpaper_path() {
        let pic = Picture::for_filename(&path);
        pic.set_content_fit(ContentFit::Cover);
        pic.set_can_shrink(true);
        bg_win.set_child(Some(&pic));
    } else {
        let fallback = GtkBox::new(Orientation::Vertical, 0);
        fallback.add_css_class("tahoe-wallpaper-fallback");
        fallback.set_hexpand(true);
        fallback.set_vexpand(true);
        bg_win.set_child(Some(&fallback));
    }

    bg_win.present();
}

enum MenuAction {
    Exec(&'static str),
    Ipc(&'static str),
    Separator,
}

fn make_dropdown_menu(
    label: Option<&str>,
    icon: Option<&str>,
    bold: bool,
    items: &[(&str, MenuAction)],
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
        }
    }

    pop.set_child(Some(&box_col));
    mb.set_popover(Some(&pop));
    mb
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
    root.append(&list_box);

    let toggle_btn = Button::with_label("Toggle Wi-Fi On/Off");
    toggle_btn.add_css_class("flat");
    toggle_btn.add_css_class("menu-action-btn");
    let status_ref2 = status_lbl.clone();
    toggle_btn.connect_clicked(move |_| {
        let state = run_cmd_output("nmcli radio wifi");
        if state == "enabled" {
            launch_app("nmcli radio wifi off");
            status_ref2.set_text("Wi-Fi: Off");
        } else {
            launch_app("nmcli radio wifi on");
            status_ref2.set_text("Wi-Fi: Turning On...");
        }
    });
    root.append(&toggle_btn);

    let status_ref = status_lbl.clone();
    let list_ref = list_box.clone();
    let pop_ref = pop.clone();
    pop.connect_show(move |_| {
        let active = run_cmd_output("nmcli -t -f active,ssid dev wifi | grep '^yes:' | cut -d: -f2");
        if active.is_empty() {
            status_ref.set_text("Not Connected");
        } else {
            status_ref.set_text(&format!("Connected: {}", active));
        }
        while let Some(child) = list_ref.first_child() {
            list_ref.remove(&child);
        }
        let nets = run_cmd_output("nmcli -t -f ssid dev wifi list | grep -v '^$' | head -n 5");
        for ssid in nets.lines() {
            let ssid_str = ssid.trim().to_string();
            let w_row = GtkBox::new(Orientation::Horizontal, 8);
            let w_ic = Image::from_icon_name("network-wireless-signal-good-symbolic");
            w_ic.set_pixel_size(14);
            let w_lbl = Label::new(Some(&ssid_str));
            w_lbl.set_halign(Align::Start);
            w_row.append(&w_ic);
            w_row.append(&w_lbl);
            let b = Button::builder().child(&w_row).build();
            b.add_css_class("flat");
            b.add_css_class("menu-action-btn");
            let p_close = pop_ref.clone();
            b.connect_clicked(move |_| {
                launch_app(&format!("nmcli dev wifi connect \"{}\"", ssid_str));
                p_close.popdown();
            });
            list_ref.append(&b);
        }
    });

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
        let powered = run_cmd_output("bluetoothctl show | grep 'Powered:' | awk '{print $2}'");
        if powered == "yes" {
            launch_app("bluetoothctl power off");
            s_ref.set_text("Status: Off");
        } else {
            launch_app("bluetoothctl power on");
            s_ref.set_text("Status: On");
        }
    });
    root.append(&toggle_btn);

    let s_show = status_lbl.clone();
    pop.connect_show(move |_| {
        let powered = run_cmd_output("bluetoothctl show | grep 'Powered:' | awk '{print $2}'");
        s_show.set_text(if powered == "yes" {
            "Status: Powered On"
        } else {
            "Status: Off"
        });
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

fn build_control_center_popover() -> Popover {
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

    let settings_btn = Button::with_label("System Settings...");
    settings_btn.add_css_class("flat");
    settings_btn.add_css_class("menu-action-btn");
    let pop_ref = popover.clone();
    settings_btn.connect_clicked(move |_| {
        pop_ref.popdown();
        launch_app("gnome-control-center");
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

fn build_menubar(app: &Application) {
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
            ("About MARCH SCARA", MenuAction::Exec("notify-send 'MARCH SCARA' 'Wayland Desktop Environment v0.1'")),
            ("---", MenuAction::Separator),
            ("System Settings...", MenuAction::Exec("gnome-control-center")),
            ("Open Terminal", MenuAction::Exec("kitty")),
            ("---", MenuAction::Separator),
            ("Quit MARCH SCARA", MenuAction::Ipc("session:quit")),
        ],
    ));

    // 2. Menu Bar Interaktif ala macOS Tahoe
    row.append(&make_dropdown_menu(
        Some("Finder"),
        None,
        true,
        &[
            ("Home Folder", MenuAction::Exec("nautilus ~")),
            ("Applications", MenuAction::Exec("nautilus /usr/share/applications")),
            ("Trash", MenuAction::Exec("nautilus trash:///")),
        ],
    ));

    row.append(&make_dropdown_menu(
        Some("File"),
        None,
        false,
        &[
            ("New Terminal Window", MenuAction::Exec("kitty")),
            ("New Finder Window", MenuAction::Exec("nautilus")),
            ("New Text Note", MenuAction::Exec("gnome-text-editor")),
            ("---", MenuAction::Separator),
            ("Close Active Window", MenuAction::Ipc("window:close")),
        ],
    ));

    row.append(&make_dropdown_menu(
        Some("Edit"),
        None,
        false,
        &[
            ("Copy (Clipboard)", MenuAction::Exec("wl-copy")),
            ("Take Screenshot", MenuAction::Exec("grim -g \"$(slurp)\" - | wl-copy")),
        ],
    ));

    row.append(&make_dropdown_menu(
        Some("View"),
        None,
        false,
        &[
            ("Toggle Full / Floating", MenuAction::Ipc("window:toggle_floating")),
            ("Center Window", MenuAction::Ipc("window:center")),
        ],
    ));

    row.append(&make_dropdown_menu(
        Some("Go"),
        None,
        false,
        &[
            ("Home", MenuAction::Exec("nautilus ~")),
            ("Documents", MenuAction::Exec("nautilus ~/Documents")),
            ("Downloads", MenuAction::Exec("nautilus ~/Downloads")),
            ("MARCH SCARA Repo", MenuAction::Exec("nautilus ~/march-scara")),
        ],
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
    ));

    row.append(&make_dropdown_menu(
        Some("Help"),
        None,
        false,
        &[
            ("Super+Return : Terminal", MenuAction::Exec("kitty")),
            ("Super+F : Maximize/Float", MenuAction::Ipc("window:toggle_floating")),
            ("Super+Q : Close Window", MenuAction::Ipc("window:close")),
        ],
    ));

    let spacer = GtkBox::new(Orientation::Horizontal, 0);
    spacer.set_hexpand(true);
    row.append(&spacer);

    // 3. Tray Kanan: Bluetooth, Battery, Wi-Fi, Spotlight, Control Center, Clock
    row.append(&build_bluetooth_button());

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
    row.append(&bat_btn);

    row.append(&build_wifi_button());
    row.append(&build_spotlight_button());

    let cc_btn = MenuButton::new();
    cc_btn.add_css_class("flat");
    cc_btn.add_css_class("menubar-item");
    cc_btn.set_child(Some(&build_cc_toggle_icon()));
    cc_btn.set_popover(Some(&build_control_center_popover()));
    cc_btn.set_tooltip_text(Some("Control Center"));
    row.append(&cc_btn);

    let clock_lbl = Label::new(Some(&format_tahoe_clock()));
    clock_lbl.add_css_class("menubar-clock");
    row.append(&clock_lbl);

    let clock_ref = clock_lbl.clone();
    let bat_ref = bat_lbl.clone();
    let bat_info_ref = bat_info_lbl.clone();
    glib::timeout_add_seconds_local(5, move || {
        clock_ref.set_text(&format_tahoe_clock());
        let (pct, stat) = read_battery_info();
        bat_ref.set_text(&pct);
        bat_info_ref.set_text(&format!("Battery: {} ({})", pct, stat));
        glib::ControlFlow::Continue
    });

    bar_win.set_child(Some(&row));
    bar_win.present();
}

fn make_dock_item(label: &str, icon_candidates: &[&str], cmd: &str, active: bool) -> GtkBox {
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
    btn.connect_clicked(move |_| launch_app(&cmd_owned));

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

fn build_dock(app: &Application) {
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
        ("Finder", &["system-file-manager", "org.gnome.Nautilus"], "nautilus", true),
        ("Launchpad", &["applications-all", "view-app-grid-symbolic"], "nautilus /usr/share/applications", false),
        ("Browser", &["google-chrome", "firefox", "web-browser"], "google-chrome-stable || firefox", false),
        ("Terminal", &["kitty", "utilities-terminal"], "kitty", true),
        ("Editor", &["visual-studio-code", "code", "accessories-text-editor"], "code || gnome-text-editor", false),
        ("Notes", &["accessories-notes", "org.gnome.TextEditor", "accessories-text-editor"], "gnome-text-editor", false),
        ("System Settings", &["preferences-system", "org.gnome.Settings"], "gnome-control-center", false),
    ];

    for (name, icons, exec_cmd, active) in apps {
        dock_box.append(&make_dock_item(name, icons, exec_cmd, *active));
    }

    let sep = Separator::new(Orientation::Vertical);
    sep.add_css_class("dock-separator");
    dock_box.append(&sep);

    dock_box.append(&make_dock_item(
        "Documents",
        &["folder-documents", "folder"],
        "nautilus ~/Documents",
        false,
    ));
    dock_box.append(&make_dock_item(
        "Downloads",
        &["folder-download", "folder"],
        "nautilus ~/Downloads",
        false,
    ));
    dock_box.append(&make_dock_item(
        "Trash",
        &["user-trash-full", "user-trash"],
        "nautilus trash:///",
        false,
    ));

    shadow_pad.append(&dock_box);
    dock_win.set_child(Some(&shadow_pad));
    dock_win.present();
}

fn main() -> glib::ExitCode {
    let app = Application::builder()
        .application_id("org.marchscara.Shell")
        .build();

    app.connect_activate(|app| {
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

        build_wallpaper(app);
        build_menubar(app);
        build_dock(app);
    });

    app.run()
}