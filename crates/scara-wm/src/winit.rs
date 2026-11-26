use std::{
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use smithay::{
    backend::{
        renderer::damage::OutputDamageTracker,
        renderer::gles::GlesRenderer,
        winit::{self, WinitEvent, WinitGraphicsBackend},
    },
    desktop::layer_map_for_output,
    output::{Mode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};
use tracing::{info, warn};

use crate::{
    display::{
        self, DisplayAction, DisplayConfig, DisplayIpcRequest, DisplayMode, DisplayOutputInfo,
        PendingPreview,
    },
    glass_renderer::GlassPipeline,
    CalloopData, Smallvil,
};

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
    display_requests: Receiver<DisplayIpcRequest>,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let saved_configs = match display::load_configs() {
        Ok(configs) => configs,
        Err(error) => {
            warn!(%error, "Could not load saved Winit display settings; using defaults");
            Vec::new()
        }
    };
    let saved = saved_configs.iter().find(|config| config.name == "winit");
    if saved.is_some_and(|config| {
        !(640..=7680).contains(&config.mode.width) || !(480..=4320).contains(&config.mode.height)
    }) {
        warn!("Saved Winit window size is outside supported bounds; using defaults");
    }
    let saved = saved.filter(|config| {
        (640..=7680).contains(&config.mode.width) && (480..=4320).contains(&config.mode.height)
    });
    let (mut backend, winit) = winit::init()?;
    if let Some(config) = saved {
        let _ = backend
            .window()
            .request_inner_size(::winit::dpi::PhysicalSize::<u32>::new(
                u32::from(config.mode.width),
                u32::from(config.mode.height),
            ));
    }

    let mut glass_pipeline = GlassPipeline::new(backend.renderer())?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: saved
            .map(|config| config.mode.refresh_millihz as i32)
            .unwrap_or(60_000),
    };
    let scale = saved.map(|config| config.scale).unwrap_or(1);
    let location = saved.map(|config| (config.x, config.y)).unwrap_or((0, 0));

    let output = Output::new(
        "winit".to_string(),
        PhysicalProperties {
            size: (0, 0).into(),
            subpixel: Subpixel::Unknown,
            make: "Smithay".into(),
            model: "Winit".into(),
        },
    );
    let _global = output.create_global::<Smallvil>(display_handle);
    output.change_current_state(
        Some(mode),
        Some(Transform::Flipped180),
        Some(Scale::Integer(scale)),
        Some(location.into()),
    );
    output.set_preferred(mode);

    state.space.map_output(&output, location);

    let mut damage_tracker = OutputDamageTracker::from_output(&output);
    let mut pending_preview: Option<PendingPreview> = None;

    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);

    event_loop
        .handle()
        .insert_source(winit, move |event, _, data| {
            while let Ok(request) = display_requests.try_recv() {
                let result = handle_display_request(
                    &request.command,
                    &mut backend,
                    &output,
                    &mut data.state,
                    &mut pending_preview,
                );
                let _ = request.reply.send(result);
            }
            revert_expired_preview(&mut backend, &output, &mut data.state, &mut pending_preview);
            let display = &mut data.display_handle;
            let state = &mut data.state;

            match event {
                WinitEvent::Resized { size, .. } => {
                    output.change_current_state(
                        Some(Mode {
                            size,
                            refresh: 60_000,
                        }),
                        None,
                        None,
                        None,
                    );
                    layer_map_for_output(&output).arrange();
                    damage_tracker = OutputDamageTracker::from_output(&output);
                }
                WinitEvent::Input(event) => state.process_input_event(event),
                WinitEvent::Redraw => {
                    let mut active_springs = Vec::new();
                    for mut anim in std::mem::take(&mut state.window_springs) {
                        let mx = anim.spring_x.step(1.0 / 60.0);
                        let my = anim.spring_y.step(1.0 / 60.0);
                        if state.space.element_location(&anim.window).is_some() {
                            let nx = anim.spring_x.value.round() as i32;
                            let ny = anim.spring_y.value.round() as i32;
                            state
                                .space
                                .map_element(anim.window.clone(), (nx, ny), false);
                        }
                        if mx || my {
                            active_springs.push(anim);
                        }
                    }
                    state.window_springs = active_springs;

                    let size = backend.window_size();
                    let damage = Rectangle::from_size(size);
                    let (refraction, blur_passes, reduce_transparency) = {
                        let cfg = state.config.lock().unwrap();
                        (
                            cfg.glass_refraction,
                            cfg.blur_passes,
                            cfg.reduce_transparency,
                        )
                    };
                    let cursor_location = state
                        .seat
                        .get_pointer()
                        .map(|pointer| pointer.current_location())
                        .and_then(|location| {
                            let geometry = state.space.output_geometry(&output)?;
                            let inside = location.x >= f64::from(geometry.loc.x)
                                && location.y >= f64::from(geometry.loc.y)
                                && location.x < f64::from(geometry.loc.x + geometry.size.w)
                                && location.y < f64::from(geometry.loc.y + geometry.size.h);
                            inside.then(|| {
                                (location - geometry.loc.to_f64()).to_physical_precise_round(1.0)
                            })
                        });

                    {
                        let (renderer, mut framebuffer) = backend.bind().unwrap();
                        glass_pipeline
                            .render_frame(
                                renderer,
                                &mut framebuffer,
                                &output,
                                &state.space,
                                &mut damage_tracker,
                                size,
                                refraction,
                                blur_passes,
                                reduce_transparency,
                                cursor_location,
                            )
                            .unwrap();
                    }
                    backend.submit(Some(&[damage])).unwrap();

                    state.space.elements().for_each(|window| {
                        window.send_frame(
                            &output,
                            state.start_time.elapsed(),
                            Some(Duration::ZERO),
                            |_, _| Some(output.clone()),
                        )
                    });

                    {
                        let map = layer_map_for_output(&output);
                        for layer in map.layers() {
                            layer.send_frame(
                                &output,
                                state.start_time.elapsed(),
                                Some(Duration::ZERO),
                                |_, _| Some(output.clone()),
                            );
                        }
                    }

                    state.space.refresh();
                    let cur_count = state.space.elements().count();
                    if cur_count != state.last_window_count {
                        state.last_window_count = cur_count;
                        let layout = state.config.lock().unwrap().default_layout.clone();
                        if layout == "tiling" {
                            state.apply_tiling_layout();
                        }
                    }
                    state.popups.cleanup();
                    let _ = display.flush_clients();

                    backend.window().request_redraw();
                }
                WinitEvent::CloseRequested => {
                    tracing::info!("CloseRequested received — shutting down MARCH SCARA...");
                    state.loop_signal.stop();
                }
                _ => (),
            };
        })?;

    Ok(())
}

fn display_mode_from_output(mode: Mode) -> DisplayMode {
    DisplayMode {
        width: mode.size.w.clamp(1, i32::from(u16::MAX)) as u16,
        height: mode.size.h.clamp(1, i32::from(u16::MAX)) as u16,
        refresh_millihz: mode.refresh.max(0) as u32,
    }
}

fn current_winit_config(output: &Output) -> Option<DisplayConfig> {
    let mode = output.current_mode()?;
    let location = output.current_location();
    Some(DisplayConfig {
        name: output.name(),
        mode: display_mode_from_output(mode),
        scale: output.current_scale().integer_scale(),
        x: location.x,
        y: location.y,
    })
}

fn winit_display_outputs(output: &Output) -> Vec<DisplayOutputInfo> {
    let Some(current) = output.current_mode() else {
        return Vec::new();
    };
    let current_mode = display_mode_from_output(current);
    let mut modes = vec![
        (800, 600),
        (1024, 768),
        (1280, 720),
        (1280, 800),
        (1366, 768),
        (1600, 900),
        (1920, 1080),
        (2560, 1440),
    ]
    .into_iter()
    .map(|(width, height)| DisplayMode {
        width,
        height,
        refresh_millihz: current_mode.refresh_millihz,
    })
    .collect::<Vec<_>>();
    if !modes.contains(&current_mode) {
        modes.push(current_mode.clone());
    }
    let location = output.current_location();
    vec![DisplayOutputInfo {
        name: output.name(),
        backend: "winit".into(),
        current: current_mode,
        modes,
        scale: output.current_scale().integer_scale(),
        x: location.x,
        y: location.y,
    }]
}

fn handle_display_request(
    command: &str,
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    output: &Output,
    state: &mut Smallvil,
    pending_preview: &mut Option<PendingPreview>,
) -> String {
    let action = match display::parse_command(command) {
        Ok(action) => action,
        Err(error) => return format!("ERROR: {error}"),
    };
    match action {
        DisplayAction::List => serde_json::to_string(&winit_display_outputs(output))
            .unwrap_or_else(|error| format!("ERROR: Could not serialize display list: {error}")),
        DisplayAction::Preview(config) => {
            if pending_preview.is_some() {
                return "ERROR: Confirm or cancel the current display preview first.".into();
            }
            if config.name != output.name() {
                return format!("ERROR: Unknown Winit output: {}", config.name);
            }
            if !(640..=7680).contains(&config.mode.width)
                || !(480..=4320).contains(&config.mode.height)
            {
                return "ERROR: Winit window size must be between 640x480 and 7680x4320.".into();
            }
            let Some(previous) = current_winit_config(output) else {
                return "ERROR: Winit output has no current mode.".into();
            };
            if config.mode.refresh_millihz != previous.mode.refresh_millihz {
                return "ERROR: Winit cannot change the host display refresh rate.".into();
            }
            match apply_winit_config(backend, output, state, &config) {
                Ok(()) => {
                    *pending_preview = Some(PendingPreview {
                        previous,
                        expires_at: Instant::now()
                            + Duration::from_secs(display::PREVIEW_TIMEOUT_SECS),
                    });
                    format!(
                        "OK: Winit window preview active for {} seconds; confirm to keep it.",
                        display::PREVIEW_TIMEOUT_SECS
                    )
                }
                Err(error) => format!("ERROR: Could not resize Winit window: {error}"),
            }
        }
        DisplayAction::Confirm => {
            if pending_preview.is_none() {
                return "ERROR: No display preview is awaiting confirmation.".into();
            }
            let Some(config) = current_winit_config(output) else {
                return "ERROR: Winit output has no current mode.".into();
            };
            match display::update_configs(&[config]) {
                Ok(()) => {
                    *pending_preview = None;
                    "OK: Winit window size and layout saved.".into()
                }
                Err(error) => format!("ERROR: Could not save Winit display settings: {error}"),
            }
        }
        DisplayAction::Cancel => {
            let Some(preview) = pending_preview.take() else {
                return "ERROR: No display preview is active.".into();
            };
            match apply_winit_config(backend, output, state, &preview.previous) {
                Ok(()) => "OK: Previous Winit window settings restored.".into(),
                Err(error) => {
                    *pending_preview = Some(preview);
                    format!("ERROR: Could not restore Winit window settings: {error}")
                }
            }
        }
    }
}

fn apply_winit_config(
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    output: &Output,
    state: &mut Smallvil,
    config: &DisplayConfig,
) -> Result<(), String> {
    display::validate_config(config).map_err(|error| error.to_string())?;
    if config.name != output.name() {
        return Err(format!("Unknown Winit output: {}", config.name));
    }
    if !(640..=7680).contains(&config.mode.width) || !(480..=4320).contains(&config.mode.height) {
        return Err("Winit window size must be between 640x480 and 7680x4320.".into());
    }

    let _ = backend
        .window()
        .request_inner_size(::winit::dpi::PhysicalSize::new(
            u32::from(config.mode.width),
            u32::from(config.mode.height),
        ));
    let mode = Mode {
        size: (i32::from(config.mode.width), i32::from(config.mode.height)).into(),
        refresh: config.mode.refresh_millihz as i32,
    };
    output.change_current_state(
        Some(mode),
        None,
        Some(Scale::Integer(config.scale)),
        Some((config.x, config.y).into()),
    );
    state.space.map_output(output, (config.x, config.y));
    layer_map_for_output(output).arrange();
    Ok(())
}

fn revert_expired_preview(
    backend: &mut WinitGraphicsBackend<GlesRenderer>,
    output: &Output,
    state: &mut Smallvil,
    pending_preview: &mut Option<PendingPreview>,
) {
    let expired = pending_preview
        .as_ref()
        .is_some_and(|preview| Instant::now() >= preview.expires_at);
    if !expired {
        return;
    }
    let mut preview = pending_preview.take().expect("preview was checked");
    match apply_winit_config(backend, output, state, &preview.previous) {
        Ok(()) => info!("Unconfirmed Winit display preview reverted"),
        Err(error) => {
            warn!(%error, "Failed to revert unconfirmed Winit display preview");
            preview.expires_at = Instant::now() + Duration::from_secs(1);
            *pending_preview = Some(preview);
        }
    }
}
