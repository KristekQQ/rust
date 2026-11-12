//! Native OpenXR host. Vulkan ownership/import is confined to this adapter.
use crate::app::{pose, projection, App};
use crate::measurement::{AppMetrics, FrameRecord, Recorder, Setup};
use ash::vk::{self, Handle};
use openxr as xr;
use std::error::Error;
use webgpu_wasm::render::target::{RenderTarget, RenderView, Viewport};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const STEREO: xr::ViewConfigurationType = xr::ViewConfigurationType::PRIMARY_STEREO;

pub use webgpu_wasm::frame_input::FrameInput;
pub trait HostApp {
    fn measurement_setup(&self) -> Option<Setup> {
        None
    }
    fn measurement_metrics(&self) -> AppMetrics {
        AppMetrics::default()
    }
    /// Opt in only when gameplay resets its own coordinate-dependent history.
    fn recenterable_floor(&self) -> bool {
        false
    }
    fn set_xr(&mut self, active: bool);
    fn set_refresh_hz(&mut self, hz: Option<f32>);
    fn reset_tracking(&mut self);
    fn reset_frame_clock(&mut self);
    fn toggle_info(&mut self) -> std::result::Result<(), String>;
    fn reference(&self) -> glam::Mat4;
    fn tick_input(&mut self, input: FrameInput) -> std::result::Result<(), String>;
    fn render(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
        slot: u32,
    ) -> std::result::Result<(), String>;
    fn frame_submitted(&mut self, work: std::time::Duration);
    fn take_haptics(&mut self) -> [f32; 2] {
        [0.; 2]
    }
}
// Optional recorder clocks do not run in ordinary builds without a run config.
struct MeasurementFrame {
    record: FrameRecord,
    stage_started: std::time::Instant,
}
impl MeasurementFrame {
    fn mark(&mut self, stage: usize) {
        let now = std::time::Instant::now();
        self.record.stages_ns[stage] = now.duration_since(self.stage_started).as_nanos() as u64;
        self.stage_started = now;
    }
    fn finish(
        mut self,
        recorder: &mut Recorder,
        origin: std::time::Instant,
        last: &mut Option<std::time::Instant>,
        app: &dyn HostApp,
        rendered: bool,
    ) {
        self.record.app = app.measurement_metrics();
        let now = std::time::Instant::now();
        self.record.monotonic_ns = now.duration_since(origin).as_nanos() as u64;
        if rendered {
            self.record.app_interval_ns = last.map(|t| now.duration_since(t).as_nanos() as u64);
            *last = Some(now);
        }
        recorder.record(self.record);
    }
}
impl HostApp for App {
    fn set_xr(&mut self, active: bool) {
        self.content.game.set_xr(active);
    }
    fn set_refresh_hz(&mut self, hz: Option<f32>) {
        self.info.refresh_hz = hz;
    }
    fn reset_tracking(&mut self) {
        self.content.player.reset_tracking(&mut self.renderer.scene);
    }
    fn reset_frame_clock(&mut self) {
        App::reset_frame_clock(self);
    }
    fn toggle_info(&mut self) -> std::result::Result<(), String> {
        App::toggle_info(self)
    }
    fn reference(&self) -> glam::Mat4 {
        self.content.game.xr_origin
    }
    fn tick_input(&mut self, input: FrameInput) -> std::result::Result<(), String> {
        self.tick_xr(
            input.dt,
            input.head,
            input.hands,
            input.rays[1],
            input.axes,
            input.triggers[1],
            input.primary,
        )
    }
    fn render(
        &mut self,
        view: RenderView,
        target: &RenderTarget<'_>,
        slot: u32,
    ) -> std::result::Result<(), String> {
        App::render(self, view, target, slot)
    }
    fn frame_submitted(&mut self, work: std::time::Duration) {
        App::frame_submitted(self, work);
    }
}
pub(crate) fn preferred_refresh_rate(rates: &[f32]) -> Option<f32> {
    [120., 90.]
        .into_iter()
        .find(|target| rates.iter().any(|hz| (*hz - target).abs() < 0.1))
}

pub(crate) fn floor_reference(
    types: &[xr::ReferenceSpaceType],
    local_floor: bool,
) -> Option<xr::ReferenceSpaceType> {
    if local_floor && types.contains(&xr::ReferenceSpaceType::LOCAL_FLOOR_EXT) {
        Some(xr::ReferenceSpaceType::LOCAL_FLOOR_EXT)
    } else if types.contains(&xr::ReferenceSpaceType::STAGE) {
        Some(xr::ReferenceSpaceType::STAGE)
    } else {
        None
    }
}

#[derive(Default)]
pub(crate) struct ReferenceChanges(std::collections::BTreeSet<i64>);
impl ReferenceChanges {
    pub(crate) fn schedule(&mut self, time: i64) {
        self.0.insert(time);
    }
    pub(crate) fn take_due(&mut self, time: i64) -> bool {
        let mut due = false;
        while self.0.first().is_some_and(|change| *change <= time) {
            self.0.pop_first();
            due = true;
        }
        due
    }
}

unsafe fn graphics(
    xr: &xr::Instance,
    system: xr::SystemId,
) -> Result<(
    wgpu::Instance,
    wgpu::Device,
    wgpu::Queue,
    ash::Instance,
    ash::Device,
    vk::PhysicalDevice,
    u32,
)> {
    let req = xr.graphics_requirements::<xr::Vulkan>(system)?;
    let version = vk::API_VERSION_1_1;
    if req.min_api_version_supported > xr::Version::new(1, 1, 0) {
        return Err("Runtime requires newer Vulkan than this prototype".into());
    }
    let entry = ash::Entry::load()?;
    let flags = wgpu::InstanceFlags::empty();
    let get_instance_proc_addr = entry.static_fn().get_instance_proc_addr;
    let extensions = wgpu::hal::vulkan::Instance::desired_extensions(&entry, version, flags)?;
    let ext: Vec<_> = extensions.iter().map(|e| e.as_ptr()).collect();
    let info = vk::ApplicationInfo::default().api_version(version);
    let create = vk::InstanceCreateInfo::default()
        .application_info(&info)
        .enabled_extension_names(&ext);
    let raw = xr
        .create_vulkan_instance(
            system,
            std::mem::transmute(entry.static_fn().get_instance_proc_addr),
            &create as *const _ as _,
        )?
        .map_err(vk::Result::from_raw)?;
    let vk_instance = ash::Instance::load(entry.static_fn(), vk::Instance::from_raw(raw as _));
    let physical = vk::PhysicalDevice::from_raw(xr.vulkan_graphics_device(system, raw)? as _);
    // HAL owns the instance; clones below only hold function tables, never destroy it.
    let hal = wgpu::hal::vulkan::Instance::from_raw(
        entry,
        vk_instance.clone(),
        version,
        32,
        None,
        extensions,
        flags,
        Default::default(),
        false,
        None,
    )?;
    let exposed = hal
        .expose_adapter(physical)
        .ok_or("OpenXR GPU unsupported by wgpu")?;
    let instance = wgpu::Instance::from_hal::<wgpu::hal::api::Vulkan>(hal);
    let adapter = instance.create_adapter_from_hal(exposed);
    let features = if cfg!(feature = "frame-profiling") {
        adapter.features()
            & (wgpu::Features::TIMESTAMP_QUERY | wgpu::Features::TIMESTAMP_QUERY_INSIDE_PASSES)
    } else {
        wgpu::Features::empty()
    };
    let limits = wgpu::Limits::default().using_resolution(adapter.limits());
    let hal_adapter = adapter
        .as_hal::<wgpu::hal::api::Vulkan>()
        .ok_or("Expected Vulkan adapter")?;
    let extensions = hal_adapter.required_device_extensions(features);
    let mut physical_features = hal_adapter.physical_device_features(&extensions, features);
    let ext: Vec<_> = extensions.iter().map(|e| e.as_ptr()).collect();
    // wgpu Vulkan adapter currently requires graphics+compute in queue family zero.
    let family = 0;
    let queues = vk_instance.get_physical_device_queue_family_properties(physical);
    if !queues.first().is_some_and(|q| {
        q.queue_flags
            .contains(vk::QueueFlags::GRAPHICS | vk::QueueFlags::COMPUTE)
    }) {
        return Err("Queue family zero cannot drive wgpu".into());
    }
    let queues = [vk::DeviceQueueCreateInfo::default()
        .queue_family_index(family)
        .queue_priorities(&[1.])];
    let create = physical_features.add_to_device_create(
        vk::DeviceCreateInfo::default()
            .queue_create_infos(&queues)
            .enabled_extension_names(&ext),
    );
    let raw = xr
        .create_vulkan_device(
            system,
            std::mem::transmute(get_instance_proc_addr),
            physical.as_raw() as _,
            &create as *const _ as _,
        )?
        .map_err(vk::Result::from_raw)?;
    let vk_device = ash::Device::load(vk_instance.fp_v1_0(), vk::Device::from_raw(raw as _));
    let memory = wgpu::MemoryHints::default();
    let hal_device = hal_adapter.device_from_raw(
        vk_device.clone(),
        None,
        &extensions,
        features,
        &limits,
        &memory,
        family,
        0,
    )?;
    drop(hal_adapter);
    let (device, queue) = adapter.create_device_from_hal(
        hal_device,
        &wgpu::DeviceDescriptor {
            label: Some("Rim OpenXR Vulkan"),
            required_features: features,
            required_limits: limits,
            ..Default::default()
        },
    )?;
    Ok((
        instance,
        device,
        queue,
        vk_instance,
        vk_device,
        physical,
        family,
    ))
}

// Declared after imported attachments: Drop runs before swapchain teardown on
// every exit, including a render/submit failure half-way through a stereo frame.
struct DrainGpu(wgpu::Device);
impl Drop for DrainGpu {
    fn drop(&mut self) {
        let _ = self.0.poll(wgpu::PollType::wait_indefinitely());
    }
}

struct Controls {
    set: xr::ActionSet,
    grip: xr::Action<xr::Posef>,
    aim: xr::Action<xr::Posef>,
    stick: xr::Action<xr::Vector2f>,
    trigger: xr::Action<f32>,
    haptic: xr::Action<xr::Haptic>,
    dash: xr::Action<bool>,
    info_toggle: xr::Action<bool>,
    paths: [xr::Path; 2],
    grips: [xr::Space; 2],
    rays: [xr::Space; 2],
}
impl Controls {
    fn new(instance: &xr::Instance, session: &xr::Session<xr::Vulkan>) -> Result<Self> {
        let paths = [
            instance.string_to_path("/user/hand/left")?,
            instance.string_to_path("/user/hand/right")?,
        ];
        let set = instance.create_action_set("gameplay", "Rim gameplay", 0)?;
        let grip = set.create_action::<xr::Posef>("grip", "Grip pose", &paths)?;
        let aim = set.create_action::<xr::Posef>("aim", "Aim pose", &paths)?;
        let stick = set.create_action::<xr::Vector2f>("move", "Movement", &paths)?;
        let trigger = set.create_action::<f32>("trigger", "Trigger", &paths)?;
        let haptic = set.create_action::<xr::Haptic>("haptic", "Cut feedback", &paths)?;
        let dash = set.create_action::<bool>("dash", "Dash", &[])?;
        let info_toggle =
            set.create_action::<bool>("info_toggle", "Toggle performance panel", &[])?;
        let mut bindings = Vec::new();
        bindings.push(xr::Binding::new(
            &dash,
            instance.string_to_path("/user/hand/right/input/a/click")?,
        ));
        bindings.push(xr::Binding::new(
            &info_toggle,
            instance.string_to_path("/user/hand/left/input/y/click")?,
        ));
        for hand in ["left", "right"] {
            bindings.push(xr::Binding::new(
                &haptic,
                instance.string_to_path(&format!("/user/hand/{hand}/output/haptic"))?,
            ));
            for (action, suffix) in [
                (0, "grip/pose"),
                (1, "aim/pose"),
                (2, "thumbstick"),
                (3, "trigger/value"),
            ] {
                let path = instance.string_to_path(&format!("/user/hand/{hand}/input/{suffix}"))?;
                bindings.push(match action {
                    0 => xr::Binding::new(&grip, path),
                    1 => xr::Binding::new(&aim, path),
                    2 => xr::Binding::new(&stick, path),
                    _ => xr::Binding::new(&trigger, path),
                });
            }
        }
        instance.suggest_interaction_profile_bindings(
            instance.string_to_path("/interaction_profiles/oculus/touch_controller")?,
            &bindings,
        )?;
        session.attach_action_sets(&[&set])?;
        let grips = [
            grip.create_space(session.clone(), paths[0], xr::Posef::IDENTITY)?,
            grip.create_space(session.clone(), paths[1], xr::Posef::IDENTITY)?,
        ];
        let rays = [
            aim.create_space(session.clone(), paths[0], xr::Posef::IDENTITY)?,
            aim.create_space(session.clone(), paths[1], xr::Posef::IDENTITY)?,
        ];
        Ok(Self {
            set,
            grip,
            aim,
            stick,
            trigger,
            haptic,
            dash,
            info_toggle,
            paths,
            grips,
            rays,
        })
    }
    fn locate(space: &xr::Space, world: &xr::Space, time: xr::Time) -> Result<Option<glam::Mat4>> {
        let loc = space.locate(world, time)?;
        Ok(loc
            .location_flags
            .contains(
                xr::SpaceLocationFlags::POSITION_VALID | xr::SpaceLocationFlags::ORIENTATION_VALID,
            )
            .then(|| pose(loc.pose)))
    }
}

pub fn run(
    read: &dyn Fn(&str) -> std::result::Result<Vec<u8>, String>,
    alive: &mut dyn FnMut() -> bool,
) -> Result<()> {
    run_with(
        "Kristek Rim Native",
        read,
        alive,
        &mut |device, queue, format, read| {
            Ok(Box::new(App::new(device, queue, format, read)?) as Box<dyn HostApp>)
        },
    )
}

pub fn run_with(
    application_name: &str,
    read: &dyn Fn(&str) -> std::result::Result<Vec<u8>, String>,
    alive: &mut dyn FnMut() -> bool,
    create: &mut dyn FnMut(
        wgpu::Device,
        wgpu::Queue,
        wgpu::TextureFormat,
        &dyn Fn(&str) -> std::result::Result<Vec<u8>, String>,
    ) -> std::result::Result<Box<dyn HostApp>, String>,
) -> Result<()> {
    let entry = unsafe { xr::Entry::load()? };
    #[cfg(target_os = "android")]
    entry.initialize_android_loader()?;
    let available = entry.enumerate_extensions()?;
    if !available.khr_vulkan_enable2 {
        return Err("OpenXR runtime lacks XR_KHR_vulkan_enable2".into());
    }
    let mut extensions = xr::ExtensionSet::default();
    extensions.khr_vulkan_enable2 = true;
    extensions.fb_display_refresh_rate = available.fb_display_refresh_rate;
    extensions.ext_local_floor = available.ext_local_floor;
    #[cfg(target_os = "android")]
    {
        extensions.khr_android_create_instance = true;
    }
    let instance = entry.create_instance(
        &xr::ApplicationInfo {
            application_name,
            application_version: 1,
            engine_name: "Kristek Rust",
            engine_version: 1,
            api_version: xr::Version::new(1, 0, 0),
        },
        &extensions,
        &[],
    )?;
    log::info!("OpenXR runtime: {:?}", instance.properties()?);
    let system = instance.system(xr::FormFactor::HEAD_MOUNTED_DISPLAY)?;
    let (_wgpu_instance, device, queue, vk_instance, vk_device, physical, family) =
        unsafe { graphics(&instance, system)? };
    let (session, mut waiter, mut stream) = unsafe {
        instance.create_session::<xr::Vulkan>(
            system,
            &xr::vulkan::SessionCreateInfo {
                instance: vk_instance.handle().as_raw() as _,
                physical_device: physical.as_raw() as _,
                device: vk_device.handle().as_raw() as _,
                queue_family_index: family,
                queue_index: 0,
            },
        )?
    };
    let controls = Controls::new(&instance, &session)?;
    let head = session.create_reference_space(xr::ReferenceSpaceType::VIEW, xr::Posef::IDENTITY)?;
    let views = instance.enumerate_view_configuration_views(system, STEREO)?;
    if views.len() != 2 {
        return Err("Expected two stereo views".into());
    }
    let (width, height) = (
        views
            .iter()
            .map(|v| v.recommended_image_rect_width)
            .max()
            .unwrap(),
        views
            .iter()
            .map(|v| v.recommended_image_rect_height)
            .max()
            .unwrap(),
    );
    let formats = session.enumerate_swapchain_formats()?;
    let raw_format = [vk::Format::R8G8B8A8_SRGB, vk::Format::B8G8R8A8_SRGB]
        .into_iter()
        .find(|f| formats.contains(&(f.as_raw() as u32)))
        .ok_or("Runtime lacks an sRGB swapchain format")?;
    let format = if raw_format == vk::Format::R8G8B8A8_SRGB {
        wgpu::TextureFormat::Rgba8UnormSrgb
    } else {
        wgpu::TextureFormat::Bgra8UnormSrgb
    };
    let mut swapchain = session.create_swapchain(&xr::SwapchainCreateInfo {
        create_flags: xr::SwapchainCreateFlags::EMPTY,
        usage_flags: xr::SwapchainUsageFlags::COLOR_ATTACHMENT,
        format: raw_format.as_raw() as _,
        sample_count: 1,
        width,
        height,
        face_count: 1,
        array_size: 2,
        mip_count: 1,
    })?;
    let desc = wgpu::TextureDescriptor {
        label: Some("OpenXR runtime owned color"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 2,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    };
    let textures: Vec<wgpu::Texture> = swapchain
        .enumerate_images()?
        .into_iter()
        .map(|image| unsafe {
            let hal = device.as_hal::<wgpu::hal::api::Vulkan>().unwrap();
            let texture = hal.texture_from_raw(
                vk::Image::from_raw(image),
                &wgpu::hal::TextureDescriptor {
                    label: desc.label,
                    size: desc.size,
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: desc.dimension,
                    format,
                    usage: wgpu::TextureUses::COLOR_TARGET,
                    memory_flags: wgpu::hal::MemoryFlags::empty(),
                    view_formats: vec![],
                },
                Some(Box::new(|| {})),
                wgpu::hal::vulkan::TextureMemory::External,
            );
            device.create_texture_from_hal::<wgpu::hal::api::Vulkan>(
                texture,
                &desc,
                wgpu::TextureUses::UNINITIALIZED,
            )
        })
        .collect();
    let eye_views: Vec<_> = textures
        .iter()
        .map(|t| {
            [0, 1].map(|eye| {
                t.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_array_layer: eye,
                    array_layer_count: Some(1),
                    ..Default::default()
                })
            })
        })
        .collect();
    let (_depth, depth) = webgpu_wasm::render::depth::create(
        &device,
        width,
        height,
        wgpu::TextureFormat::Depth32Float,
    );
    let mut app = create(device.clone(), queue.clone(), format, read)?;
    let mut recorder = app
        .measurement_setup()
        .and_then(|setup| match Recorder::new(setup) {
            Ok(recorder) => Some(recorder),
            Err(error) => {
                log::warn!("Native measurement disabled: {error}");
                None
            }
        });
    let measurement_origin = std::time::Instant::now();
    let mut measurement_last = None;
    let mut measurement_frame_id = 0u64;
    let mut active_refresh = None;
    let world_type = floor_reference(
        &session.enumerate_reference_spaces()?,
        extensions.ext_local_floor && app.recenterable_floor(),
    )
    .ok_or("Runtime offers no supported floor reference space")?;
    let world = session.create_reference_space(world_type, xr::Posef::IDENTITY)?;
    log::info!(
        "OpenXR world reference: {world_type:?}; local_floor_extension={}",
        extensions.ext_local_floor
    );
    if app.recenterable_floor() && world_type == xr::ReferenceSpaceType::STAGE {
        log::warn!("LOCAL_FLOOR unavailable; STAGE fallback follows room origin, not necessarily system recenter");
    }
    app.set_xr(true);
    let _drain = DrainGpu(device.clone());
    let target_refresh = if extensions.fb_display_refresh_rate {
        let rates = session.enumerate_display_refresh_rates()?;
        let target = preferred_refresh_rate(&rates);
        log::info!("Supported OpenXR display refresh rates: {rates:?}; target={target:?} Hz");
        active_refresh = session.get_display_refresh_rate().ok();
        app.set_refresh_hz(active_refresh);
        if target.is_none() {
            log::warn!("Neither 120 nor 90 Hz listed by runtime; keeping active rate");
        }
        target
    } else {
        log::warn!("XR_FB_display_refresh_rate unavailable; keeping runtime default");
        None
    };
    if let Some(recorder) = &mut recorder {
        recorder.profile(0, serde_json::json!({"kind":"setup","eye_width":width,"eye_height":height,
            "format":format!("{format:?}"),"msaa":1,"display_hz":active_refresh,
            "frame_profiling":cfg!(feature="frame-profiling"),"reference_space":format!("{world_type:?}")}));
    }
    // Same sampled, nonblocking profiler as the browser renderer; opt-in build.
    use webgpu_wasm::render::profiler;
    struct ProfileReset;
    impl Drop for ProfileReset {
        fn drop(&mut self) {
            profiler::reset();
        }
    }
    let _profile_reset = if cfg!(feature = "frame-profiling") {
        profiler::initialize(&device);
        Some(ProfileReset)
    } else {
        None
    };
    let mut profile_log = std::time::Instant::now();
    let mut events = xr::EventDataBuffer::new();
    let mut running = false;
    let mut focused = false;
    let mut last = None;
    let mut reference_changes = ReferenceChanges::default();
    loop {
        if !alive() {
            break;
        }
        while let Some(event) = instance.poll_event(&mut events)? {
            match event {
                xr::Event::SessionStateChanged(e) => match e.state() {
                    xr::SessionState::READY => {
                        session.begin(STEREO)?;
                        running = true;
                        last = None;
                        app.reset_frame_clock();
                        if let Some(target) = target_refresh {
                            match session.request_display_refresh_rate(target) {
                                Ok(()) => log::info!("Requested OpenXR {target} Hz"),
                                Err(error) => {
                                    log::warn!("{target} Hz request failed: {error}");
                                    if target > 90.
                                        && preferred_refresh_rate(
                                            &session
                                                .enumerate_display_refresh_rates()?
                                                .into_iter()
                                                .filter(|hz| *hz < 100.)
                                                .collect::<Vec<_>>(),
                                        ) == Some(90.)
                                    {
                                        match session.request_display_refresh_rate(90.) {
                                            Ok(()) => log::info!("Requested fallback OpenXR 90 Hz"),
                                            Err(error) => {
                                                log::warn!("90 Hz fallback failed: {error}")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        if extensions.fb_display_refresh_rate {
                            active_refresh = session.get_display_refresh_rate().ok();
                            app.set_refresh_hz(active_refresh);
                        }
                    }
                    xr::SessionState::STOPPING => {
                        session.end()?;
                        running = false;
                        focused = false;
                        last = None;
                        app.reset_frame_clock();
                        app.reset_tracking();
                    }
                    xr::SessionState::EXITING | xr::SessionState::LOSS_PENDING => return Ok(()),
                    state => {
                        focused = state == xr::SessionState::FOCUSED;
                        if !focused {
                            app.reset_tracking();
                        }
                    }
                },
                xr::Event::ReferenceSpaceChangePending(event) => {
                    if app.recenterable_floor()
                        && event.session() == session.as_raw()
                        && event.reference_space_type() == world_type
                    {
                        reference_changes.schedule(event.change_time().as_nanos());
                        log::info!(
                            "OpenXR recenter pending: {:?} at {}",
                            world_type,
                            event.change_time().as_nanos()
                        );
                    }
                }
                xr::Event::DisplayRefreshRateChangedFB(event) => {
                    active_refresh = Some(event.to_display_refresh_rate());
                    app.set_refresh_hz(active_refresh);
                    log::info!(
                        "OpenXR display refresh changed to {} Hz",
                        event.to_display_refresh_rate()
                    );
                }
                xr::Event::InstanceLossPending(_) => return Ok(()),
                _ => {}
            }
        }
        if !running {
            std::thread::sleep(std::time::Duration::from_millis(10));
            continue;
        }
        let measurement_start = recorder.as_ref().map(|_| std::time::Instant::now());
        profiler::begin_frame();
        let wait_scope = profiler::cpu_scope("xr.waitFrame");
        let frame = waiter.wait()?;
        drop(wait_scope);
        measurement_frame_id += 1;
        let mut measured = measurement_start.map(|start| {
            let now = std::time::Instant::now();
            let mut stages_ns = [0; 10];
            stages_ns[0] = now.duration_since(start).as_nanos() as u64;
            MeasurementFrame {
                record: FrameRecord {
                    frame_id: measurement_frame_id,
                    monotonic_ns: 0,
                    predicted_ns: frame.predicted_display_time.as_nanos(),
                    period_ns: frame.predicted_display_period.as_nanos(),
                    display_hz: active_refresh,
                    should_render: frame.should_render,
                    focused,
                    tracking_valid: false,
                    stages_ns,
                    app_interval_ns: None,
                    app: AppMetrics::default(),
                },
                stage_started: now,
            }
        });
        if reference_changes.take_due(frame.predicted_display_time.as_nanos()) {
            // Runtime applies the new origin at changeTime; do not bridge a blade
            // sweep across coordinate systems on either the XR or worker owner.
            app.reset_tracking();
            app.reset_frame_clock();
            last = None;
            log::info!("OpenXR recenter applied; tracking history reset");
        }
        let work_started = std::time::Instant::now();
        stream.begin()?;
        let dt = last
            .map(|t: i64| ((frame.predicted_display_time.as_nanos() - t) as f64 * 1e-9) as f32)
            .unwrap_or(1. / 72.)
            .max(0.);
        last = Some(frame.predicted_display_time.as_nanos());
        let (flags, views) = session.locate_views(STEREO, frame.predicted_display_time, &world)?;
        let valid = flags
            .contains(xr::ViewStateFlags::POSITION_VALID | xr::ViewStateFlags::ORIENTATION_VALID)
            && views.len() == 2;
        if let Some(m) = &mut measured {
            m.record.tracking_valid = valid;
        }
        if !frame.should_render || !valid {
            if let Some(m) = &mut measured {
                m.mark(1);
            }
            stream.end(
                frame.predicted_display_time,
                xr::EnvironmentBlendMode::OPAQUE,
                &[],
            )?;
            if let Some(mut m) = measured {
                m.mark(8);
                m.finish(
                    recorder.as_mut().unwrap(),
                    measurement_origin,
                    &mut measurement_last,
                    app.as_ref(),
                    false,
                );
            }
            continue;
        }
        let time = frame.predicted_display_time;
        let h = Controls::locate(&head, &world, time)?;
        let mut hands = [None; 2];
        let mut axes = [0.; 3];
        let mut rays = [None; 2];
        let mut aim = false;
        let mut left_trigger = false;
        let mut dash = false;
        if focused {
            session.sync_actions(&[(&controls.set).into()])?;
            let toggle = controls.info_toggle.state(&session, xr::Path::NULL)?;
            if toggle.is_active && toggle.changed_since_last_sync && toggle.current_state {
                app.toggle_info()?;
            }

            for (i, hand) in hands.iter_mut().enumerate() {
                if controls.grip.is_active(&session, controls.paths[i])? {
                    *hand = Controls::locate(&controls.grips[i], &world, time)?;
                }
            }
            for (i, ray) in rays.iter_mut().enumerate() {
                if controls.aim.is_active(&session, controls.paths[i])? {
                    *ray = Controls::locate(&controls.rays[i], &world, time)?;
                }
            }
            let left = controls.stick.state(&session, controls.paths[0])?;
            let right = controls.stick.state(&session, controls.paths[1])?;
            if left.is_active {
                axes[0] = left.current_state.x;
                axes[1] = -left.current_state.y;
            }
            if right.is_active {
                axes[2] = right.current_state.x;
            }
            let r = controls.trigger.state(&session, controls.paths[1])?;
            let l = controls.trigger.state(&session, controls.paths[0])?;
            left_trigger = l.is_active && l.current_state > 0.5;
            let a = controls.dash.state(&session, xr::Path::NULL)?;
            dash = a.is_active && a.current_state;
            aim = r.is_active && r.current_state > 0.5;
        }
        if let Some(m) = &mut measured {
            m.mark(1);
        }
        let update_scope = profiler::cpu_scope("xr.update");
        app.tick_input(FrameInput {
            dt: if focused { dt } else { 0. },
            head: h,
            hands,
            rays,
            axes,
            triggers: [left_trigger, aim],
            primary: dash,
        })?;
        drop(update_scope);
        if let Some(m) = &mut measured {
            m.mark(2);
        }
        for (i, amplitude) in app.take_haptics().into_iter().enumerate() {
            if focused && amplitude.is_finite() && amplitude > 0. {
                let vibration = xr::HapticVibration::new()
                    .amplitude(amplitude.clamp(0., 1.))
                    .duration(xr::Duration::from_nanos(20_000_000))
                    .frequency(xr::FREQUENCY_UNSPECIFIED);
                if let Err(error) =
                    controls
                        .haptic
                        .apply_feedback(&session, controls.paths[i], &vibration)
                {
                    log::warn!("Controller haptic: {error}");
                }
            }
        }
        if let Some(m) = &mut measured {
            m.mark(3);
        }
        let acquire_scope = profiler::cpu_scope("xr.acquireWait");
        let index = swapchain.acquire_image()? as usize;
        swapchain.wait_image(xr::Duration::INFINITE)?;
        drop(acquire_scope);
        if let Some(m) = &mut measured {
            m.mark(4);
        }
        for (eye, v) in views.iter().enumerate() {
            let world_pose = app.reference() * pose(v.pose);
            let view = RenderView {
                view_projection: projection(v.fov, 0.08, 350.) * world_pose.inverse(),
                camera_position: world_pose.w_axis.truncate(),
            };
            let _eye_scope = profiler::cpu_scope(if eye == 0 {
                "xr.leftEncodeSubmit"
            } else {
                "xr.rightEncodeSubmit"
            });
            app.render(
                view,
                &RenderTarget {
                    color: &eye_views[index][eye],
                    material_mask: false,
                    auxiliary: None,
                    depth: &depth,
                    color_format: format,
                    depth_format: wgpu::TextureFormat::Depth32Float,
                    width,
                    height,
                    viewport: Viewport::full(width, height),
                    color_load: wgpu::LoadOp::Load,
                    depth_load: wgpu::LoadOp::Clear(1.),
                    // This host submits color only. The next eye/frame clears
                    // this depth attachment; no compositor depth layer uses it.
                    depth_store: wgpu::StoreOp::Discard,
                },
                eye as u32 + 1,
            )?;
            if let Some(m) = &mut measured {
                m.mark(5 + eye);
            }
        }
        // Submit both layers in COLOR_ATTACHMENT_OPTIMAL on the session's queue.
        // OpenXR synchronizes the submitted work; CPU completion is not required
        // before release/end_frame. Queue and XR calls remain on this one thread.
        // DrainGpu still waits before any imported image/session teardown.
        let mut encoder = device.create_command_encoder(&Default::default());
        encoder.transition_resources(
            std::iter::empty(),
            std::iter::once(wgpu::TextureTransition {
                texture: &textures[index],
                selector: None,
                state: wgpu::TextureUses::COLOR_TARGET,
            }),
        );
        queue.submit([encoder.finish()]);
        profiler::end_frame(&device, &queue);
        device.poll(wgpu::PollType::Poll)?;
        swapchain.release_image()?;
        let projection_views: Vec<_> = views
            .iter()
            .enumerate()
            .map(|(eye, v)| {
                xr::CompositionLayerProjectionView::new()
                    .pose(v.pose)
                    .fov(v.fov)
                    .sub_image(
                        xr::SwapchainSubImage::new()
                            .swapchain(&swapchain)
                            .image_array_index(eye as u32)
                            .image_rect(xr::Rect2Di {
                                offset: xr::Offset2Di { x: 0, y: 0 },
                                extent: xr::Extent2Di {
                                    width: width as i32,
                                    height: height as i32,
                                },
                            }),
                    )
            })
            .collect();
        if let Some(m) = &mut measured {
            m.mark(7);
        }
        let end_started = cfg!(feature = "frame-profiling").then(std::time::Instant::now);
        stream.end(
            time,
            xr::EnvironmentBlendMode::OPAQUE,
            &[&xr::CompositionLayerProjection::new()
                .space(&world)
                .views(&projection_views)],
        )?;
        let end_frame_ms = end_started.map_or(0., |t| t.elapsed().as_secs_f64() * 1000.);
        if let Some(m) = &mut measured {
            m.mark(8);
        }
        app.frame_submitted(work_started.elapsed());
        if cfg!(feature = "frame-profiling") && profile_log.elapsed().as_secs_f32() >= 1. {
            let data = profiler::stats();
            log::info!(
                "Native profile: frame={} eye={}x{} end_frame_ms={:.3} data={}",
                measurement_frame_id,
                width,
                height,
                end_frame_ms,
                data
            );
            if let Some(recorder) = &mut recorder {
                recorder.profile(measurement_frame_id, data);
            }
            profile_log = std::time::Instant::now();
        }
        if let Some(mut m) = measured {
            m.mark(9);
            m.finish(
                recorder.as_mut().unwrap(),
                measurement_origin,
                &mut measurement_last,
                app.as_ref(),
                true,
            );
        }
    }
    device.poll(wgpu::PollType::wait_indefinitely())?;
    Ok(())
}
