//! Keyboard-driven commands that have no menu entry.

use std::collections::BTreeSet;

use effectcraft_project::{ItemId, LayerSource, Project};
use effectcraft_time::Tick;
use serde_json::{Value, json};

use super::{CommandSpec, always, comp_id, has_comp, layers_p};
use crate::{EngineError, Result, Session, cmd};

/// Ctrl+Alt+B: the work area spans the selected layers (their earliest In to their latest Out
/// point), or the whole composition with none selected.
fn work_area_to_selection(s: &mut Session, p: &Value) -> Result<Value> {
    let (cid, ids) = layers_p(s, p)?;
    s.edit("Work Area", None, |proj, _| {
        let c = proj.comp_mut(cid).ok_or(EngineError::NoComp)?;
        let fd = c.frame_duration();
        let spans: Vec<(Tick, Tick)> = c.layers.iter().filter(|l| ids.contains(&l.id)).map(|l| (l.in_point, l.out_point)).collect();
        let (start, end) = match (spans.iter().map(|s| s.0).min(), spans.iter().map(|s| s.1).max()) {
            (Some(a), Some(b)) => (a, b),
            _ => (Tick::ZERO, c.duration),
        };
        // min/max, not clamp: the bounds cross on a comp shorter than a frame.
        let start = start.min(c.duration - fd).max(Tick::ZERO);
        c.work_area = (start, end.min(c.duration).max(start + fd));
        Ok(())
    })?;
    let c = s.project.comp(cid).ok_or(EngineError::NoComp)?;
    Ok(json!({"start": c.work_area.0.seconds(), "end": c.work_area.1.seconds()}))
}

/// Shift+F2: no keyframe, property or property group is selected; the layers stay selected.
fn deselect_keys(s: &mut Session, _: &Value) -> Result<Value> {
    s.state.selected_keys.clear();
    s.state.selected_props.clear();
    s.state.selected_vertices.clear();
    Ok(Value::Null)
}

/// The comps connected to `from` by nesting, in either direction (its network of nested
/// compositions), `from` included.
fn hierarchy(proj: &Project, from: ItemId) -> BTreeSet<ItemId> {
    let mut edges: Vec<(ItemId, ItemId)> = Vec::new();
    for item in proj.items.values() {
        let Some(c) = item.as_comp() else { continue };
        for l in &c.layers {
            if let LayerSource::Comp { item: nested } = &l.source {
                edges.push((item.id, *nested));
            }
        }
    }
    let mut seen = BTreeSet::from([from]);
    let mut todo = vec![from];
    while let Some(c) = todo.pop() {
        for (a, b) in &edges {
            let other = if *a == c {
                *b
            } else if *b == c {
                *a
            } else {
                continue;
            };
            if seen.insert(other) {
                todo.push(other);
            }
        }
    }
    seen
}

/// Shift+Esc: activate the composition that was active most recently among those in the active
/// composition's nesting hierarchy.
fn activate_recent(s: &mut Session, p: &Value) -> Result<Value> {
    let cid = comp_id(s, p)?;
    let related = hierarchy(&s.project, cid);
    let next = s.state.recent_comps.iter().copied().find(|c| *c != cid && related.contains(c) && s.project.comp(*c).is_some());
    let Some(next) = next else { return Ok(Value::Null) };
    s.open_comp(next);
    Ok(json!({"comp": next.0}))
}

pub fn specs() -> Vec<CommandSpec> {
    vec![
        cmd!(
            "comp.workAreaToSelection",
            "Set Work Area to Selected Layers",
            [],
            Some("Cmd+Alt+B"),
            "{comp?, layers? (default the selected layers; the whole composition with none)}",
            has_comp,
            work_area_to_selection
        ),
        cmd!("comp.activateRecent", "Activate Most Recent Composition in Hierarchy", [], Some("Shift+Escape"), "{comp?}", has_comp, activate_recent),
        cmd!("keys.deselectAll", "Deselect All Keyframes and Properties", [], ["Shift+F2", "Cmd+Alt+Shift+A"], "{}", always, deselect_keys),
    ]
}
