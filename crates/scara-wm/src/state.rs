use std::{
    ffi::OsString,
    sync::{Arc, Mutex},
};

use smithay::{
    desktop::{layer_map_for_output, PopupManager, Space, Window, WindowSurfaceType},
    input::{Seat, SeatState},
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, LoopSignal, Mode, PostAction},
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            protocol::wl_surface::WlSurface,
            Display, DisplayHandle,
        },
    },
    utils::{Logical, Point, Size},
    wayland::{
        compositor::{CompositorClientState, CompositorState},
        output::OutputManagerState,
        selection::data_device::DataDeviceState,
        shell::{
            wlr_layer::{Layer as WlrLayer, WlrLayerShellState},
            xdg::XdgShellState,
        },
        shm::ShmState,
        socket::ListeningSocketSource,
    },
};

use crate::{lua_bridge::ScaraConfig, physics::Spring, CalloopData};

pub struct WindowAnim {
    pub window: Window,
    pub spring_x: Spring,
    pub spring_y: Spring,
}

pub struct Smallvil {
    pub start_time: std::time::Instant,
    pub socket_name: OsString,
    pub display_handle: DisplayHandle,

    pub space: Space<Window>,
    pub loop_signal: LoopSignal,

    pub config_path: String,
    pub config: Arc<Mutex<ScaraConfig>>,
    pub window_springs: Vec<WindowAnim>,
    pub active_workspace: usize,
    pub workspaces: [Vec<(Window, Point<i32, Logical>)>; 5],
    pub last_window_count: usize,
    pub super_key_pressed: bool,
    pub super_combo_used: bool,

    pub compositor_state: CompositorState,
    pub xdg_shell_state: XdgShellState,
    pub layer_shell_state: WlrLayerShellState,
    pub shm_state: ShmState,
    pub output_manager_state: OutputManagerState,
    pub seat_state: SeatState<Smallvil>,
    pub data_device_state: DataDeviceState,
    pub popups: PopupManager,

    pub seat: Seat<Self>,
}

impl Smallvil {
    pub fn new(
        event_loop: &mut EventLoop<CalloopData>,
        display: Display<Self>,
        config: Arc<Mutex<ScaraConfig>>,
        config_path: String,
        seat_name: &str,
    ) -> Self {
        let start_time = std::time::Instant::now();
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let popups = PopupManager::default();

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, seat_name);
        seat.add_keyboard(Default::default(), 200, 25).unwrap();
        seat.add_pointer();

        let space = Space::default();
        let socket_name = Self::init_wayland_listener(display, event_loop);
        let loop_signal = event_loop.get_signal();

        Self {
            start_time,
            display_handle: dh,

            space,
            loop_signal,
            socket_name,
            config_path,
            config,
            window_springs: Vec::new(),
            active_workspace: 0,
            workspaces: Default::default(),
            last_window_count: 0,
            super_key_pressed: false,
            super_combo_used: false,

            compositor_state,
            xdg_shell_state,
            layer_shell_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            popups,
            seat,
        }
    }

    fn init_wayland_listener(
        display: Display<Smallvil>,
        event_loop: &mut EventLoop<CalloopData>,
    ) -> OsString {
        let listening_socket = ListeningSocketSource::new_auto().unwrap();
        let socket_name = listening_socket.socket_name().to_os_string();
        let loop_handle = event_loop.handle();

        loop_handle
            .insert_source(listening_socket, move |client_stream, _, state| {
                state
                    .display_handle
                    .insert_client(client_stream, Arc::new(ClientState::default()))
                    .unwrap();
            })
            .expect("Failed to init the wayland event source.");

        loop_handle
            .insert_source(
                Generic::new(display, Interest::READ, Mode::Level),
                |_, display, state| {
                    unsafe {
                        display
                            .get_mut()
                            .dispatch_clients(&mut state.state)
                            .unwrap();
                    }
                    Ok(PostAction::Continue)
                },
            )
            .unwrap();

        socket_name
    }

    pub fn animate_window_to(&mut self, win: Window, target_x: i32, target_y: i32) {
        let cur = self
            .space
            .element_location(&win)
            .unwrap_or_else(|| (target_x, target_y + 44).into());
        let mut sx = Spring::new(cur.x as f32, 340.0, 28.0);
        sx.target = target_x as f32;
        let mut sy = Spring::new(cur.y as f32, 340.0, 28.0);
        sy.target = target_y as f32;

        self.window_springs.retain(|a| a.window != win);
        self.window_springs.push(WindowAnim {
            window: win,
            spring_x: sx,
            spring_y: sy,
        });
    }

    pub fn apply_tiling_layout(&mut self) {
        let Some(output) = self.space.outputs().next().cloned() else {
            return;
        };
        let Some(out_geo) = self.space.output_geometry(&output) else {
            return;
        };

        let windows: Vec<Window> = self.space.elements().cloned().collect();
        let n = windows.len();
        if n == 0 {
            return;
        }

        let gap = 12;
        let top_pad = 38;
        let bottom_pad = 86;
        let x0 = gap;
        let y0 = top_pad;
        let w_total = (out_geo.size.w - gap * 2).max(200);
        let h_total = (out_geo.size.h - top_pad - bottom_pad).max(200);

        if n == 1 {
            let win = windows[0].clone();
            if let Some(toplevel) = win.toplevel() {
                toplevel.with_pending_state(|s| {
                    s.size = Some((w_total, h_total).into());
                });
                toplevel.send_configure();
            }
            self.animate_window_to(win, x0, y0);
        } else {
            let master_w = (w_total - gap) / 2;
            let stack_w = w_total - gap - master_w;
            let stack_count = (n - 1) as i32;
            let stack_h = ((h_total - gap * (stack_count - 1)) / stack_count).max(120);

            for (idx, win) in windows.into_iter().enumerate() {
                let (loc, size): (Point<i32, Logical>, Size<i32, Logical>) = if idx == 0 {
                    ((x0, y0).into(), (master_w, h_total).into())
                } else {
                    let row = (idx - 1) as i32;
                    (
                        (x0 + master_w + gap, y0 + row * (stack_h + gap)).into(),
                        (stack_w, stack_h).into(),
                    )
                };

                if let Some(toplevel) = win.toplevel() {
                    toplevel.with_pending_state(|s| {
                        s.size = Some(size);
                    });
                    toplevel.send_configure();
                }
                self.animate_window_to(win, loc.x, loc.y);
            }
        }
    }

    pub fn switch_workspace(&mut self, target_ws: usize) {
        if target_ws >= 5 || target_ws == self.active_workspace {
            return;
        }
        let cur_ws = self.active_workspace;
        let current_wins: Vec<(Window, Point<i32, Logical>)> = self
            .space
            .elements()
            .filter_map(|w| self.space.element_location(w).map(|loc| (w.clone(), loc)))
            .collect();

        for (w, _) in &current_wins {
            self.space.unmap_elem(w);
        }
        self.workspaces[cur_ws] = current_wins;

        let next_wins = std::mem::take(&mut self.workspaces[target_ws]);
        for (w, loc) in next_wins {
            self.space
                .map_element(w.clone(), (loc.x, loc.y + 36), false);
            self.animate_window_to(w, loc.x, loc.y);
        }
        self.active_workspace = target_ws;

        let layout = self.config.lock().unwrap().default_layout.clone();
        if layout == "tiling" {
            self.apply_tiling_layout();
        }
    }

    pub fn move_active_to_workspace(&mut self, target_ws: usize) {
        if target_ws >= 5 || target_ws == self.active_workspace {
            return;
        }
        let Some(win) = self.space.elements().last().cloned() else {
            return;
        };
        let loc = self
            .space
            .element_location(&win)
            .unwrap_or((140, 74).into());
        self.space.unmap_elem(&win);
        self.workspaces[target_ws].push((win, loc));

        let layout = self.config.lock().unwrap().default_layout.clone();
        if layout == "tiling" {
            self.apply_tiling_layout();
        }
    }

    pub fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(WlSurface, Point<f64, Logical>)> {
        if let Some(output) = self.space.outputs().next() {
            let map = layer_map_for_output(output);
            for level in [WlrLayer::Overlay, WlrLayer::Top] {
                if let Some(layer) = map.layer_under(level, pos) {
                    if let Some(geo) = map.layer_geometry(layer) {
                        if let Some((s, p)) =
                            layer.surface_under(pos - geo.loc.to_f64(), WindowSurfaceType::ALL)
                        {
                            return Some((s, (p + geo.loc).to_f64()));
                        }
                    }
                }
            }
        }

        self.space
            .element_under(pos)
            .and_then(|(window, location)| {
                window
                    .surface_under(pos - location.to_f64(), WindowSurfaceType::ALL)
                    .map(|(s, p)| (s, (p + location).to_f64()))
            })
    }
}

#[derive(Default)]
pub struct ClientState {
    pub compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
