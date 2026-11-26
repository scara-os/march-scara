use std::{
    collections::HashSet,
    error::Error,
    path::{Path, PathBuf},
    time::Duration,
};

use smithay::{
    backend::{
        allocator::{
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{DrmDevice, DrmDeviceFd, DrmDeviceNotifier, DrmEvent, GbmBufferedSurface},
        egl::{EGLContext, EGLDisplay},
        input::InputEvent,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        renderer::{damage::OutputDamageTracker, gles::GlesRenderer, Bind, ImportMemWl},
        session::{
            libseat::{LibSeatSession, LibSeatSessionNotifier},
            Event as SessionEvent, Session,
        },
        udev::all_gpus,
    },
    desktop::layer_map_for_output,
    output::{Mode as WlMode, Output, PhysicalProperties, Scale, Subpixel},
    reexports::{
        calloop::{
            timer::{TimeoutAction, Timer},
            EventLoop,
        },
        drm::control::{connector, crtc, Device as ControlDevice, Mode, ModeTypeFlags},
        input::{DeviceCapability, Libinput},
        rustix::fs::OFlags,
    },
    utils::{DeviceFd, Physical, Rectangle, Size},
};
use tracing::{error, info, warn};

use crate::{
    display::{self, DisplayAction, DisplayConfig, DisplayMode, DisplayOutputInfo, PendingPreview},
    glass_renderer::GlassPipeline,
    CalloopData, Smallvil,
};

type DrmSurface = GbmBufferedSurface<GbmAllocator<DrmDeviceFd>, ()>;
type OpenedDrmDevice = (
    DrmDevice,
    DrmDeviceNotifier,
    Vec<ConnectedOutput>,
    GbmDevice<DrmDeviceFd>,
);
type OpenDrmDeviceResult = Result<OpenedDrmDevice, Box<dyn Error>>;

struct ConnectedOutput {
    connector: connector::Handle,
    crtc: crtc::Handle,
    mode: Mode,
    preferred_mode: Mode,
    modes: Vec<Mode>,
    name: String,
    physical_size: (i32, i32),
    subpixel: Subpixel,
    scale: i32,
    location: (i32, i32),
}

struct OutputState {
    output: Output,
    crtc: crtc::Handle,
    surface: DrmSurface,
    modes: Vec<Mode>,
    damage_tracker: OutputDamageTracker,
    pipeline: GlassPipeline,
    size: Size<i32, Physical>,
}

pub struct DrmBackend {
    session: LibSeatSession,
    libinput: Libinput,
    drm_device: DrmDevice,
    renderer: GlesRenderer,
    outputs: Vec<OutputState>,
    pending_preview: Option<PendingPreview>,
}

impl DrmBackend {
    pub fn initialize(
        event_loop: &mut EventLoop<CalloopData>,
        data: &mut CalloopData,
        session: LibSeatSession,
        session_notifier: LibSeatSessionNotifier,
    ) -> Result<(), Box<dyn Error>> {
        let seat_name = session.seat();
        let saved_configs = match display::load_configs() {
            Ok(configs) => configs,
            Err(error) => {
                warn!(%error, "Could not load saved display configuration; using defaults");
                Vec::new()
            }
        };
        let (mut drm_device, drm_notifier, connected_outputs, gbm) =
            open_display_device(&session, &saved_configs)?;

        let egl_display = unsafe { EGLDisplay::new(gbm.clone())? };
        let context = EGLContext::new(&egl_display)?;
        let mut renderer = unsafe { GlesRenderer::new(context)? };
        data.state.shm_state.update_formats(renderer.shm_formats());

        let allocator = GbmAllocator::new(
            gbm.clone(),
            GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
        );
        let mut outputs = Vec::with_capacity(connected_outputs.len());
        let mut x = 0;

        for connected in connected_outputs {
            let wl_mode = WlMode::from(connected.mode);
            let (width, height) = connected.mode.size();
            let size: Size<i32, Physical> = (i32::from(width), i32::from(height)).into();
            let output = Output::new(
                connected.name,
                PhysicalProperties {
                    size: connected.physical_size.into(),
                    subpixel: connected.subpixel,
                    make: "Unknown".into(),
                    model: "Unknown".into(),
                },
            );
            output.create_global::<Smallvil>(&data.display_handle);
            output.set_preferred(WlMode::from(connected.preferred_mode));
            for mode in &connected.modes {
                output.add_mode(WlMode::from(*mode));
            }
            output.change_current_state(
                Some(wl_mode),
                None,
                Some(Scale::Integer(connected.scale)),
                Some(connected.location.into()),
            );
            data.state.space.map_output(&output, connected.location);
            x = x.max(connected.location.0.saturating_add(size.w));

            let drm_surface = drm_device.create_surface(
                connected.crtc,
                connected.mode,
                &[connected.connector],
            )?;
            let surface = DrmSurface::new(
                drm_surface,
                allocator.clone(),
                &[Fourcc::Abgr8888, Fourcc::Argb8888],
                renderer
                    .egl_context()
                    .dmabuf_render_formats()
                    .iter()
                    .copied(),
            )?;
            let pipeline = GlassPipeline::new(&mut renderer)?;
            let damage_tracker = OutputDamageTracker::from_output(&output);

            info!(
                output = %output.name(),
                width = size.w,
                height = size.h,
                "Initialized DRM output"
            );

            outputs.push(OutputState {
                output,
                crtc: connected.crtc,
                surface,
                modes: connected.modes,
                damage_tracker,
                pipeline,
                size,
            });
        }

        let mut libinput = Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(
            session.clone().into(),
        );
        libinput.udev_assign_seat(&seat_name).map_err(|_| {
            std::io::Error::other(format!("Failed to assign libinput to seat {seat_name}"))
        })?;
        let libinput_backend = LibinputInputBackend::new(libinput.clone());

        data.drm_backend = Some(Self {
            session,
            libinput,
            drm_device,
            renderer,
            outputs,
            pending_preview: None,
        });

        event_loop
            .handle()
            .insert_source(libinput_backend, |event, _, data| {
                if let InputEvent::DeviceAdded { device } = &event {
                    if device.has_capability(DeviceCapability::Keyboard) {
                        info!(device = %device.name(), "Keyboard added");
                    }
                    let mut tap_device = device.clone();
                    if tap_device.config_tap_finger_count() > 0 {
                        match tap_device.config_tap_set_enabled(true) {
                            Ok(()) => info!(device = %device.name(), "Enabled touchpad tap-to-click"),
                            Err(err) => {
                                warn!(device = %device.name(), ?err, "Failed to enable touchpad tap-to-click");
                            }
                        }
                    }
                }
                data.state.process_input_event(event);
            })?;

        event_loop
            .handle()
            .insert_source(drm_notifier, |event, _, data| match event {
                DrmEvent::VBlank(crtc) => {
                    if let Some(output) = data
                        .drm_backend
                        .as_mut()
                        .and_then(|backend| backend.outputs.iter_mut().find(|out| out.crtc == crtc))
                    {
                        if let Err(err) = output.surface.frame_submitted() {
                            error!(?crtc, %err, "Failed to acknowledge DRM frame");
                            data.state.loop_signal.stop();
                        }
                    }
                }
                DrmEvent::Error(err) => {
                    error!(%err, "DRM event error");
                    data.state.loop_signal.stop();
                }
            })?;

        event_loop
            .handle()
            .insert_source(session_notifier, |event, _, data| {
                let Some(backend) = data.drm_backend.as_mut() else {
                    return;
                };
                match event {
                    SessionEvent::PauseSession => {
                        backend.libinput.suspend();
                        backend.drm_device.pause();
                        info!("Paused DRM session");
                    }
                    SessionEvent::ActivateSession => {
                        if let Err(err) = backend.drm_device.activate(false) {
                            error!(%err, "Failed to reactivate DRM device");
                            data.state.loop_signal.stop();
                            return;
                        }
                        if let Err(err) = backend.libinput.resume() {
                            error!(?err, "Failed to resume libinput");
                            data.state.loop_signal.stop();
                            return;
                        }
                        for output in &mut backend.outputs {
                            output.surface.reset_buffers();
                        }
                        info!("Resumed DRM session");
                    }
                }
            })?;

        event_loop.handle().insert_source(
            Timer::from_duration(Duration::from_millis(16)),
            |_, _, data| {
                let Some(backend) = data.drm_backend.as_mut() else {
                    return TimeoutAction::Drop;
                };
                backend.revert_expired_display_preview(&mut data.state);
                if backend.session.is_active() {
                    if let Err(err) = backend.render_all(&mut data.state) {
                        error!(%err, "DRM rendering failed");
                        data.state.loop_signal.stop();
                        return TimeoutAction::Drop;
                    }
                    if let Err(err) = data.display_handle.flush_clients() {
                        warn!(%err, "Failed to flush Wayland clients");
                    }
                }
                TimeoutAction::ToDuration(Duration::from_millis(16))
            },
        )?;

        Ok(())
    }

    pub fn handle_display_request(&mut self, command: &str, state: &mut Smallvil) -> String {
        let action = match display::parse_command(command) {
            Ok(action) => action,
            Err(error) => return format!("ERROR: {error}"),
        };
        match action {
            DisplayAction::List => {
                serde_json::to_string(&self.display_outputs()).unwrap_or_else(|error| {
                    format!("ERROR: Could not serialize display list: {error}")
                })
            }
            DisplayAction::Preview(config) => {
                if self.pending_preview.is_some() {
                    return "ERROR: Confirm or cancel the current display preview first.".into();
                }
                let Some(previous) = self.current_display_config(&config.name) else {
                    return format!("ERROR: Unknown display output: {}", config.name);
                };
                match self.apply_display_config(&config, state) {
                    Ok(()) => {
                        self.pending_preview = Some(PendingPreview {
                            previous,
                            expires_at: std::time::Instant::now()
                                + Duration::from_secs(display::PREVIEW_TIMEOUT_SECS),
                        });
                        format!(
                            "OK: Display preview active for {} seconds; confirm to keep it.",
                            display::PREVIEW_TIMEOUT_SECS
                        )
                    }
                    Err(error) => format!("ERROR: Could not apply display preview: {error}"),
                }
            }
            DisplayAction::Confirm => {
                if self.pending_preview.is_none() {
                    return "ERROR: No display preview is awaiting confirmation.".into();
                }
                match self.save_current_display_configs() {
                    Ok(()) => {
                        self.pending_preview = None;
                        "OK: Display configuration saved.".into()
                    }
                    Err(error) => format!("ERROR: Could not save display configuration: {error}"),
                }
            }
            DisplayAction::Cancel => {
                let Some(preview) = self.pending_preview.take() else {
                    return "ERROR: No display preview is active.".into();
                };
                match self.apply_display_config(&preview.previous, state) {
                    Ok(()) => "OK: Previous display configuration restored.".into(),
                    Err(error) => {
                        self.pending_preview = Some(preview);
                        format!("ERROR: Could not restore previous display configuration: {error}")
                    }
                }
            }
        }
    }

    pub fn revert_expired_display_preview(&mut self, state: &mut Smallvil) {
        let expired = self
            .pending_preview
            .as_ref()
            .is_some_and(|preview| std::time::Instant::now() >= preview.expires_at);
        if !expired {
            return;
        }
        let preview = self.pending_preview.take().expect("preview was checked");
        match self.apply_display_config(&preview.previous, state) {
            Ok(()) => info!("Unconfirmed display preview reverted"),
            Err(error) => {
                self.pending_preview = Some(preview);
                error!(%error, "Failed to revert unconfirmed display preview");
            }
        }
    }

    fn display_outputs(&self) -> Vec<DisplayOutputInfo> {
        self.outputs
            .iter()
            .filter_map(|output| {
                let current = output.output.current_mode()?;
                let location = output.output.current_location();
                let modes = output
                    .modes
                    .iter()
                    .copied()
                    .map(display_mode_from_drm)
                    .collect::<Vec<_>>();
                Some(DisplayOutputInfo {
                    name: output.output.name(),
                    backend: "drm".into(),
                    current: DisplayMode {
                        width: current.size.w.max(0) as u16,
                        height: current.size.h.max(0) as u16,
                        refresh_millihz: current.refresh.max(0) as u32,
                    },
                    modes,
                    scale: output.output.current_scale().integer_scale(),
                    x: location.x,
                    y: location.y,
                })
            })
            .collect()
    }

    fn current_display_config(&self, name: &str) -> Option<DisplayConfig> {
        let output = self
            .outputs
            .iter()
            .find(|output| output.output.name() == name)?;
        let mode = output.output.current_mode()?;
        let location = output.output.current_location();
        Some(DisplayConfig {
            name: name.to_string(),
            mode: DisplayMode {
                width: mode.size.w.max(0) as u16,
                height: mode.size.h.max(0) as u16,
                refresh_millihz: mode.refresh.max(0) as u32,
            },
            scale: output.output.current_scale().integer_scale(),
            x: location.x,
            y: location.y,
        })
    }

    fn apply_display_config(
        &mut self,
        config: &DisplayConfig,
        state: &mut Smallvil,
    ) -> Result<(), String> {
        display::validate_config(config).map_err(|error| error.to_string())?;
        let output = self
            .outputs
            .iter_mut()
            .find(|output| output.output.name() == config.name)
            .ok_or_else(|| format!("Unknown display output: {}", config.name))?;
        let mode = output
            .modes
            .iter()
            .copied()
            .find(|mode| display_mode_from_drm(*mode) == config.mode)
            .ok_or_else(|| format!("Mode is not supported by {}", config.name))?;
        output
            .surface
            .use_mode(mode)
            .map_err(|error| error.to_string())?;

        let wl_mode = WlMode::from(mode);
        output.output.set_preferred(wl_mode);
        output.output.change_current_state(
            Some(wl_mode),
            None,
            Some(Scale::Integer(config.scale)),
            Some((config.x, config.y).into()),
        );
        let (width, height) = mode.size();
        output.size = (i32::from(width), i32::from(height)).into();
        output.damage_tracker = OutputDamageTracker::from_output(&output.output);
        state.space.map_output(&output.output, (config.x, config.y));
        layer_map_for_output(&output.output).arrange();
        Ok(())
    }

    fn save_current_display_configs(&self) -> Result<(), String> {
        let configs = self
            .outputs
            .iter()
            .filter_map(|output| {
                let mode = output.output.current_mode()?;
                let location = output.output.current_location();
                Some(DisplayConfig {
                    name: output.output.name(),
                    mode: DisplayMode {
                        width: mode.size.w.max(0) as u16,
                        height: mode.size.h.max(0) as u16,
                        refresh_millihz: mode.refresh.max(0) as u32,
                    },
                    scale: output.output.current_scale().integer_scale(),
                    x: location.x,
                    y: location.y,
                })
            })
            .collect::<Vec<_>>();
        display::update_configs(&configs).map_err(|error| error.to_string())
    }

    fn render_all(&mut self, state: &mut Smallvil) -> Result<(), Box<dyn Error>> {
        state.space.refresh();
        let pointer_location = state
            .seat
            .get_pointer()
            .map(|pointer| pointer.current_location());

        for output in &mut self.outputs {
            let (refraction, blur_passes, reduce_transparency) = {
                let config = state.config.lock().unwrap();
                (
                    config.glass_refraction,
                    config.blur_passes,
                    config.reduce_transparency,
                )
            };
            let cursor_location = pointer_location.and_then(|location| {
                let geometry = state.space.output_geometry(&output.output)?;
                let inside = location.x >= f64::from(geometry.loc.x)
                    && location.y >= f64::from(geometry.loc.y)
                    && location.x < f64::from(geometry.loc.x + geometry.size.w)
                    && location.y < f64::from(geometry.loc.y + geometry.size.h);
                inside.then(|| (location - geometry.loc.to_f64()).to_physical_precise_round(1.0))
            });
            let (mut buffer, _) = output.surface.next_buffer()?;
            let mut framebuffer = self.renderer.bind(&mut buffer)?;
            output.pipeline.render_frame(
                &mut self.renderer,
                &mut framebuffer,
                &output.output,
                &state.space,
                &mut output.damage_tracker,
                output.size,
                refraction,
                blur_passes,
                reduce_transparency,
                cursor_location,
            )?;

            output
                .surface
                .queue_buffer(None, Some(vec![Rectangle::from_size(output.size)]), ())?;

            state.space.elements().for_each(|window| {
                window.send_frame(
                    &output.output,
                    state.start_time.elapsed(),
                    Some(Duration::ZERO),
                    |_, _| Some(output.output.clone()),
                )
            });

            let layer_map = smithay::desktop::layer_map_for_output(&output.output);
            for layer in layer_map.layers() {
                layer.send_frame(
                    &output.output,
                    state.start_time.elapsed(),
                    Some(Duration::ZERO),
                    |_, _| Some(output.output.clone()),
                );
            }
        }

        state.popups.cleanup();
        Ok(())
    }
}

fn open_display_device(
    session: &LibSeatSession,
    saved_configs: &[DisplayConfig],
) -> OpenDrmDeviceResult {
    let requested_device = std::env::var_os("SCARA_DRM_DEVICE").map(PathBuf::from);
    let candidates = match requested_device {
        Some(path) => vec![path],
        None => {
            let candidates = all_gpus(session.seat())?;
            let connected = candidates
                .iter()
                .filter_map(|path| match sysfs_has_connected_output(path) {
                    Ok(true) => Some(path.clone()),
                    Ok(false) => None,
                    Err(err) => {
                        warn!(device = %path.display(), %err, "Could not inspect connector status");
                        None
                    }
                })
                .collect::<Vec<_>>();
            if connected.is_empty() {
                candidates
            } else {
                connected
            }
        }
    };

    let mut failures = Vec::new();
    for path in candidates {
        match open_candidate(session, &path, saved_configs) {
            Ok((device, notifier, outputs, gbm)) if !outputs.is_empty() => {
                info!(device = %path.display(), outputs = outputs.len(), "Selected DRM device");
                return Ok((device, notifier, outputs, gbm));
            }
            Ok((_, _, _, _)) => {
                failures.push(format!("{} has no connected outputs", path.display()))
            }
            Err(err) => failures.push(format!("{}: {err}", path.display())),
        }
    }

    Err(std::io::Error::other(format!(
        "No usable DRM device with connected outputs was found. Set SCARA_DRM_DEVICE to a /dev/dri/cardN path. Details: {}",
        failures.join("; ")
    ))
    .into())
}

fn sysfs_has_connected_output(path: &Path) -> std::io::Result<bool> {
    let card = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| std::io::Error::other("DRM device path has no UTF-8 file name"))?;
    let connector_prefix = format!("{card}-");

    for entry in std::fs::read_dir("/sys/class/drm")? {
        let entry = entry?;
        if !entry
            .file_name()
            .to_string_lossy()
            .starts_with(&connector_prefix)
        {
            continue;
        }
        if std::fs::read_to_string(entry.path().join("status"))
            .is_ok_and(|status| status.trim() == "connected")
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn open_candidate(
    session: &LibSeatSession,
    path: &Path,
    saved_configs: &[DisplayConfig],
) -> OpenDrmDeviceResult {
    let mut session = session.clone();
    let fd = session.open(
        path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    let fd = DrmDeviceFd::new(DeviceFd::from(fd));
    let (device, notifier) = DrmDevice::new(fd.clone(), true)?;
    let outputs = connected_outputs(&device, saved_configs)?;
    if outputs.is_empty() {
        return Ok((device, notifier, outputs, GbmDevice::new(fd)?));
    }
    let gbm = GbmDevice::new(fd)?;
    Ok((device, notifier, outputs, gbm))
}

fn connected_outputs(
    device: &DrmDevice,
    saved_configs: &[DisplayConfig],
) -> Result<Vec<ConnectedOutput>, Box<dyn Error>> {
    let resources = device.resource_handles()?;
    let mut used_crtcs = HashSet::new();
    let mut outputs = Vec::new();
    let mut next_default_x = 0;

    for connector_handle in resources.connectors() {
        let Ok(connector) = device.get_connector(*connector_handle, true) else {
            continue;
        };
        if connector.state() != connector::State::Connected || connector.modes().is_empty() {
            continue;
        }

        let mut possible_crtcs = Vec::new();
        for encoder_handle in connector.encoders() {
            let Ok(encoder) = device.get_encoder(*encoder_handle) else {
                continue;
            };
            possible_crtcs.extend(resources.filter_crtcs(encoder.possible_crtcs()));
        }

        let current_crtc = connector
            .current_encoder()
            .and_then(|encoder| device.get_encoder(encoder).ok())
            .and_then(|encoder| encoder.crtc())
            .filter(|crtc| possible_crtcs.contains(crtc) && !used_crtcs.contains(crtc));
        let crtc = current_crtc.or_else(|| {
            possible_crtcs
                .into_iter()
                .find(|crtc| !used_crtcs.contains(crtc))
        });
        let Some(crtc) = crtc else {
            warn!(connector = %connector, "No free compatible CRTC for connected output");
            continue;
        };

        let modes = connector.modes().to_vec();
        let preferred_mode = modes
            .iter()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .copied()
            .unwrap_or(modes[0]);
        let name = format!(
            "{}-{}",
            connector.interface().as_str(),
            connector.interface_id()
        );
        let saved = saved_configs.iter().find(|config| config.name == name);
        let mode = saved
            .and_then(|config| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| display_mode_from_drm(*mode) == config.mode)
            })
            .unwrap_or(preferred_mode);
        if saved.is_some_and(|config| display_mode_from_drm(mode) != config.mode) {
            warn!(output = %name, "Saved display mode is unavailable; using the preferred mode");
        }
        let scale = saved.map(|config| config.scale).unwrap_or(1);
        let (width, _) = mode.size();
        let location = saved
            .map(|config| (config.x, config.y))
            .unwrap_or((next_default_x, 0));
        next_default_x = location.0.saturating_add(i32::from(width) / scale.max(1));
        let (width_mm, height_mm) = connector.size().unwrap_or((0, 0));
        used_crtcs.insert(crtc);
        outputs.push(ConnectedOutput {
            connector: *connector_handle,
            crtc,
            mode,
            preferred_mode,
            modes,
            name,
            physical_size: (width_mm as i32, height_mm as i32),
            subpixel: connector.subpixel().into(),
            scale,
            location,
        });
    }

    Ok(outputs)
}

fn display_mode_from_drm(mode: Mode) -> DisplayMode {
    let mode = WlMode::from(mode);
    DisplayMode {
        width: mode.size.w.max(0) as u16,
        height: mode.size.h.max(0) as u16,
        refresh_millihz: mode.refresh.max(0) as u32,
    }
}
