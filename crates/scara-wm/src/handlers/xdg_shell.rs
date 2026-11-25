use smithay::{
    delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, layer_map_for_output, PopupKind, PopupManager, Space, Window, WindowSurfaceType},
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_seat, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Rectangle, Serial, SERIAL_COUNTER},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    grabs::{MoveSurfaceGrab, ResizeSurfaceGrab},
    Smallvil,
};

impl XdgShellHandler for Smallvil {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        let layout = self.config.lock().unwrap().default_layout.clone();
        let offset = (self.space.elements().count() as i32) * 28;
        let x = 140 + offset;
        let y = 74 + offset;
        self.space.map_element(window.clone(), (x, y + 48), true);

        if layout == "tiling" {
            self.apply_tiling_layout();
        } else {
            self.animate_window_to(window, x, y);
        }

        if self.space.elements().count() == 1 {
            let focus_surface = {
                self.space.elements().next().and_then(|window| {
                    window.set_activated(true);
                    window
                        .toplevel()
                        .map(|toplevel| toplevel.wl_surface().clone())
                })
            };
            if let Some(surface) = focus_surface {
                self.seat.get_keyboard().unwrap().set_focus(
                    self,
                    Some(surface),
                    SERIAL_COUNTER.next_serial(),
                );
            }
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();

            let grab = MoveSurfaceGrab {
                start_data,
                window,
                initial_window_location,
            };

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        if let Some(start_data) = check_grab(&seat, wl_surface, serial) {
            let pointer = seat.get_pointer().unwrap();

            let window = self
                .space
                .elements()
                .find(|w| w.toplevel().unwrap().wl_surface() == wl_surface)
                .unwrap()
                .clone();
            let initial_window_location = self.space.element_location(&window).unwrap();
            let initial_window_size = window.geometry().size;

            surface.with_pending_state(|state| {
                state.states.set(xdg_toplevel::State::Resizing);
            });

            surface.send_pending_configure();

            let grab = ResizeSurfaceGrab::start(
                start_data,
                window,
                edges.into(),
                Rectangle::new(initial_window_location, initial_window_size),
            );

            pointer.set_grab(self, grab, serial, Focus::Clear);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let win_opt = self
            .space
            .elements()
            .find(|w| w.toplevel().map(|t| t == &surface).unwrap_or(false))
            .cloned();
        if let Some(win) = win_opt {
            self.space.unmap_elem(&win);
            self.window_springs.retain(|a| a.window != win);
        }
    }
}

// Xdg Shell
delegate_xdg_shell!(Smallvil);

fn check_grab(
    seat: &Seat<Smallvil>,
    surface: &WlSurface,
    serial: Serial,
) -> Option<PointerGrabStartData<Smallvil>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == surface)
        .cloned()
    {
        let initial_configure_sent = with_states(surface, |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .unwrap()
                .lock()
                .unwrap()
                .initial_configure_sent
        });

        if !initial_configure_sent {
            window.toplevel().unwrap().send_configure();
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                unconstrain_popup_in_space(space, xdg);
                if !xdg.is_initial_configure_sent() {
                    xdg.send_configure().expect("initial configure failed");
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl Smallvil {
    fn unconstrain_popup(&self, popup: &PopupSurface) {
        unconstrain_popup_in_space(&self.space, popup);
    }
}

fn unconstrain_popup_in_space(space: &Space<Window>, popup: &PopupSurface) {
    let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
        return;
    };
    let Some(output) = space.outputs().next() else {
        return;
    };
    let Some(output_geo) = space.output_geometry(output) else {
        return;
    };

    let parent_loc = if let Some(window) = space
        .elements()
        .find(|w| w.toplevel().unwrap().wl_surface() == &root)
    {
        space.element_geometry(window).unwrap().loc
    } else {
        let map = layer_map_for_output(output);
        let Some(layer) = map.layer_for_surface(&root, WindowSurfaceType::ALL) else {
            return;
        };
        let Some(layer_geo) = map.layer_geometry(layer) else {
            return;
        };
        layer_geo.loc
    };

    let toplevel_coords = get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
    let mut target = output_geo;
    target.loc.x += 6;
    target.size.w = (target.size.w - 12).max(1);
    target.loc -= toplevel_coords;
    target.loc -= parent_loc;

    popup.with_pending_state(|state| {
        let mut geo = state.positioner.get_unconstrained_geometry(target);
        let min_x = output_geo.loc.x + 6 - parent_loc.x - toplevel_coords.x;
        let max_x = (output_geo.loc.x + output_geo.size.w - 6 - geo.size.w)
            - parent_loc.x
            - toplevel_coords.x;
        let min_y = output_geo.loc.y + 4 - parent_loc.y - toplevel_coords.y;
        let max_y = (output_geo.loc.y + output_geo.size.h - 6 - geo.size.h)
            - parent_loc.y
            - toplevel_coords.y;

        if max_x >= min_x {
            geo.loc.x = geo.loc.x.clamp(min_x, max_x);
        } else {
            geo.loc.x = min_x;
        }
        if max_y >= min_y {
            geo.loc.y = geo.loc.y.clamp(min_y, max_y);
        }
        state.geometry = geo;
    });
}
