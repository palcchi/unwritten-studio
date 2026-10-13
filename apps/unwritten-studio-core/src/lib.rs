#![deny(unsafe_code, unsafe_op_in_unsafe_fn)]

use std::ffi::{CString, c_char};
use std::panic::{AssertUnwindSafe, catch_unwind};
#[cfg(target_os = "ios")]
use std::sync::Arc;
use std::sync::Mutex;

use effectcraft_engine::Session;
use effectcraft_project::{ItemId, ItemKind, Project, SavedView};
use effectcraft_time::Tick;
use serde_json::{Value, json};

pub struct UnwrittenStudioCore {
    session: Mutex<Session>,
}

fn make_session() -> Session {
    let mut session = effectcraft_host::session();
    #[cfg(target_os = "ios")]
    {
        match effectcraft_gpu::Gpu::headless() {
            Some(gpu) => {
                session.window_adapter = Some(effectcraft_engine::render::Accelerator::name(&gpu));
                session.accel = Some(Arc::new(gpu));
                session.accel_note = None;
            }
            None => {
                session.window_adapter = Some("CPU fallback (Metal adapter unavailable)".into());
                session.accel_note = Some("wgpu could not initialize a Metal adapter; rendering uses the existing CPU compositor".into());
            }
        }
    }
    #[cfg(not(target_os = "ios"))]
    {
        session.window_adapter = Some("CPU (host validation)".into());
    }
    session
}

fn c_string(value: impl AsRef<str>) -> *mut c_char {
    CString::new(value.as_ref()).map(CString::into_raw).unwrap_or(std::ptr::null_mut())
}

fn result_json<T: serde::Serialize>(result: Result<T, String>) -> *mut c_char {
    match result {
        Ok(value) => c_string(json!({"ok": true, "result": value}).to_string()),
        Err(error) => c_string(json!({"ok": false, "error": error}).to_string()),
    }
}

fn snapshot(session: &Session) -> Value {
    let items: Vec<Value> = session
        .project
        .items
        .values()
        .map(|item| {
            let (kind, width, height, duration, layer_count) = match &item.kind {
                ItemKind::Folder => ("Folder", None, None, None, None),
                ItemKind::Comp(comp) => ("Composition", Some(comp.width), Some(comp.height), Some(comp.duration.seconds()), Some(comp.layers.len())),
                ItemKind::Footage(footage) => (item.type_name(), Some(footage.width), Some(footage.height), Some(footage.duration.seconds()), None),
                ItemKind::Solid(solid) => ("Solid", Some(solid.width), Some(solid.height), None, None),
            };
            json!({
                "id": item.id.0,
                "name": item.name,
                "kind": kind,
                "width": width,
                "height": height,
                "durationSeconds": duration,
                "layerCount": layer_count,
            })
        })
        .collect();

    let active = session.active_comp().and_then(|comp| {
        let id = session.active_comp_id()?;
        let item = session.project.item(id)?;
        let layers: Vec<Value> = comp
            .layers
            .iter()
            .enumerate()
            .map(|(index, layer)| {
                json!({
                    "id": layer.id.0,
                    "name": layer.name,
                    "kind": layer.source.type_name(),
                    "stackIndex": index + 1,
                    "inSeconds": layer.in_point.seconds(),
                    "outSeconds": layer.out_point.seconds(),
                    "startSeconds": layer.start_time.seconds(),
                    "durationSeconds": (layer.out_point - layer.in_point).seconds(),
                })
            })
            .collect();
        Some(json!({
            "id": id.0,
            "name": item.name,
            "width": comp.width,
            "height": comp.height,
            "durationSeconds": comp.duration.seconds(),
            "frameRate": comp.frame_rate.as_f64(),
            "layers": layers,
        }))
    });

    json!({
        "projectName": session.path.as_deref().and_then(|p| std::path::Path::new(p).file_stem()).and_then(|s| s.to_str()).unwrap_or("Untitled Project"),
        "hasProject": !session.project.items.is_empty(),
        "items": items,
        "activeComp": active,
        "selectedLayers": session.state.selected_layers.iter().map(|id| id.0).collect::<Vec<_>>(),
        "currentTimeSeconds": session.time().seconds(),
        "dirty": session.is_dirty(),
        "canUndo": !session.history.undo.is_empty(),
        "canRedo": !session.history.redo.is_empty(),
        "renderer": session.window_adapter.as_deref().unwrap_or("CPU fallback"),
        "rendererNote": session.accel_note,
    })
}

fn with_session<T>(core: &UnwrittenStudioCore, f: impl FnOnce(&mut Session) -> Result<T, String>) -> Result<T, String> {
    let mut session = core.session.lock().map_err(|_| "editor session is unavailable".to_string())?;
    f(&mut session)
}

fn command(session: &mut Session, name: &str, params: Value) -> Result<Value, String> {
    session.execute_checked(name, params).map_err(|error| error.to_string())
}

#[allow(unsafe_code, unsafe_op_in_unsafe_fn)]
mod ffi {
    use super::*;
    use std::ffi::CStr;
    use std::os::raw::c_uint;

    unsafe fn read_text<'a>(pointer: *const c_char, label: &str) -> Result<&'a str, String> {
        if pointer.is_null() {
            return Err(format!("{label} is required"));
        }
        // SAFETY: all exported functions document that non-null string inputs must be NUL-terminated.
        unsafe { CStr::from_ptr(pointer) }.to_str().map_err(|_| format!("{label} must be valid UTF-8"))
    }

    unsafe fn core_ref<'a>(pointer: *const UnwrittenStudioCore) -> Result<&'a UnwrittenStudioCore, String> {
        // SAFETY: the caller must pass a live pointer returned by unwritten_studio_create.
        unsafe { pointer.as_ref() }.ok_or_else(|| "editor session is unavailable".to_string())
    }

    fn guard(f: impl FnOnce() -> *mut c_char) -> *mut c_char {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(pointer) => pointer,
            Err(_) => super::c_string(r#"{"ok":false,"error":"native editor operation failed"}"#),
        }
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn unwritten_studio_create() -> *mut UnwrittenStudioCore {
        match catch_unwind(AssertUnwindSafe(|| Box::new(UnwrittenStudioCore { session: Mutex::new(make_session()) }))) {
            Ok(core) => Box::into_raw(core),
            Err(_) => std::ptr::null_mut(),
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_destroy(pointer: *mut UnwrittenStudioCore) {
        if !pointer.is_null() {
            // SAFETY: the pointer must be the unique, live handle returned by create; caller must destroy it once.
            drop(unsafe { Box::from_raw(pointer) });
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_snapshot_json(pointer: *const UnwrittenStudioCore) -> *mut c_char {
        guard(|| {
            // SAFETY: checked against the C API's live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return super::c_string(json!({"error":e}).to_string()),
            };
            match core.session.lock() {
                Ok(session) => super::c_string(snapshot(&session).to_string()),
                Err(_) => super::c_string(r#"{"error":"editor session is unavailable"}"#),
            }
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_execute(pointer: *mut UnwrittenStudioCore, command_ptr: *const c_char, params_ptr: *const c_char) -> *mut c_char {
        guard(|| {
            // SAFETY: input pointers follow the documented C API string/handle lifetime contract.
            let command_name = match unsafe { read_text(command_ptr, "command") } {
                Ok(value) => value.to_owned(),
                Err(e) => return result_json::<Value>(Err(e)),
            };
            let params_text = match unsafe { read_text(params_ptr, "params_json") } {
                Ok(value) => value,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            let params = match serde_json::from_str::<Value>(params_text) {
                Ok(value) => value,
                Err(e) => return result_json::<Value>(Err(format!("invalid command JSON: {e}"))),
            };
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| command(session, &command_name, params)))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_new_project(pointer: *mut UnwrittenStudioCore, name_ptr: *const c_char) -> *mut c_char {
        guard(|| {
            // SAFETY: input pointers follow the documented C API string/handle lifetime contract.
            let name = match unsafe { read_text(name_ptr, "project name") } {
                Ok(value) => value.to_owned(),
                Err(e) => return result_json::<Value>(Err(e)),
            };
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| {
                session.replace_project(Project::default(), None);
                command(session, "comp.new", json!({"name":name, "width":1920, "height":1080, "frameRate":30, "duration":10.0}))
            }))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_load_project_json(pointer: *mut UnwrittenStudioCore, project_ptr: *const c_char) -> *mut c_char {
        guard(|| {
            // SAFETY: project_ptr follows the documented NUL-terminated UTF-8 contract.
            let text = match unsafe { read_text(project_ptr, "project_json") } {
                Ok(value) => value.to_owned(),
                Err(e) => return result_json::<Value>(Err(e)),
            };
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| {
                let json_text = if effectcraft_engine::xml_project::is_xml(&text) { effectcraft_engine::xml_project::from_xml(&text)? } else { text };
                let (project, repaired) = Project::from_json_repaired(&json_text).map_err(|e| e.to_string())?;
                let view: SavedView = effectcraft_project::saved_view(&json_text);
                session.replace_project(project, None);
                session.restore_view(&view);
                Ok(json!({"repaired":repaired}))
            }))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_serialize_project_json(pointer: *const UnwrittenStudioCore) -> *mut c_char {
        guard(|| {
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(_) => return std::ptr::null_mut(),
            };
            match core.session.lock() {
                Ok(session) => session.project.to_file_json_with_view(&session.saved_view()).map(super::c_string).unwrap_or_else(|_| std::ptr::null_mut()),
                Err(_) => std::ptr::null_mut(),
            }
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_import_media(pointer: *mut UnwrittenStudioCore, path_ptr: *const c_char) -> *mut c_char {
        guard(|| {
            // SAFETY: input pointers follow the documented NUL-terminated UTF-8 contract.
            let path = match unsafe { read_text(path_ptr, "absolute_path") } {
                Ok(value) => value.to_owned(),
                Err(e) => return result_json::<Value>(Err(e)),
            };
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| {
                let mut imported = command(session, "file.import", json!({"paths":[path], "sequence":false}))?;
                let item_ids: Vec<u64> = imported.get("items").and_then(Value::as_array).into_iter().flatten().filter_map(Value::as_u64).collect();
                let mut added = 0usize;
                for id in item_ids {
                    if session.project.item(ItemId(id)).is_some_and(|item| matches!(&item.kind, ItemKind::Footage(_))) {
                        command(session, "layer.addItem", json!({"item":id}))?;
                        added += 1;
                    }
                }
                imported["addedToComposition"] = json!(added);
                Ok(imported)
            }))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_set_time(pointer: *mut UnwrittenStudioCore, seconds: f64) -> *mut c_char {
        guard(|| {
            if !seconds.is_finite() || seconds < 0.0 {
                return result_json::<Value>(Err("time must be a finite non-negative number".into()));
            }
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| {
                session.set_time(Tick::from_seconds_f64(seconds));
                Ok(json!({"timeSeconds":session.time().seconds()}))
            }))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_undo(pointer: *mut UnwrittenStudioCore) -> *mut c_char {
        guard(|| {
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| Ok(json!({"undone":session.undo()}))))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_redo(pointer: *mut UnwrittenStudioCore) -> *mut c_char {
        guard(|| {
            // SAFETY: checked against the live-handle contract.
            let core = match unsafe { core_ref(pointer) } {
                Ok(core) => core,
                Err(e) => return result_json::<Value>(Err(e)),
            };
            result_json(with_session(core, |session| Ok(json!({"redone":session.redo()}))))
        })
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_render_rgba8(
        pointer: *const UnwrittenStudioCore,
        max_side: c_uint,
        width_out: *mut c_uint,
        height_out: *mut c_uint,
    ) -> *mut u8 {
        if width_out.is_null() || height_out.is_null() {
            return std::ptr::null_mut();
        }
        // SAFETY: checked against the live-handle contract.
        let Ok(core) = (unsafe { core_ref(pointer) }) else { return std::ptr::null_mut() };
        let rendered = catch_unwind(AssertUnwindSafe(|| {
            let session = core.session.lock().ok()?;
            let comp = session.active_comp_id()?;
            session.render_rgba8(comp, session.time(), max_side).ok()
        }));
        let Ok(Some((width, height, bytes))) = rendered else { return std::ptr::null_mut() };
        // SAFETY: caller supplies valid writable out-pointers per C API contract.
        unsafe {
            width_out.write(width);
            height_out.write(height);
        }
        Box::into_raw(bytes.into_boxed_slice()) as *mut u8
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_bytes_free(pointer: *mut u8, length: usize) {
        if !pointer.is_null() {
            // SAFETY: pointer and length must be the exact pair returned by render_rgba8.
            let slice = std::ptr::slice_from_raw_parts_mut(pointer, length);
            drop(unsafe { Box::from_raw(slice) });
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_mark_saved(pointer: *mut UnwrittenStudioCore) {
        // SAFETY: the caller must pass a live handle returned by create.
        let Ok(core) = (unsafe { core_ref(pointer) }) else { return };
        if let Ok(mut session) = core.session.lock() {
            session.mark_saved();
        }
    }

    #[unsafe(no_mangle)]
    pub unsafe extern "C" fn unwritten_studio_string_free(pointer: *mut c_char) {
        if !pointer.is_null() {
            // SAFETY: pointer must be one live string returned by this library and freed once.
            drop(unsafe { CString::from_raw(pointer) });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_creates_edits_serializes_and_renders_a_real_project() {
        let mut session = make_session();
        command(&mut session, "comp.new", json!({"name":"Native Slice", "width":320, "height":180, "frameRate":30, "duration":2.0})).unwrap();
        command(&mut session, "layer.newSolid", json!({"name":"Slate", "color":"#E85D45", "width":120, "height":80})).unwrap();
        let comp_id = session.active_comp_id().unwrap();
        assert_eq!(session.active_comp().unwrap().layers.len(), 1);
        let (width, height, rgba) = session.render_rgba8(comp_id, Tick::ZERO, 240).unwrap();
        assert_eq!((width, height), (240, 135));
        assert_eq!(rgba.len(), 240 * 135 * 4);
        assert!(rgba.chunks_exact(4).any(|pixel| pixel[0] > 80 && pixel[3] == 255));
        let file = session.project.to_file_json_with_view(&session.saved_view()).unwrap();
        let (round_trip, _) = Project::from_json_repaired(&file).unwrap();
        assert_eq!(round_trip.items.len(), session.project.items.len());
        assert!(session.undo());
        assert!(session.redo());
    }

    #[test]
    fn snapshot_contains_native_workspace_data_and_cpu_fallback_status() {
        let mut session = make_session();
        command(&mut session, "comp.new", json!({"name":"Untitled Comp", "width":640, "height":360, "duration":3})).unwrap();
        let value = snapshot(&session);
        assert_eq!(value["activeComp"]["name"], "Untitled Comp");
        assert_eq!(value["activeComp"]["width"], 640);
        assert_eq!(value["renderer"], "CPU (host validation)");
    }
}
