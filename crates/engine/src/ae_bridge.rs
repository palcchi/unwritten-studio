//! Clean-room interchange via the public After Effects desktop scripting API.
//! AEP bytes are never decoded or relabelled as native projects. v1 is a limited 2D subset.
use serde::{Deserialize, Serialize};
use serde_json::{Value as Json, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};
use unwritten_studio_color::Label;
use unwritten_studio_keyframe::{Interp, Keyframe, Value};
use unwritten_studio_project::{Comp, Footage, FootageKind, ItemKind, Layer, LayerSource, Project, Solid, build};
use unwritten_studio_time::{FrameRate, Tick};

pub const FORMAT: &str = "unwritten-after-effects";
pub const VERSION: u32 = 1;
const MAX_BYTES: usize = 32 * 1024 * 1024;
const MAX_LAYERS: usize = 20_000;
const MAX_KEYS: usize = 100_000;
pub const EXPORTER: &str = include_str!("../scripts/Export Unwritten AE Bridge.jsx");
const IMPORTER: &str = include_str!("../scripts/Import Unwritten AE Bridge.jsx");

#[derive(Serialize, Deserialize)]
struct Document {
    format: String,
    version: u32,
    #[serde(default)]
    warnings: Vec<String>,
    comps: Vec<Composition>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Composition {
    id: String,
    name: String,
    width: u32,
    height: u32,
    fps: f64,
    duration: f64,
    #[serde(default = "one")]
    pixel_aspect: f64,
    #[serde(default)]
    background: [f32; 3],
    #[serde(default)]
    layers: Vec<BridgeLayer>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BridgeLayer {
    name: String,
    kind: String,
    #[serde(default)]
    source: String,
    #[serde(default = "size_default")]
    width: u32,
    #[serde(default = "size_default")]
    height: u32,
    #[serde(default = "white")]
    color: [f32; 3],
    #[serde(default)]
    text: String,
    #[serde(default)]
    font: String,
    #[serde(default = "font_size")]
    font_size: f64,
    #[serde(default)]
    start_time: f64,
    #[serde(default)]
    in_point: f64,
    out_point: f64,
    #[serde(default = "hundred")]
    stretch: f64,
    #[serde(default = "yes")]
    enabled: bool,
    #[serde(default)]
    transform: BTreeMap<String, BridgeProperty>,
}
#[derive(Serialize, Deserialize)]
struct BridgeProperty {
    value: Json,
    #[serde(default)]
    keys: Vec<BridgeKey>,
}
#[derive(Serialize, Deserialize)]
struct BridgeKey {
    time: f64,
    value: Json,
    #[serde(default)]
    hold: bool,
}
fn one() -> f64 {
    1.0
}
fn hundred() -> f64 {
    100.0
}
fn font_size() -> f64 {
    48.0
}
fn size_default() -> u32 {
    100
}
fn white() -> [f32; 3] {
    [1.0; 3]
}
fn yes() -> bool {
    true
}
fn valid_time(v: f64) -> bool {
    v.is_finite() && v.abs() <= 86_400.0
}
fn string_ok(s: &str) -> bool {
    s.len() <= 16_384 && !s.contains('\0')
}
fn color_ok(c: &[f32; 3]) -> bool {
    c.iter().all(|x| x.is_finite() && (0.0..=1.0).contains(x))
}
fn dims(w: u32, h: u32) -> bool {
    (4..=30_000).contains(&w) && (4..=30_000).contains(&h)
}
fn property_value(name: &str, value: &Json) -> Result<Value, String> {
    let number = |v: &Json| v.as_f64().filter(|n| n.is_finite() && n.abs() <= 1e9).ok_or_else(|| format!("invalid {name} value"));
    match name {
        "anchor" | "position" | "scale" => {
            // v1 is deliberately 2D.  Accepting a third component here would
            // silently discard Z (and the JSX importer cannot apply it to a
            // 2D property), producing a misleading partial reconstruction.
            let a = value.as_array().filter(|a| a.len() == 2).ok_or_else(|| format!("{name} requires a 2D vector"))?;
            let x = number(a.first().ok_or("missing vector x")?)?;
            let y = number(a.get(1).ok_or("missing vector y")?)?;
            let z = if name == "scale" { 100.0 } else { 0.0 };
            Ok(Value::Vec3([x, y, z]))
        }
        "rotation" | "opacity" => Ok(Value::Scalar(number(value)?)),
        _ => Err(format!("unsupported transform property {name}")),
    }
}
fn check_graph(comps: &[Composition]) -> Result<(), String> {
    fn visit(id: &str, edges: &BTreeMap<&str, Vec<&str>>, stack: &mut BTreeSet<String>, done: &mut BTreeSet<String>, depth: usize) -> Result<(), String> {
        if depth > 64 {
            return Err("precomposition nesting exceeds 64 levels".into());
        }
        if done.contains(id) {
            return Ok(());
        }
        if !stack.insert(id.to_string()) {
            return Err("cyclic precomposition reference".into());
        }
        let next = edges.get(id).ok_or_else(|| format!("missing precomposition {id}"))?;
        for child in next {
            visit(child, edges, stack, done, depth + 1)?;
        }
        stack.remove(id);
        done.insert(id.to_string());
        Ok(())
    }
    let edges: BTreeMap<&str, Vec<&str>> =
        comps.iter().map(|c| (c.id.as_str(), c.layers.iter().filter(|l| l.kind == "comp").map(|l| l.source.as_str()).collect())).collect();
    let mut done = BTreeSet::new();
    for c in comps {
        visit(&c.id, &edges, &mut BTreeSet::new(), &mut done, 0)?;
    }
    Ok(())
}
fn decode(bytes: &[u8]) -> Result<Document, String> {
    if bytes.len() > MAX_BYTES {
        return Err(".unwae exceeds the 32 MiB safety limit".into());
    }
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let d: Document = serde_json::from_slice(bytes).map_err(|e| format!("invalid .unwae JSON: {e}"))?;
    if d.format != FORMAT || d.version != VERSION {
        return Err("unsupported .unwae format or version".into());
    }
    if d.comps.is_empty() || d.comps.len() > 512 || d.warnings.len() > 1000 || d.warnings.iter().any(|s| !string_ok(s)) {
        return Err("bridge requires 1–512 compositions and at most 1000 bounded warnings".into());
    }
    let mut ids = BTreeSet::new();
    let (mut layers, mut keys) = (0usize, 0usize);
    for c in &d.comps {
        if c.id.is_empty()
            || !string_ok(&c.id)
            || !ids.insert(&c.id)
            || !string_ok(&c.name)
            || !dims(c.width, c.height)
            || !c.fps.is_finite()
            || !(0.001..=1000.0).contains(&c.fps)
            || !valid_time(c.duration)
            || c.duration <= 0.0
            || !c.pixel_aspect.is_finite()
            || !(0.1..=10.0).contains(&c.pixel_aspect)
            || !color_ok(&c.background)
        {
            return Err("invalid composition id, dimensions, timing or colour".into());
        }
        layers = layers.saturating_add(c.layers.len());
        for l in &c.layers {
            if !["solid", "null", "text", "footage", "comp"].contains(&l.kind.as_str()) {
                return Err(format!("unsupported layer kind {}", l.kind));
            }
            if [&l.name, &l.source, &l.text, &l.font].iter().any(|s| !string_ok(s))
                || !dims(l.width, l.height)
                || !color_ok(&l.color)
                || !valid_time(l.start_time)
                || !valid_time(l.in_point)
                || !valid_time(l.out_point)
                || l.out_point <= l.in_point
                || !l.stretch.is_finite()
                || !(0.01..=10_000.0).contains(&l.stretch)
                || !l.font_size.is_finite()
                || !(0.1..=10_000.0).contains(&l.font_size)
            {
                return Err(format!("invalid layer {}", l.name));
            }
            if l.kind == "footage" && l.source.is_empty() {
                return Err("footage source path is empty".into());
            }
            for (name, p) in &l.transform {
                property_value(name, &p.value)?;
                keys = keys.saturating_add(p.keys.len());
                let mut last = None;
                for key in &p.keys {
                    if !valid_time(key.time) || !valid_time((key.time - l.start_time) * 100.0 / l.stretch) || last.is_some_and(|t| key.time <= t) {
                        return Err("keyframe times must be finite and strictly increasing".into());
                    }
                    property_value(name, &key.value)?;
                    last = Some(key.time);
                }
            }
        }
    }
    if layers > MAX_LAYERS || keys > MAX_KEYS {
        return Err("bridge exceeds 20,000 layers or 100,000 keyframes".into());
    }
    check_graph(&d.comps)?;
    Ok(d)
}

/// Builds a fresh project only after validating the entire envelope; the caller's document is untouched on error.
pub fn import_unwae(bytes: &[u8]) -> Result<(Project, Vec<String>), String> {
    let d = decode(bytes)?;
    let mut p = Project::default();
    let mut warnings = d.warnings;
    warnings.push(
        "Experimental AE bridge: basic 2D layers and linear/hold transform keys only. Unsupported features are not preserved; keep the source AEP.".into(),
    );
    let mut ids = BTreeMap::new();
    for c in &d.comps {
        let mut comp = Comp::new(c.width, c.height, FrameRate::from_f64(c.fps), Tick::from_seconds_f64(c.duration));
        comp.pixel_aspect = c.pixel_aspect;
        comp.background = c.background;
        let id = p.add_item(&c.name, Label::default(), None, ItemKind::Comp(Arc::new(comp)));
        ids.insert(c.id.clone(), id);
    }
    let mut media = false;
    for c in d.comps {
        let id = *ids.get(&c.id).ok_or("missing mapped composition")?;
        let mut comp = p.comp(id).cloned().ok_or("missing native composition")?;
        for l in c.layers {
            let source = match l.kind.as_str() {
                "null" => LayerSource::Null,
                "text" => LayerSource::Text,
                "comp" => LayerSource::Comp { item: *ids.get(&l.source).ok_or("missing nested composition")? },
                "solid" => {
                    let item = p.add_item(
                        &l.name,
                        Label::default(),
                        None,
                        ItemKind::Solid(Solid { color: l.color, width: l.width, height: l.height, pixel_aspect: 1.0 }),
                    );
                    LayerSource::Solid { item }
                }
                "footage" => {
                    media = true;
                    let ext = std::path::Path::new(&l.source).extension().and_then(|x| x.to_str()).unwrap_or("").to_ascii_lowercase();
                    let kind = if ["png", "jpg", "jpeg", "tif", "tiff", "psd", "webp", "bmp"].contains(&ext.as_str()) {
                        FootageKind::Still
                    } else {
                        FootageKind::Video
                    };
                    let f = Footage {
                        path: l.source.clone(),
                        kind,
                        width: l.width,
                        height: l.height,
                        frame_rate: comp.frame_rate,
                        duration: Tick::from_seconds_f64((l.out_point - l.start_time).abs().max(comp.duration.seconds())),
                        has_video: true,
                        ..Footage::default()
                    };
                    let item = p.add_item(&l.name, Label::default(), None, ItemKind::Footage(f));
                    LayerSource::Footage { item }
                }
                _ => return Err("unsupported layer kind".into()),
            };
            let mut layer = build::layer(&mut p, &comp, &l.name, source, (l.width, l.height), None);
            layer.name = l.name;
            layer.start_time = Tick::from_seconds_f64(l.start_time);
            layer.in_point = Tick::from_seconds_f64(l.in_point);
            layer.out_point = Tick::from_seconds_f64(l.out_point);
            layer.stretch = l.stretch;
            layer.switches.video = l.enabled;
            // v1 does not transfer audio; make the limitation explicit rather than playing a wrong mix.
            layer.switches.audio = false;
            if let Some(prop) = layer.props.prop_mut("text/sourceText")
                && let Value::Text(doc) = &mut prop.value
            {
                doc.text = l.text;
                doc.size = l.font_size;
                if !l.font.is_empty() {
                    doc.font = l.font;
                }
                doc.fill = [l.color[0], l.color[1], l.color[2], 1.0];
            }
            for (name, property) in l.transform {
                let mut native_keys = Vec::new();
                for k in property.keys {
                    let t = layer.layer_time(Tick::from_seconds_f64(k.time));
                    let mut native = Keyframe::new(t, property_value(&name, &k.value)?);
                    if k.hold {
                        native = native.hold();
                    }
                    native_keys.push(native);
                }
                native_keys.sort_by_key(|k| k.time);
                let prop = layer.props.prop_mut(&format!("transform/{name}")).ok_or("missing standard transform property")?;
                prop.value = property_value(&name, &property.value)?;
                prop.keys = native_keys;
            }
            comp.layers.push(layer);
        }
        *p.comp_mut(id).ok_or("missing composition destination")? = comp;
    }
    if media {
        warnings.push("Footage is referenced, not embedded. Transfer media separately and use Relink if desktop paths are unavailable on this device. Audio is not transferred.".into());
    }
    Ok((p, warnings))
}
fn json_value(value: &Value) -> Option<Json> {
    match value {
        Value::Scalar(v) => Some(json!(v)),
        Value::Vec2(v) => Some(json!(v)),
        Value::Vec3(v) => Some(json!([v[0], v[1]])),
        _ => None,
    }
}
fn native_layer(p: &Project, layer: &Layer, warnings: &mut Vec<String>) -> Result<Option<BridgeLayer>, String> {
    if layer.switches.three_d || layer.parent.is_some() || layer.track_matte.is_some() || layer.switches.adjustment {
        warnings.push(format!("Omitted {}: 3D, parenting, adjustment or track matte is outside bridge v1.", layer.name));
        return Ok(None);
    }
    let mut out = BridgeLayer {
        name: layer.name.clone(),
        kind: String::new(),
        source: String::new(),
        width: 100,
        height: 100,
        color: white(),
        text: String::new(),
        font: String::new(),
        font_size: 48.0,
        start_time: layer.start_time.seconds(),
        in_point: layer.in_point.seconds(),
        out_point: layer.out_point.seconds(),
        stretch: layer.stretch,
        enabled: layer.switches.video,
        transform: BTreeMap::new(),
    };
    match &layer.source {
        LayerSource::Null => out.kind = "null".into(),
        LayerSource::Text => {
            out.kind = "text".into();
            if let Some(prop) = layer.props.prop("text/sourceText")
                && let Value::Text(doc) = &prop.value
            {
                out.text = doc.text.clone();
                out.font = doc.font.clone();
                out.font_size = doc.size;
                out.color = [doc.fill[0], doc.fill[1], doc.fill[2]];
                warnings.push(format!("{}: text transfers base font, size, fill and content only; mixed styling and text animation are omitted.", layer.name));
            }
        }
        LayerSource::Comp { item } => {
            let comp = p.comp(*item).ok_or_else(|| format!("{}: missing native composition source", layer.name))?;
            out.kind = "comp".into();
            out.source = item.0.to_string();
            out.width = comp.width;
            out.height = comp.height;
        }
        LayerSource::Solid { item } => {
            let item = p.item(*item).ok_or_else(|| format!("{}: missing native solid source", layer.name))?;
            let ItemKind::Solid(s) = &item.kind else {
                return Err(format!("{}: native solid source has the wrong item kind", layer.name));
            };
            out.kind = "solid".into();
            out.width = s.width;
            out.height = s.height;
            out.color = s.color;
        }
        LayerSource::Footage { item } => {
            let item = p.item(*item).ok_or_else(|| format!("{}: missing native footage source", layer.name))?;
            let ItemKind::Footage(f) = &item.kind else {
                return Err(format!("{}: native footage source has the wrong item kind", layer.name));
            };
            if f.kind == FootageKind::Audio || !f.sequence.is_empty() || f.layer.is_some() {
                warnings.push(format!("Omitted {}: audio, sequences and layered footage are not supported.", layer.name));
                return Ok(None);
            }
            out.kind = "footage".into();
            out.source = f.path.clone();
            out.width = f.width;
            out.height = f.height;
        }
        _ => {
            warnings.push(format!("Omitted {}: unsupported {} layer.", layer.name, layer.source.type_name()));
            return Ok(None);
        }
    }
    if layer.effects().is_some_and(|g| !g.children.is_empty()) || layer.masks().is_some_and(|g| !g.children.is_empty()) {
        warnings.push(format!("{}: masks and effects are omitted.", layer.name));
    }
    for name in ["anchor", "position", "scale", "rotation", "opacity"] {
        let prop = layer
            .props
            .prop(&format!("transform/{name}"))
            .ok_or_else(|| format!("{}: missing native transform property transform/{name}; export refused", layer.name))?;
        if prop.expr.as_ref().is_some_and(|e| e.enabled) {
            warnings.push(format!("{} {name}: expression omitted; base value and keys used.", layer.name));
        }
        let value = json_value(&prop.value).ok_or_else(|| format!("{}: unsupported native value for transform/{name}; export refused", layer.name))?;
        let mut keys = Vec::new();
        for k in &prop.keys {
            if (k.in_interp == Interp::Bezier || k.out_interp == Interp::Bezier || k.spatial_in != [0.0; 3] || k.spatial_out != [0.0; 3])
                && !warnings.iter().any(|w| w == "Bezier easing and spatial curves are approximated as linear keys.")
            {
                warnings.push("Bezier easing and spatial curves are approximated as linear keys.".into());
            }
            let value = json_value(&k.value).ok_or_else(|| format!("{}: unsupported native key value for transform/{name}; export refused", layer.name))?;
            keys.push(BridgeKey { time: layer.comp_time(k.time).seconds(), value, hold: k.out_interp == Interp::Hold });
        }
        keys.sort_by(|a, b| a.time.total_cmp(&b.time));
        out.transform.insert(name.into(), BridgeProperty { value, keys });
    }
    Ok(Some(out))
}
pub fn export_jsx(project: &Project) -> Result<String, String> {
    let mut warnings = vec!["Experimental bridge: only basic 2D layers, static base text styling and linear/hold transform keys. Audio, colour management, plugins and other features are omitted. Keep the native project.".into()];
    let mut comps = Vec::new();
    for (id, c) in project.comps() {
        let mut layers = Vec::new();
        for layer in &c.layers {
            if let Some(layer) = native_layer(project, layer, &mut warnings)? {
                layers.push(layer);
            }
        }
        comps.push(Composition {
            id: id.0.to_string(),
            name: project.item(*id).map_or_else(|| "Composition".into(), |i| i.name.clone()),
            width: c.width,
            height: c.height,
            fps: c.frame_rate.as_f64(),
            duration: c.duration.seconds(),
            pixel_aspect: c.pixel_aspect,
            background: c.background,
            layers,
        });
    }
    if warnings.len() > 999 {
        warnings.truncate(999);
        warnings.push("Additional omissions were reported; inspect the native project before trusting the reconstruction.".into());
    }
    let d = Document { format: FORMAT.into(), version: VERSION, warnings, comps };
    let bytes = serde_json::to_vec(&d).map_err(|e| format!("cannot serialize bridge: {e}"))?;
    // The same validator gates generated and incoming data, including the graph and allocation limits.
    decode(&bytes)?;
    let literal = String::from_utf8(bytes).map_err(|e| e.to_string())?.replace('\u{2028}', "\\u2028").replace('\u{2029}', "\\u2029");
    if IMPORTER.matches("__UNWRITTEN_PAYLOAD__").count() != 1 {
        return Err("invalid bundled AE importer template".into());
    }
    Ok(IMPORTER.replacen("__UNWRITTEN_PAYLOAD__", &literal, 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> Json {
        json!({"format":FORMAT,"version":1,"comps":[{"id":"main","name":"日本語 <main>","width":1280,"height":720,"fps":29.97,"duration":5,"layers":[
            {"name":"Solid","kind":"solid","width":1280,"height":720,"color":[0.1,0.2,0.3],"startTime":1,"inPoint":1,"outPoint":5,"stretch":200,"transform":{
                "position":{"value":[640,360],"keys":[{"time":1,"value":[0,0]},{"time":3,"value":[100,200],"hold":true}]}}},
            {"name":"Title","kind":"text","text":"Hello \"Cue\"","fontSize":60,"outPoint":5},
            {"name":"Nested","kind":"comp","source":"child","outPoint":5}]},
            {"id":"child","name":"Child","width":320,"height":240,"fps":24,"duration":5,"layers":[]}]})
    }
    #[test]
    fn imports_real_comps_layers_and_stretch_aware_keys() {
        let (p, _) = import_unwae(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        let c = p.find_by_name("日本語 <main>").unwrap().as_comp().unwrap();
        assert_eq!(c.layers.len(), 3);
        let l = c.layers.first().unwrap();
        assert_eq!(l.stretch, 200.0);
        let position = l.props.prop("transform/position").unwrap();
        assert_eq!(position.keys.get(1).unwrap().time, Tick::from_seconds_f64(1.0));
        assert_eq!(position.keys.get(1).unwrap().out_interp, Interp::Hold);
        assert!(matches!(c.layers.get(2).unwrap().source, LayerSource::Comp { .. }));
    }
    #[test]
    fn rejects_invalid_or_cyclic_documents_without_panicking() {
        for mutate in 0..5 {
            let mut v = fixture();
            match mutate {
                0 => v["version"] = json!(99),
                1 => v["comps"][0]["width"] = json!(0),
                2 => v["comps"][0]["layers"][0]["stretch"] = json!(0),
                3 => v["comps"][0]["layers"][2]["source"] = json!("main"),
                _ => v["comps"][1]["id"] = json!("main"),
            }
            assert!(import_unwae(&serde_json::to_vec(&v).unwrap()).is_err(), "case {mutate}");
        }
        assert!(import_unwae(b"garbage").is_err());
        assert!(import_unwae(&vec![b' '; MAX_BYTES + 1]).is_err());
    }
    #[test]
    fn rejects_3d_transform_vectors_in_2d_bridge() {
        let mut v = fixture();
        v["comps"][0]["layers"][0]["transform"]["position"]["value"] = json!([1, 2, 3]);
        assert!(import_unwae(&serde_json::to_vec(&v).unwrap()).is_err());
    }
    #[test]
    fn generated_jsx_contains_real_ae_reconstruction_and_safe_unicode() {
        let (p, _) = import_unwae(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        let script = export_jsx(&p).unwrap();
        assert!(script.contains("addComp"));
        assert!(script.contains("addSolid"));
        assert!(script.contains("日本語"));
        assert!(!script.contains("__UNWRITTEN_PAYLOAD__"));
        assert!(!script.contains("eval("));
        assert!(!script.contains("JSON.parse"));
    }
    #[test]
    fn generated_bridge_payload_reimports_supported_subset() {
        let (p, _) = import_unwae(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        let mut warnings = vec![];
        let c = p.find_by_name("日本語 <main>").unwrap().as_comp().unwrap();
        let b = native_layer(&p, c.layers.first().unwrap(), &mut warnings).unwrap().unwrap();
        assert_eq!(b.kind, "solid");
        assert_eq!(b.transform.get("position").unwrap().keys.get(1).unwrap().time, 3.0);
    }
    #[test]
    fn export_rejects_missing_native_transform_instead_of_silent_loss() {
        let (mut p, _) = import_unwae(&serde_json::to_vec(&fixture()).unwrap()).unwrap();
        let comp_id = p.find_by_name("日本語 <main>").unwrap().id;
        let c = p.comp(comp_id).unwrap();
        let layer_id = c.layers.first().unwrap().id;
        let position_uid = c.layers.first().unwrap().props.prop("transform/position").unwrap().uid;
        p.comp_mut(comp_id)
            .unwrap()
            .layers
            .iter_mut()
            .find(|l| l.id == layer_id)
            .unwrap()
            .props
            .group_mut("transform")
            .unwrap()
            .children
            .retain(|n| n.uid() != position_uid);
        let err = export_jsx(&p).unwrap_err();
        assert!(err.contains("missing native transform property transform/position"), "{err}");
    }
}
