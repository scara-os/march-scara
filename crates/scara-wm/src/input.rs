use smithay::{
    backend::input::{
        AbsolutePositionEvent, Axis, AxisSource, ButtonState, Event, InputBackend, InputEvent,
        KeyState, KeyboardKeyEvent, PointerAxisEvent, PointerButtonEvent, PointerMotionEvent,
    },
    input::{
        keyboard::{FilterResult, Keysym, ModifiersState},
        pointer::{AxisFrame, ButtonEvent, Focus, GrabStartData as PointerGrabStartData, MotionEvent},
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::protocol::wl_surface::WlSurface,
    },
    utils::{Logical, Point, Rectangle, Size, SERIAL_COUNTER},
};
use tracing::{info, warn};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    lua_bridge,
    state::Smallvil,
};

fn matches_keybind(combo: &str, mods: &ModifiersState, sym: Keysym) -> bool {
    let parts: Vec<String> = combo
        .split('+')
        .map(|s| s.trim().to_uppercase())
        .collect();
    if parts.is_empty() {
        return false;
    }

    let need_super = parts.iter().any(|p| p == "SUPER" || p == "LOGO");
    let need_ctrl = parts.iter().any(|p| p == "CTRL" || p == "CONTROL");
    let need_alt = parts.iter().any(|p| p == "ALT");
    let need_shift = parts.iter().any(|p| p == "SHIFT");

    if mods.logo != need_super
        || mods.ctrl != need_ctrl
        || mods.alt != need_alt
        || mods.shift != need_shift
    {
        return false;
    }

    let key_part = parts.last().map(|s| s.as_str()).unwrap_or("");
    match key_part {
        "RETURN" | "ENTER" => sym == Keysym::Return,
        "SPACE" => sym == Keysym::space,
        "TAB" => sym == Keysym::Tab || sym == Keysym::ISO_Left_Tab,
        "ESC" | "ESCAPE" => sym == Keysym::Escape,
        "BACKSPACE" => sym == Keysym::BackSpace,
        "DELETE" | "DEL" => sym == Keysym::Delete,
        "HOME" => sym == Keysym::Home,
        "END" => sym == Keysym::End,
        "LEFT" => sym == Keysym::Left,
        "RIGHT" => sym == Keysym::Right,
        "UP" => sym == Keysym::Up,
        "DOWN" => sym == Keysym::Down,
        "F1" => sym == Keysym::F1,
        "F2" => sym == Keysym::F2,
        "F3" => sym == Keysym::F3,
        "F4" => sym == Keysym::F4,
        "F5" => sym == Keysym::F5,
        "F6" => sym == Keysym::F6,
        "F7" => sym == Keysym::F7,
        "F8" => sym == Keysym::F8,
        "F9" => sym == Keysym::F9,
        "F10" => sym == Keysym::F10,
        "F11" => sym == Keysym::F11,
        "F12" => sym == Keysym::F12,
        "0" => sym == Keysym::_0 || sym == Keysym::parenright,
        "1" => sym == Keysym::_1 || sym == Keysym::exclam,
        "2" => sym == Keysym::_2 || sym == Keysym::at,
        "3" => sym == Keysym::_3 || sym == Keysym::numbersign,
        "4" => sym == Keysym::_4 || sym == Keysym::dollar,
        "5" => sym == Keysym::_5 || sym == Keysym::percent,
        "6" => sym == Keysym::_6 || sym == Keysym::asciicircum,
        "7" => sym == Keysym::_7 || sym == Keysym::ampersand,
        "8" => sym == Keysym::_8 || sym == Keysym::asterisk,
        "9" => sym == Keysym::_9 || sym == Keysym::parenleft,
        "A" => sym == Keysym::a || sym == Keysym::A,
        "B" => sym == Keysym::b || sym == Keysym::B,
        "C" => sym == Keysym::c || sym == Keysym::C,
        "D" => sym == Keysym::d || sym == Keysym::D,
        "E" => sym == Keysym::e || sym == Keysym::E,
        "F" => sym == Keysym::f || sym == Keysym::F,
        "G" => sym == Keysym::g || sym == Keysym::G,
        "H" => sym == Keysym::h || sym == Keysym::H,
        "I" => sym == Keysym::i || sym == Keysym::I,
        "J" => sym == Keysym::j || sym == Keysym::J,
        "K" => sym == Keysym::k || sym == Keysym::K,
        "L" => sym == Keysym::l || sym == Keysym::L,
        "M" => sym == Keysym::m || sym == Keysym::M,
        "N" => sym == Keysym::n || sym == Keysym::N,
        "O" => sym == Keysym::o || sym == Keysym::O,
        "P" => sym == Keysym::p || sym == Keysym::P,
        "Q" => sym == Keysym::q || sym == Keysym::Q,
        "R" => sym == Keysym::r || sym == Keysym::R,
        "S" => sym == Keysym::s || sym == Keysym::S,
        "T" => sym == Keysym::t || sym == Keysym::T,
        "U" => sym == Keysym::u || sym == Keysym::U,
        "V" => sym == Keysym::v || sym == Keysym::V,
        "W" => sym == Keysym::w || sym == Keysym::W,
        "X" => sym == Keysym::x || sym == Keysym::X,
        "Y" => sym == Keysym::y || sym == Keysym::Y,
        "Z" => sym == Keysym::z || sym == Keysym::Z,
        _ => false,
    }
}

impl Smallvil {
    pub fn execute_action(&mut self, action: &str) {
        info!("Executing MARCH SCARA action: {}", action);

        if let Some(cmd) = action.strip_prefix("exec:") {
            let _ = std::process::Command::new("sh").arg("-c").arg(cmd).spawn();
        } else if action == "session:quit" {
            self.loop_signal.stop();
        } else if action == "config:reload" {
            if let Ok(new_cfg) = lua_bridge::load_user_config(&self.config_path) {
                let mut cur = self.config.lock().unwrap();
                *cur = new_cfg.lock().unwrap().clone();
                drop(cur);
                info!("Reloaded init.lua live!");
                if self.config.lock().unwrap().default_layout == "tiling" {
                    self.apply_tiling_layout();
                }
            }
        } else if action == "layout:toggle" {
            let new_layout = {
                let mut cfg = self.config.lock().unwrap();
                cfg.default_layout = if cfg.default_layout == "tiling" {
                    "hybrid".to_string()
                } else {
                    "tiling".to_string()
                };
                cfg.default_layout.clone()
            };
            info!("Switched layout mode to: {}", new_layout);
            if new_layout == "tiling" {
                self.apply_tiling_layout();
            }
        } else if let Some(ws_str) = action.strip_prefix("workspace:") {
            if let Ok(ws_num) = ws_str.parse::<usize>() {
                if ws_num >= 1 {
                    self.switch_workspace(ws_num - 1);
                }
            }
        } else if let Some(ws_str) = action.strip_prefix("movetoworkspace:") {
            if let Ok(ws_num) = ws_str.parse::<usize>() {
                if ws_num >= 1 {
                    self.move_active_to_workspace(ws_num - 1);
                }
            }
        } else if action == "window:close" {
            if let Some(win) = self.space.elements().last().cloned() {
                if let Some(toplevel) = win.toplevel() {
                    toplevel.send_close();
                }
            }
        } else if matches!(
            action,
            "window:toggle_floating" | "window:tile_left" | "window:tile_right" | "window:center"
        ) {
            let win_opt = self.space.elements().last().cloned();
            let out_opt = self.space.outputs().next().cloned();
            if let (Some(win), Some(output)) = (win_opt, out_opt) {
                if let Some(out_geo) = self.space.output_geometry(&output) {
                    let cur_geo = self.space.element_geometry(&win);
                    let is_big = cur_geo
                        .map(|g| g.size.w >= out_geo.size.w - 80)
                        .unwrap_or(false);

                    let (target_loc, target_size): (Point<i32, Logical>, Size<i32, Logical>) =
                        match action {
                            "window:tile_left" => (
                                (12, 38).into(),
                                (out_geo.size.w / 2 - 18, out_geo.size.h - 124).into(),
                            ),
                            "window:tile_right" => (
                                (out_geo.size.w / 2 + 6, 38).into(),
                                (out_geo.size.w / 2 - 18, out_geo.size.h - 124).into(),
                            ),
                            "window:center" => (
                                ((out_geo.size.w - 820) / 2, (out_geo.size.h - 520) / 2).into(),
                                (820, 480).into(),
                            ),
                            _ if is_big => ((140, 74).into(), (820, 480).into()),
                            _ => (
                                (14, 38).into(),
                                (out_geo.size.w - 28, out_geo.size.h - 124).into(),
                            ),
                        };

                    if let Some(toplevel) = win.toplevel() {
                        toplevel.with_pending_state(|s| {
                            s.size = Some(target_size);
                        });
                        toplevel.send_configure();
                    }
                    self.space.raise_element(&win, true);
                    self.animate_window_to(win, target_loc.x, target_loc.y);
                }
            }
        }
    }

    pub fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event, .. } => {
                let serial = SERIAL_COUNTER.next_serial();
                let time = Event::time_msec(&event);
                let is_pressed = event.state() == KeyState::Pressed;

                let intercepted = self.seat.get_keyboard().unwrap().input::<String, _>(
                    self,
                    event.key_code(),
                    event.state(),
                    serial,
                    time,
                    |state, modifiers, handle| {
                        if is_pressed {
                            let sym = handle.modified_sym();
                            let cfg = state.config.lock().unwrap();
                            for (combo, action) in &cfg.keybinds {
                                if matches_keybind(combo, modifiers, sym) {
                                    return FilterResult::Intercept(action.clone());
                                }
                            }
                        }
                        FilterResult::Forward
                    },
                );

                if let Some(action) = intercepted {
                    self.execute_action(&action);
                }
            }
            InputEvent::PointerMotion { event, .. } => {
                let Some(pointer) = self.seat.get_pointer() else {
                    return;
                };
                let mut bounds: Option<(f64, f64, f64, f64)> = None;
                for output in self.space.outputs() {
                    let Some(geometry) = self.space.output_geometry(output) else {
                        continue;
                    };
                    let left = f64::from(geometry.loc.x);
                    let top = f64::from(geometry.loc.y);
                    let right = left + f64::from(geometry.size.w);
                    let bottom = top + f64::from(geometry.size.h);
                    bounds = Some(match bounds {
                        Some((min_x, min_y, max_x, max_y)) => (
                            min_x.min(left),
                            min_y.min(top),
                            max_x.max(right),
                            max_y.max(bottom),
                        ),
                        None => (left, top, right, bottom),
                    });
                }
                let Some((left, top, right, bottom)) = bounds else {
                    return;
                };
                let mut location = pointer.current_location() + event.delta();
                location.x = location.x.clamp(left, right - 1.0);
                location.y = location.y.clamp(top, bottom - 1.0);

                let serial = SERIAL_COUNTER.next_serial();
                let under = self.surface_under(location);
                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerMotionAbsolute { event, .. } => {
                let output = self.space.outputs().next().unwrap();
                let output_geo = self.space.output_geometry(output).unwrap();
                let pos = event.position_transformed(output_geo.size) + output_geo.loc.to_f64();
                let serial = SERIAL_COUNTER.next_serial();
                let pointer = self.seat.get_pointer().unwrap();
                let under = self.surface_under(pos);

                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerButton { event, .. } => {
                let pointer = self.seat.get_pointer().unwrap();
                let keyboard = self.seat.get_keyboard().unwrap();
                let serial = SERIAL_COUNTER.next_serial();
                let button = event.button_code();
                let button_state = event.state();
                let super_held = keyboard.modifier_state().logo;
                let pos = pointer.current_location();
                let under = self.surface_under(pos);
                pointer.motion(
                    self,
                    under,
                    &MotionEvent {
                        location: pos,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);

                info!(
                    button,
                    ?button_state,
                    focused = pointer.current_focus().is_some(),
                    "Pointer button event"
                );
                if button_state == ButtonState::Pressed && pointer.current_focus().is_none() {
                    warn!(button, "Pointer button pressed without a focused surface");
                }

                if ButtonState::Pressed == button_state && !pointer.is_grabbed() {
                    if let Some((window, loc)) = self
                        .space
                        .element_under(pos)
                        .map(|(w, l)| (w.clone(), l))
                    {
                        self.space.raise_element(&window, true);
                        window.set_activated(true);
                        let wl_surf = window.toplevel().unwrap().wl_surface().clone();
                        keyboard.set_focus(self, Some(wl_surf.clone()), serial);
                        self.space.elements().for_each(|w| {
                            if w != &window {
                                w.set_activated(false);
                            }
                            w.toplevel().unwrap().send_pending_configure();
                        });

                        // Kebebasan Hyprland: SUPER + Klik Kiri (Geser) / Klik Kanan (Resize) dari mana saja!
                        if super_held && (button == 0x110 || button == 0x111) {
                            let start_data = PointerGrabStartData {
                                focus: Some((wl_surf, loc.to_f64())),
                                button,
                                location: pos,
                            };

                            if button == 0x110 {
                                let grab = MoveSurfaceGrab {
                                    start_data,
                                    window,
                                    initial_window_location: loc,
                                };
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                                return;
                            } else if button == 0x111 {
                                let size = window.geometry().size;
                                if let Some(toplevel) = window.toplevel() {
                                    toplevel.with_pending_state(|s| {
                                        s.states.set(xdg_toplevel::State::Resizing);
                                    });
                                    toplevel.send_pending_configure();
                                }
                                let grab = ResizeSurfaceGrab::start(
                                    start_data,
                                    window,
                                    xdg_toplevel::ResizeEdge::BottomRight.into(),
                                    Rectangle::new(loc, size),
                                );
                                pointer.set_grab(self, grab, serial, Focus::Clear);
                                return;
                            }
                        }
                    } else if let Some((layer_surface, _)) = self.surface_under(pos) {
                        keyboard.set_focus(self, Some(layer_surface), serial);
                    } else {
                        self.space.elements().for_each(|window| {
                            window.set_activated(false);
                            window.toplevel().unwrap().send_pending_configure();
                        });
                        keyboard.set_focus(self, Option::<WlSurface>::None, serial);
                    }
                };

                pointer.button(
                    self,
                    &ButtonEvent {
                        button,
                        state: button_state,
                        serial,
                        time: event.time_msec(),
                    },
                );
                pointer.frame(self);
            }
            InputEvent::PointerAxis { event, .. } => {
                let source = event.source();

                let horizontal_amount = event.amount(Axis::Horizontal).unwrap_or_else(|| {
                    event.amount_v120(Axis::Horizontal).unwrap_or(0.0) * 15.0 / 120.
                });
                let vertical_amount = event.amount(Axis::Vertical).unwrap_or_else(|| {
                    event.amount_v120(Axis::Vertical).unwrap_or(0.0) * 15.0 / 120.
                });
                let horizontal_amount_discrete = event.amount_v120(Axis::Horizontal);
                let vertical_amount_discrete = event.amount_v120(Axis::Vertical);

                let mut frame = AxisFrame::new(event.time_msec()).source(source);
                if horizontal_amount != 0.0 {
                    frame = frame.value(Axis::Horizontal, horizontal_amount);
                    if let Some(discrete) = horizontal_amount_discrete {
                        frame = frame.v120(Axis::Horizontal, discrete as i32);
                    }
                }
                if vertical_amount != 0.0 {
                    frame = frame.value(Axis::Vertical, vertical_amount);
                    if let Some(discrete) = vertical_amount_discrete {
                        frame = frame.v120(Axis::Vertical, discrete as i32);
                    }
                }

                if source == AxisSource::Finger {
                    if event.amount(Axis::Horizontal) == Some(0.0) {
                        frame = frame.stop(Axis::Horizontal);
                    }
                    if event.amount(Axis::Vertical) == Some(0.0) {
                        frame = frame.stop(Axis::Vertical);
                    }
                }

                let pointer = self.seat.get_pointer().unwrap();
                pointer.axis(self, frame);
                pointer.frame(self);
            }
            _ => {}
        }
    }
}
