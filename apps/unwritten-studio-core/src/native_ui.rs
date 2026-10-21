//! Native presentation/input host for the existing editor, not a second UI.
#![allow(unsafe_code, unsafe_op_in_unsafe_fn)]
use super::native_audio::NativeAudioBridge;
use super::native_session::NativeSessionRecovery;
use egui::{Event, Key, Modifiers, Pos2, RawInput, Rect, Vec2};
use egui_wgpu::{Renderer, RendererOptions, ScreenDescriptor};
use serde_json::{Value, json};
use std::{
    collections::VecDeque,
    ffi::{CStr, CString, c_char},
    panic::{AssertUnwindSafe, catch_unwind},
    sync::{Arc, Mutex, mpsc},
    time::Duration,
};
use unwritten_studio_ui_egui::{DialogDone, FileDialog, UnwrittenStudioApp};
const MAX_DIM: u32 = 4096;
const MAX_EVENTS: usize = 1024;
struct PendingDialog {
    id: u64,
    metadata: Value,
    done: DialogDone,
}
#[derive(Default)]
struct HostState {
    output: String,
    pending: VecDeque<PendingDialog>,
    next_id: u64,
    recovery_error: Option<String>,
}
/// Opaque handle. All operations must use its creating serial queue.
pub struct NativeUi {
    app: UnwrittenStudioApp,
    audio_bridge: Arc<NativeAudioBridge>,
    ctx: egui::Context,
    gpu: Gpu,
    host: Arc<Mutex<HostState>>,
    smoke_opened: bool,
    modifiers: Modifiers,
    recovery: NativeSessionRecovery,
}
struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Renderer,
}
struct FramePixels {
    bytes: Vec<u8>,
    width: u32,
    height: u32,
}
fn guard<T>(f: impl FnOnce() -> T) -> Option<T> {
    catch_unwind(AssertUnwindSafe(f)).ok()
}
fn key(s: &str) -> Option<Key> {
    Key::from_name(s).or_else(|| Key::from_name(&s.to_ascii_uppercase()))
}
fn modifiers(v: &Value) -> Modifiers {
    let command = v["command"].as_bool().unwrap_or(false);
    Modifiers {
        alt: v["alt"].as_bool().unwrap_or(false),
        ctrl: v["ctrl"].as_bool().unwrap_or(false),
        shift: v["shift"].as_bool().unwrap_or(false),
        mac_cmd: command,
        // Ctrl and Command are distinct on iPad hardware keyboards.  Treating
        // Ctrl as Command made Ctrl-click and Ctrl shortcuts take the macOS
        // command path, while also making the two modifier bits disagree.
        command,
    }
}
fn event_modifiers(v: &Value, current: Modifiers) -> Modifiers {
    let has_snapshot = ["command", "ctrl", "shift", "alt"].iter().any(|name| v.get(*name).is_some());
    if has_snapshot { modifiers(v) } else { current }
}
fn coordinate(v: &Value, name: &str) -> f32 {
    v[name].as_f64().filter(|n| n.is_finite()).unwrap_or(0.0).clamp(-65536.0, 65536.0) as f32
}
fn pos(v: &Value) -> Pos2 {
    Pos2::new(coordinate(v, "x"), coordinate(v, "y"))
}
fn geometry(input: &Value) -> Result<(u32, u32, f32, u32, u32), String> {
    let w = input["width"].as_u64().unwrap_or(1024);
    let h = input["height"].as_u64().unwrap_or(640);
    if w == 0 || h == 0 || w > u64::from(MAX_DIM) || h > u64::from(MAX_DIM) {
        return Err("Viewport dimensions must be between 1 and 4096 points".into());
    }
    let scale = input["scale"].as_f64().unwrap_or(1.0);
    if !scale.is_finite() || !(0.25..=2.0).contains(&scale) {
        return Err("Invalid viewport pixel scale".into());
    }
    let pw = ((w as f64 * scale).round() as u32).max(1);
    let ph = ((h as f64 * scale).round() as u32).max(1);
    if pw > MAX_DIM || ph > MAX_DIM {
        return Err("Rendered viewport exceeds 4096 pixels".into());
    }
    Ok((w as u32, h as u32, scale as f32, pw, ph))
}
fn presentation_limits(supported: wgpu::Limits) -> wgpu::Limits {
    wgpu::Limits::downlevel_defaults().using_resolution(supported)
}

#[cfg(target_os = "ios")]
fn smoke_export_to_ios_storage() -> Result<(), String> {
    // The regular macOS tests cannot reproduce iOS file permissions. Exercise
    // the real default output policy inside the simulator's app container too.
    // Use a separate tiny project so the visible demo remains untouched.
    let mut session = unwritten_studio_host::session();
    let name = format!("UISmoke-Storage-{}", std::process::id());
    session.execute("comp.new", json!({"name":name,"width":32,"height":24,"frameRate":2,"duration":1})).map_err(|e| e.to_string())?;
    session.execute("layer.newSolid", json!({"color":"#E85D45","width":32,"height":24})).map_err(|e| e.to_string())?;
    let item = session.execute("renderQueue.add", json!({"format":"h264"})).map_err(|e| e.to_string())?;
    let path = item["outputPath"].as_str().ok_or("iOS smoke render has no output path")?;
    let result = (|| {
        session.execute("renderQueue.render", json!({"wait":true})).map_err(|e| e.to_string())?;
        let Some(queued) = session.project.render_queue.first() else { return Err("iOS smoke render queue is empty".into()) };
        if queued.status != unwritten_studio_project::render_queue::RenderStatus::Done {
            return Err(format!("iOS sandbox render failed at {path}: {:?}", queued.status));
        }
        let imported = session.execute("file.import", json!({"paths":[path]})).map_err(|e| e.to_string())?;
        if imported["items"].as_array().is_none_or(|items| items.len() != 1) || imported["errors"].as_array().is_some_and(|errors| !errors.is_empty()) {
            return Err(format!("iOS smoke render could not be reimported: {imported}"));
        }
        Ok(())
    })();
    let _ = std::fs::remove_file(path);
    result
}

fn dialog_metadata(id: u64, dialog: FileDialog) -> Value {
    match dialog {
        FileDialog::Open { exts, many } => json!({"id":id,"kind":"open","extensions":exts,"many":many,"project":false}),
        FileDialog::OpenProject => {
            json!({"id":id,"kind":"open","extensions":["unw","unwae","uwproj","uwprojx","ecproj","ecprojx"],"many":false,"project":true})
        }
        FileDialog::Folder => json!({"id":id,"kind":"folder","extensions":[],"many":false,"project":false}),
        FileDialog::Save { default, ext, project } => {
            let name = std::path::Path::new(&default).file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or(default);
            json!({"id":id,"kind":"save","extensions":if ext.is_empty(){vec![]}else{vec![ext]},"many":false,"defaultName":name,"project":project})
        }
    }
}
impl NativeUi {
    fn new() -> Result<Self, String> {
        let host = Arc::new(Mutex::new(HostState::default()));
        let callback_host = Arc::clone(&host);
        let audio_bridge = NativeAudioBridge::new();
        // The desktop launcher supplies this through its session setup, but
        // the embedded iOS host constructs the session directly. Without a
        // ConfigStore the workspace store is memory-only and layouts disappear
        // when the native view is recreated. Reuse the host's per-app directory
        // so the existing debounced/periodic workspace saves cover iOS too.
        let mut session = unwritten_studio_host::session();
        if let Some(dir) = unwritten_studio_host::config_dir() {
            session.config = Some(Arc::new(unwritten_studio_engine::config::DirConfig::new(dir)));
        }
        let mut recovery = NativeSessionRecovery::new(unwritten_studio_host::config_dir());
        let recovery_result = recovery.restore(&mut session);
        let recovered = recovery_result.as_ref().ok().copied().unwrap_or(false);
        let recovery_error = recovery_result.err();
        let mut app = UnwrittenStudioApp::new(session);
        if recovered {
            app.ui.start_screen = false;
        }
        if let Some(error) = recovery_error
            && let Ok(mut h) = host.lock()
        {
            h.recovery_error = Some(error);
        }
        app.hooks.audio_device = Some(audio_bridge.factory());
        app.hooks.file_dialog_async = Some(Box::new(move |dialog, done| {
            let mut h = match callback_host.lock() {
                Ok(h) => h,
                Err(_) => {
                    done(Vec::new());
                    return;
                }
            };
            if h.pending.len() >= 8 {
                drop(h);
                done(Vec::new());
                return;
            }
            h.next_id = h.next_id.wrapping_add(1).max(1);
            let id = h.next_id;
            h.pending.push_back(PendingDialog { id, metadata: dialog_metadata(id, dialog), done });
        }));
        Ok(Self { app, audio_bridge, ctx: egui::Context::default(), gpu: Gpu::new()?, host, smoke_opened: false, modifiers: Modifiers::default(), recovery })
    }
    fn set_error(&self, error: impl Into<String>) {
        if let Ok(mut h) = self.host.lock() {
            h.output = json!({"ok":false,"error":error.into(),"wantsKeyboard":false}).to_string();
        }
    }
    fn complete_dialog(&mut self, input: &Value) {
        let Some(id) = input["dialog"]["id"].as_u64() else { return };
        let pending = self.host.lock().ok().and_then(|mut h| {
            let i = h.pending.iter().position(|p| p.id == id)?;
            h.pending.remove(i)
        });
        if let Some(pending) = pending {
            let paths = input["dialog"]["paths"]
                .as_array()
                .map(|a| a.iter().take(256).filter_map(Value::as_str).filter(|p| !p.contains('\0') && p.len() <= 16384).map(str::to_owned).collect())
                .unwrap_or_default();
            (pending.done)(paths);
        }
    }
    fn frame(&mut self, input: &Value) -> Result<FramePixels, String> {
        let (w, h, scale, pw, ph) = geometry(input)?;
        self.complete_dialog(input);
        if input["smoke"].as_bool().unwrap_or(false) && !self.smoke_opened {
            #[cfg(target_os = "ios")]
            smoke_export_to_ios_storage()?;
            self.app.session.execute("file.openDemoProject", json!({})).map_err(|e| e.to_string())?;
            self.app.ui.start_screen = false;
            self.smoke_opened = true;
        }
        let mut raw = RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(w as f32, h as f32))),
            time: input["time"].as_f64().filter(|t| t.is_finite()),
            focused: true,
            ..Default::default()
        };
        if let Some(viewport) = raw.viewports.get_mut(&egui::ViewportId::ROOT) {
            viewport.native_pixels_per_point = Some(scale);
            viewport.inner_rect = raw.screen_rect;
            viewport.focused = Some(true);
        }
        if let Some(events) = input["events"].as_array() {
            for e in events.iter().take(MAX_EVENTS) {
                match e["type"].as_str().unwrap_or("") {
                    "move" => raw.events.push(Event::PointerMoved(pos(e))),
                    "button" => {
                        let event_modifiers = event_modifiers(e, self.modifiers);
                        self.modifiers = event_modifiers;
                        raw.events.push(Event::PointerButton {
                            pos: pos(e),
                            button: match e["button"].as_str() {
                                Some("secondary") => egui::PointerButton::Secondary,
                                Some("middle") => egui::PointerButton::Middle,
                                _ => egui::PointerButton::Primary,
                            },
                            pressed: e["pressed"].as_bool().unwrap_or(false),
                            modifiers: event_modifiers,
                        });
                    }
                    "scroll" => {
                        let event_modifiers = event_modifiers(e, self.modifiers);
                        self.modifiers = event_modifiers;
                        raw.events.push(Event::MouseWheel {
                            unit: egui::MouseWheelUnit::Point,
                            delta: Vec2::new(coordinate(e, "dx"), coordinate(e, "dy")),
                            modifiers: event_modifiers,
                            phase: egui::TouchPhase::Move,
                        });
                    }
                    "text" => {
                        if let Some(s) = e["text"].as_str() {
                            raw.events.push(Event::Text(s.chars().take(4096).collect()));
                        }
                    }
                    "paste" => {
                        if let Some(s) = e["text"].as_str() {
                            raw.events.push(Event::Paste(s.chars().take(262144).collect()));
                        }
                    }
                    "key" => {
                        self.modifiers = modifiers(e);
                        raw.events.push(Event::ModifiersChanged(self.modifiers));
                        if let Some(k) = e["key"].as_str().and_then(key) {
                            raw.events.push(Event::Key {
                                key: k,
                                physical_key: None,
                                pressed: e["pressed"].as_bool().unwrap_or(false),
                                repeat: false,
                                modifiers: self.modifiers,
                            });
                        }
                    }
                    _ => {}
                }
            }
        }
        let mut full = self.ctx.run_ui(raw, |ui| {
            self.app.embedded_logic(ui.ctx());
            self.app.embedded_ui(ui);
        });
        if let Err(error) = self.recovery.tick(&self.app.session, input["checkpoint"].as_bool().unwrap_or(false))
            && let Ok(mut host) = self.host.lock()
        {
            host.recovery_error = Some(format!("{error}. Save your project manually to Files."));
        }
        self.host_output(&full.platform_output);
        let bytes = self.gpu.render(&self.ctx, &mut full, pw, ph)?;
        Ok(FramePixels { bytes, width: pw, height: ph })
    }
    fn host_output(&self, out: &egui::PlatformOutput) {
        let Ok(mut h) = self.host.lock() else { return };
        let mut v = json!({"ok":true,"wantsKeyboard":self.ctx.egui_wants_keyboard_input()});
        for cmd in &out.commands {
            match cmd {
                egui::OutputCommand::CopyText(s) => v["copiedText"] = json!(s),
                egui::OutputCommand::OpenUrl(u) => v["openURL"] = json!(u.url),
                _ => {}
            }
        }
        if let Some(p) = h.pending.front() {
            v["fileDialog"] = p.metadata.clone();
        }
        if let Some(error) = h.recovery_error.take() {
            v["recoveryError"] = json!(error);
        }
        h.output = v.to_string();
    }
}

impl Drop for NativeUi {
    fn drop(&mut self) {
        let _ = self.recovery.tick(&self.app.session, true);
    }
}

impl Gpu {
    fn new() -> Result<Self, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle().with_env());
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default())).map_err(|e| e.to_string())?;
        // Metal on iOS exposes 15 inter-stage variables, below wgpu's desktop
        // default of 16. egui needs only the portable/downlevel baseline; retain
        // the adapter's texture dimensions for Retina framebuffers and atlases.
        let descriptor = wgpu::DeviceDescriptor { required_limits: presentation_limits(adapter.limits()), ..Default::default() };
        let (device, queue) = pollster::block_on(adapter.request_device(&descriptor)).map_err(|e| e.to_string())?;
        let renderer = Renderer::new(&device, wgpu::TextureFormat::Rgba8Unorm, RendererOptions::default());
        Ok(Self { device, queue, renderer })
    }
    fn render(&mut self, ctx: &egui::Context, full: &mut egui::FullOutput, w: u32, h: u32) -> Result<Vec<u8>, String> {
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Original Unwritten Studio UI"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let jobs = ctx.tessellate(std::mem::take(&mut full.shapes), full.pixels_per_point);
        let screen = ScreenDescriptor { size_in_pixels: [w, h], pixels_per_point: full.pixels_per_point };
        for (id, deltas) in full.textures_delta.set.drain() {
            for delta in deltas {
                self.renderer.update_texture(&self.device, &self.queue, id, &delta);
            }
        }
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let commands = self.renderer.update_buffers(&self.device, &self.queue, &mut encoder, &jobs, &screen);
        {
            let mut pass = encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("Original egui frontend"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations { load: wgpu::LoadOp::Clear(wgpu::Color::BLACK), store: wgpu::StoreOp::Store },
                    })],
                    ..Default::default()
                })
                .forget_lifetime();
            self.renderer.render(&mut pass, &jobs, &screen);
        }
        let stride = (w * 4 + 255) & !255;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("UI readback"),
            size: u64::from(stride) * u64::from(h),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo { texture: &texture, mip_level: 0, origin: Default::default(), aspect: wgpu::TextureAspect::All },
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(stride), rows_per_image: Some(h) },
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let submission = self.queue.submit(commands.into_iter().chain(Some(encoder.finish())));
        for id in std::mem::take(&mut full.textures_delta.free) {
            self.renderer.free_texture(&id);
        }
        let slice = buffer.slice(..);
        let (sender, receiver) = mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::PollType::Wait { submission_index: Some(submission), timeout: Some(Duration::from_secs(10)) }).map_err(|e| e.to_string())?;
        receiver.recv_timeout(Duration::from_secs(1)).map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
        let mapped = slice.get_mapped_range().map_err(|e| e.to_string())?;
        let mut bytes = vec![0; w as usize * h as usize * 4];
        for y in 0..h as usize {
            bytes[y * w as usize * 4..(y + 1) * w as usize * 4].copy_from_slice(&mapped[y * stride as usize..y * stride as usize + w as usize * 4]);
        }
        drop(mapped);
        buffer.unmap();
        Ok(bytes)
    }
}
static INIT_ERROR: Mutex<String> = Mutex::new(String::new());
#[unsafe(no_mangle)]
pub extern "C" fn unwritten_studio_ui_last_error() -> *mut c_char {
    guard(|| INIT_ERROR.lock().ok().and_then(|error| CString::new(error.as_str()).ok()).map(CString::into_raw).unwrap_or(std::ptr::null_mut()))
        .unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub extern "C" fn unwritten_studio_ui_create() -> *mut NativeUi {
    let error = match catch_unwind(AssertUnwindSafe(NativeUi::new)) {
        Ok(Ok(ui)) => return Box::into_raw(Box::new(ui)),
        Ok(Err(error)) => error,
        Err(payload) => payload
            .downcast_ref::<String>()
            .cloned()
            .or_else(|| payload.downcast_ref::<&str>().map(|s| (*s).to_owned()))
            .unwrap_or_else(|| "Unknown panic during editor initialization".to_owned()),
    };
    eprintln!("Unwritten Studio frontend initialization failed: {error}");
    if let Ok(mut stored) = INIT_ERROR.lock() {
        *stored = error;
    }
    std::ptr::null_mut()
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_ui_destroy(p: *mut NativeUi) {
    let _ = guard(|| {
        if !p.is_null() {
            drop(unsafe { Box::from_raw(p) });
        }
    });
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_ui_audio_handle(p: *mut NativeUi) -> *mut super::native_audio::UnwrittenStudioAudio {
    if p.is_null() {
        return std::ptr::null_mut();
    }
    let ui = unsafe { &*p };
    guard(|| ui.audio_bridge.callback_handle()).unwrap_or(std::ptr::null_mut())
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_ui_frame(p: *mut NativeUi, input: *const c_char, w: *mut u32, h: *mut u32) -> *mut u8 {
    if p.is_null() || input.is_null() || w.is_null() || h.is_null() {
        return std::ptr::null_mut();
    }
    unsafe {
        *w = 0;
        *h = 0;
    }
    let ui = unsafe { &mut *p };
    let result = catch_unwind(AssertUnwindSafe(|| {
        let text = unsafe { CStr::from_ptr(input) }.to_str().map_err(|e| e.to_string())?;
        if text.len() > 1024 * 1024 {
            return Err("Input too large".to_owned());
        }
        let v: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        ui.frame(&v)
    }));
    match result {
        Ok(Ok(frame)) => {
            unsafe {
                *w = frame.width;
                *h = frame.height;
            }
            Box::into_raw(frame.bytes.into_boxed_slice()).cast::<u8>()
        }
        Ok(Err(e)) => {
            ui.set_error(e);
            std::ptr::null_mut()
        }
        Err(_) => {
            ui.set_error("The original UI renderer encountered an internal error");
            std::ptr::null_mut()
        }
    }
}
#[unsafe(no_mangle)]
pub unsafe extern "C" fn unwritten_studio_ui_output_json(p: *mut NativeUi) -> *mut c_char {
    guard(|| {
        if p.is_null() {
            return std::ptr::null_mut();
        }
        let ui = unsafe { &*p };
        let s = match ui.host.lock() {
            Ok(h) => h.output.clone(),
            Err(_) => return std::ptr::null_mut(),
        };
        CString::new(s).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
    })
    .unwrap_or(std::ptr::null_mut())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_project_dialogs_offer_unw_without_losing_legacy_extensions() {
        let open = dialog_metadata(3, FileDialog::OpenProject);
        let extensions = open["extensions"].as_array().unwrap();
        for ext in ["unw", "unwae", "uwproj", "uwprojx", "ecproj", "ecprojx"] {
            assert!(extensions.contains(&json!(ext)));
        }
        let save = dialog_metadata(4, FileDialog::save("Untitled Project.unw"));
        assert_eq!(save["defaultName"], "Untitled Project.unw");
        assert_eq!(save["extensions"], json!(["unw"]));
        assert_eq!(save["project"], true);
        assert_eq!(dialog_metadata(5, FileDialog::save("Render.mp4"))["project"], false);
    }

    #[test]
    fn presentation_limits_fit_ios_metal_inter_stage_cap() {
        let supported = wgpu::Limits { max_inter_stage_shader_variables: 15, ..Default::default() };
        let requested = presentation_limits(supported.clone());
        assert!(requested.check_limits(&supported));
        assert_eq!(requested.max_inter_stage_shader_variables, 15);
        assert_eq!(requested.max_texture_dimension_2d, supported.max_texture_dimension_2d);
    }

    #[test]
    fn retina_geometry_and_key_names_are_validated() {
        assert_eq!(geometry(&json!({"width":1024,"height":640,"scale":2})).unwrap(), (1024, 640, 2.0, 2048, 1280));
        assert!(geometry(&json!({"width":4096,"scale":2})).is_err());
        assert!(geometry(&json!({"width":0})).is_err());
        assert_eq!(key("Space"), Some(Key::Space));
        assert_eq!(key("ArrowLeft"), Some(Key::ArrowLeft));
        assert_eq!(key("1"), Some(Key::Num1));
    }

    #[test]
    fn modifiers_keep_command_and_control_distinct() {
        let command = modifiers(&json!({"command":true}));
        assert!(command.command && command.mac_cmd);
        assert!(!command.ctrl);
        let control = modifiers(&json!({"ctrl":true}));
        assert!(control.ctrl);
        assert!(!control.command && !control.mac_cmd);
    }

    #[test]
    fn pointer_modifier_snapshot_overrides_stale_key_state() {
        let stale = modifiers(&json!({"command":true}));
        let button = event_modifiers(&json!({"type":"button", "ctrl":true}), stale);
        assert!(button.ctrl);
        assert!(!button.command);
        let touch_event = event_modifiers(&json!({"type":"button", "x":4}), button);
        assert_eq!(touch_event, button);
    }

    /// This is deliberately an original renderer test, not a command-only engine test.  It
    /// drives the same app/session owned by NativeUi, sends real egui input, observes the async
    /// dialog queue, then exercises the session's edit, persistence, import, and export paths.
    #[test]
    #[ignore = "Requires Metal/Vulkan; run explicitly with --include-ignored"]
    fn original_app_session_input_dialog_and_project_workflow() {
        let mut ui = NativeUi::new().expect("GPU presentation adapter");
        ui.frame(&json!({"width":800,"height":600,"scale":1,"time":0.0,"smoke":true})).expect("initial original UI frame");

        // Text and paste enter through the same RawInput path used by the iOS bridge.
        ui.frame(&json!({"width":800,"height":600,"scale":1,"time":0.1,"events":[
            {"type":"key","key":"A","pressed":true,"command":false,"ctrl":false,"shift":false,"alt":false},
            {"type":"text","text":"native"},
            {"type":"paste","text":" input"},
            {"type":"key","key":"A","pressed":false,"command":false,"ctrl":false,"shift":false,"alt":false}
        ]}))
        .expect("input frame");

        let (dialog_done, dialog_paths) = (Arc::new(Mutex::new(None)), Arc::new(Mutex::new(None)));
        let done_slot = Arc::clone(&dialog_done);
        let paths_slot = Arc::clone(&dialog_paths);
        let done: DialogDone = Box::new(move |paths| {
            if let Ok(mut slot) = paths_slot.lock() {
                *slot = Some(paths);
            }
        });
        if let Some(request) = ui.app.hooks.file_dialog_async.as_ref() {
            request(FileDialog::OpenProject, done);
            let host = ui.host.lock().expect("host lock");
            let pending = host.pending.front().expect("dialog request");
            assert_eq!(pending.metadata["kind"], "open");
            assert_eq!(pending.metadata["project"], true);
            *done_slot.lock().expect("done slot") = Some(pending.id);
        } else {
            panic!("native dialog hook not installed");
        }
        let dialog_id = dialog_done.lock().expect("done slot").expect("dialog id");
        ui.complete_dialog(&json!({"dialog":{"id":dialog_id,"paths":["/tmp/native-workflow.uwproj"]}}));
        assert_eq!(dialog_paths.lock().expect("paths slot").as_ref().and_then(|paths| paths.first()).map(String::as_str), Some("/tmp/native-workflow.uwproj"),);

        ui.app.session.execute("file.openDemoProject", json!({})).expect("demo project");
        let undo_before = ui.app.session.history.undo.len();
        ui.app.session.execute("layer.newSolid", json!({"name":"Native regression","color":"#406080","width":64,"height":64})).expect("edit");
        assert_eq!(ui.app.session.history.undo.len(), undo_before + 1);
        ui.app.session.execute("edit.undo", json!({})).expect("undo");
        ui.app.session.execute("edit.redo", json!({})).expect("redo");

        let root = std::env::temp_dir().join(format!("unwritten-native-ui-{}", std::process::id()));
        let project = root.with_extension("uwproj");
        let svg = root.with_extension("svg");
        let lottie = root.with_extension("json");
        std::fs::write(&svg, "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"8\" height=\"8\"><rect width=\"8\" height=\"8\" fill=\"#406080\"/></svg>")
            .expect("svg fixture");
        ui.app.session.execute("file.saveAs", json!({"path":project.to_string_lossy()})).expect("save project");
        ui.app.session.execute("file.import", json!({"paths":[svg.to_string_lossy()]})).expect("import svg");
        ui.app.session.execute("file.exportLottie", json!({"path":lottie.to_string_lossy()})).expect("export lottie");
        assert!(project.is_file() && lottie.is_file());
        let mut reopened = unwritten_studio_host::session();
        reopened.execute("file.open", json!({"path":project.to_string_lossy()})).expect("reopen project");
        assert!(reopened.active_comp_id().is_some());
        let _ = std::fs::remove_file(project);
        let _ = std::fs::remove_file(svg);
        let _ = std::fs::remove_file(lottie);
    }

    #[test]
    #[ignore = "Requires Metal/Vulkan; run explicitly to capture rendering evidence"]
    fn original_frontend_renders_real_demo_pixels() {
        let mut ui = NativeUi::new().expect("GPU presentation adapter");
        let mut last = None;
        for i in 0..8 {
            last = Some(ui.frame(&json!({"width":1280,"height":800,"scale":1,"time":i as f64/30.0,"smoke":true})).expect("Original UI frame"));
            std::thread::sleep(Duration::from_millis(100));
        }
        let frame = last.expect("frame");
        assert_eq!(frame.bytes.len(), 1280 * 800 * 4);
        assert!(frame.bytes.chunks_exact(4).any(|p| p[0] > 100 && p[1] > 100));
        if let Ok(path) = std::env::var("UNWRITTEN_UI_SMOKE_PATH") {
            image::save_buffer(path, &frame.bytes, frame.width, frame.height, image::ColorType::Rgba8).expect("PNG preview");
        }
    }
}
