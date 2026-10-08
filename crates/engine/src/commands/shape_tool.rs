//! The shape tools (Rectangle, Rounded Rectangle, Ellipse, Polygon, Star) as a command: a drawn
//! shape goes into the selected shape layer's Contents as a new group, as in After Effects, or
//! into a new shape layer.

use effectcraft_geom::vec2;
use effectcraft_keyframe::Value as KV;
use effectcraft_project::build::{self, Ids};
use effectcraft_project::{Comp, ItemId, LayerId, LayerSource, Project, PropGroup};
use serde_json::{Value, json};

use super::layer::{c4, color_p, insert_layer, place, position_p};
use super::{CommandSpec, bad, comp_id, f_p, has_comp, layer_mut, str_p};
use crate::{EditorState, EngineError, Result, Session, cmd};

/// The shape tools' kinds, as `shape.newShape` / `layer.newShape` name them.
const KINDS: [&str; 5] = ["rect", "rounded", "ellipse", "polygon", "star"];

/// A new shape group for a shape tool `kind` of `size`, centred on the group's origin, with a
/// Stroke and a Fill ("Rectangle 1", "Ellipse 1" or "Polystar 1"). None for another kind.
pub(crate) fn shape_group(ids: &mut Ids, kind: &str, size: [f64; 2], fill: Option<[f64; 4]>, stroke: Option<([f64; 4], f64)>) -> Option<PropGroup> {
    let (path, gname) = match kind {
        "rect" | "rectangle" => (build::shape_rect(ids, size, [0.0, 0.0], 0.0), "Rectangle 1"),
        "rounded" | "roundedRect" => (build::shape_rect(ids, size, [0.0, 0.0], size[0].min(size[1]) * 0.15), "Rectangle 1"),
        "ellipse" => (build::shape_ellipse(ids, size, [0.0, 0.0]), "Ellipse 1"),
        "star" => (build::shape_star(ids, true, 5.0, [0.0, 0.0], size[0] / 2.0, size[0] / 4.0), "Polystar 1"),
        "polygon" => (build::shape_star(ids, false, 6.0, [0.0, 0.0], size[0] / 2.0, 0.0), "Polystar 1"),
        _ => return None,
    };
    let mut items = vec![path];
    if let Some((c, w)) = stroke {
        items.push(build::shape_stroke(ids, c, w));
    }
    if let Some(c) = fill {
        items.push(build::shape_fill(ids, c));
    }
    Some(build::shape_group(ids, gname, items))
}

/// The shape layer a shape tool or the Pen draws into: `layer` (which must be a shape layer),
/// else the first selected unlocked shape layer, else none (a new shape layer).
pub(crate) fn draw_target(s: &Session, comp: &Comp, p: &Value, cmd: &str) -> Result<Option<LayerId>> {
    let target = match p.get("layer") {
        Some(l) => Some(super::resolve_layer(comp, l).ok_or_else(|| bad(cmd, format!("no layer {l}")))?),
        None => s.state.selected_layers.iter().copied().find(|l| comp.layer(*l).is_some_and(|l| matches!(l.source, LayerSource::Shape) && !l.switches.locked)),
    };
    if target.and_then(|l| comp.layer(l)).is_some_and(|l| !matches!(l.source, LayerSource::Shape)) {
        return Err(bad(cmd, "the layer is not a shape layer"));
    }
    Ok(target)
}

/// A new empty shape layer ("Shape Layer n" unless `name`) with its Position at `position` (comp
/// pixels; else the comp centre), above the selected layer and selected.
pub(crate) fn new_shape_layer(
    proj: &mut Project,
    st: &mut EditorState,
    comp: &Comp,
    cid: ItemId,
    name: Option<&str>,
    position: Option<[f64; 2]>,
) -> Result<LayerId> {
    let count = comp.layers.iter().filter(|l| matches!(l.source, LayerSource::Shape)).count();
    let name = name.map(str::to_string).unwrap_or_else(|| format!("Shape Layer {}", count + 1));
    let mut l = build::layer(proj, comp, &name, LayerSource::Shape, (comp.width, comp.height), None);
    place(&mut l, position);
    insert_layer(proj, st, cid, l)
}

/// Put `g` on top of the shape layer's Contents with a name unique there ("Rectangle 2"…) and
/// its Transform's Position at `position` (layer space). Returns its uid.
pub(crate) fn add_to_contents(
    proj: &mut Project,
    cid: ItemId,
    lid: LayerId,
    mut g: PropGroup,
    position: [f64; 2],
    cmd: &str,
) -> Result<effectcraft_project::Uid> {
    let contents = layer_mut(proj, cid, lid)?.props.sub_mut("contents").ok_or_else(|| bad(cmd, "the layer has no contents"))?;
    g.name = super::effect::unique_name(contents, &g.name);
    if let Some(pr) = g.sub_mut("transform").and_then(|t| t.get_mut("position")) {
        pr.value = KV::Vec2(position);
    }
    let uid = g.uid;
    contents.children.insert(0, g.into());
    Ok(uid)
}

/// A finite `[w, h]` / `[x, y]` parameter.
fn pair_p(p: &Value, k: &str) -> Option<[f64; 2]> {
    let a = p.get(k)?.as_array()?;
    Some([a.first()?.as_f64()?, a.get(1)?.as_f64()?]).filter(|v| v.iter().all(|x| x.is_finite()))
}

/// The shape tools: a rectangle, rounded rectangle, ellipse, polygon or star of `size`, centred
/// at `position`. With a shape layer given or selected it goes on top of that layer's Contents
/// as a new group ("Rectangle 2"…) placed by the group's Transform, as After Effects draws into
/// the selected shape layer; otherwise a new shape layer is centred on it. `position` and `size`
/// are in comp pixels, or the layer's space with `space: "layer"`.
fn new_shape(s: &mut Session, p: &Value) -> Result<Value> {
    let c = "shape.newShape";
    let cid = comp_id(s, p)?;
    let comp = s.project.comp(cid).ok_or(EngineError::NoComp)?.clone();
    let kind = str_p(p, "kind").unwrap_or("rect");
    if !KINDS.contains(&kind) {
        return Err(bad(c, format!("unknown kind `{kind}`; one of: {}", KINDS.join(", "))));
    }
    let size = match p.get("size") {
        None => [200.0, 200.0],
        Some(_) => pair_p(p, "size").filter(|s| s.iter().all(|v| *v >= 0.0)).ok_or_else(|| bad(c, "size: [w, h], not negative"))?,
    };
    let pos = position_p(p, c)?.unwrap_or([comp.width as f64 / 2.0, comp.height as f64 / 2.0]);
    let fill = Some(c4(color_p(p, "fill"), [0.25, 0.55, 1.0, 1.0]));
    let stroke = (f_p(p, "strokeWidth").unwrap_or(0.0) > 0.0).then(|| (c4(color_p(p, "stroke"), [1.0, 1.0, 1.0, 1.0]), f_p(p, "strokeWidth").unwrap_or(2.0)));
    let target = draw_target(s, &comp, p, c)?;
    // In the target layer's space: the centre maps through the layer's transform, the size by
    // its scale.
    let (centre, size) = match target.and_then(|l| comp.layer(l)) {
        Some(l) if str_p(p, "space") != Some("layer") => {
            let ctx = effectcraft_render::EvalCtx::new(&s.project, cid, &comp, s.time_of(cid));
            let inv = ctx.layer_to_comp(l).0.inverse().ok_or_else(|| bad(c, "the layer is scaled to nothing"))?;
            let q = inv.apply(vec2(pos[0], pos[1]));
            ([q.x, q.y], [inv.apply_vec(vec2(size[0], 0.0)).length(), inv.apply_vec(vec2(0.0, size[1])).length()])
        }
        Some(_) => (pos, size),
        None => ([0.0, 0.0], size),
    };
    let label = match kind {
        "rounded" => "Rounded Rectangle Tool",
        "ellipse" => "Ellipse Tool",
        "polygon" => "Polygon Tool",
        "star" => "Star Tool",
        _ => "Rectangle Tool",
    };
    let name = str_p(p, "name");
    let (lid, group) = s.edit(label, None, |proj, st| {
        let lid = match target {
            Some(l) => l,
            None => new_shape_layer(proj, st, &comp, cid, name, Some(pos))?,
        };
        let mut next = proj.next_id;
        let g = shape_group(&mut Ids(&mut next), kind, size, fill, stroke).ok_or_else(|| bad(c, "unknown kind"))?;
        proj.next_id = next;
        let group = add_to_contents(proj, cid, lid, g, centre, c)?;
        st.selected_layers = vec![lid];
        Ok((lid, group))
    })?;
    Ok(json!({"layer": lid.0, "group": group}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![cmd!(
        "shape.newShape",
        "Shape Tool",
        [],
        None,
        "{layer?, kind?: rect|rounded|ellipse|polygon|star, size? [w,h], position? [x,y] (the centre), space?: comp|layer, name? (a new layer's), fill?, stroke?, strokeWidth?} → {layer, group}",
        has_comp,
        new_shape
    )]
}
