use std::time::Duration;

use smithay::{
    backend::{
        renderer::damage::OutputDamageTracker,
        winit::{self, WinitEvent},
    },
    desktop::layer_map_for_output,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    reexports::calloop::EventLoop,
    utils::{Rectangle, Transform},
};

use crate::{glass_renderer::GlassPipeline, CalloopData, Smallvil};

pub fn init_winit(
    event_loop: &mut EventLoop<CalloopData>,
    data: &mut CalloopData,
) -> Result<(), Box<dyn std::error::Error>> {
    let display_handle = &mut data.display_handle;
    let state = &mut data.state;

    let (mut backend, winit) = winit::init()?;

    let mut glass_pipeline = GlassPipeline::new(backend.renderer())?;

    let mode = Mode {
        size: backend.window_size(),
        refresh: 60_000,
    };

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
    output.change_current_state(Some(mode), Some(Transform::Flipped180), None, Some((0, 0).into()));
    output.set_preferred(mode);

    state.space.map_output(&output, (0, 0));

    let mut damage_tracker = OutputDamageTracker::from_output(&output);

    std::env::set_var("WAYLAND_DISPLAY", &state.socket_name);

    event_loop.handle().insert_source(winit, move |event, _, data| {
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
                        state.space.map_element(anim.window.clone(), (nx, ny), false);
                    }
                    if mx || my {
                        active_springs.push(anim);
                    }
                }
                state.window_springs = active_springs;

                let size = backend.window_size();
                let damage = Rectangle::from_size(size);
                let (refraction, blur_passes) = {
                    let cfg = state.config.lock().unwrap();
                    (cfg.glass_refraction, cfg.blur_passes)
                };

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
