//! Convert the persisted workspace schema to shared docking transactions.

use craft_ui::docking::{Action, Floating as SharedFloating, Layout as SharedLayout, Node, Placement, StackEntry as SharedStackEntry};
use craft_ui::layout::{SplitAxis, SplitSize as SharedSplitSize};

use crate::dock::{DockNode, Floating, Layout, PanelKind, SplitSize, StackEntry};

const MAX_DEPTH: usize = 32;
const MAX_NODES: usize = 4096;
const MAX_PANELS: usize = 128;

fn encode_node(node: &DockNode, depth: usize, count: &mut usize) -> Result<Node<PanelKind>, String> {
    *count = count.saturating_add(1);
    if depth > MAX_DEPTH || *count > MAX_NODES {
        return Err("workspace dock tree exceeds supported limits".into());
    }
    Ok(match node {
        DockNode::Tabs { panels, active } => {
            if panels.len() > MAX_PANELS {
                return Err("too many workspace tabs".into());
            }
            Node::Tabs { panels: panels.clone(), active: *active }
        }
        DockNode::Stack { entries } => {
            if entries.len() > MAX_PANELS {
                return Err("too many workspace stack entries".into());
            }
            Node::Stack { entries: entries.iter().map(|entry| SharedStackEntry { panel: entry.panel, open: entry.open, height: entry.height }).collect() }
        }
        DockNode::Split { vertical, size, a, b } => Node::Split {
            axis: if *vertical { SplitAxis::Vertical } else { SplitAxis::Horizontal },
            size: match *size {
                SplitSize::Ratio(value) => SharedSplitSize::Ratio(value),
                SplitSize::FixedA(value) => SharedSplitSize::FixedFirst(value),
                SplitSize::FixedB(value) => SharedSplitSize::FixedSecond(value),
            },
            first: Box::new(encode_node(a, depth.saturating_add(1), count)?),
            second: Box::new(encode_node(b, depth.saturating_add(1), count)?),
        },
    })
}

fn decode_node(node: Node<PanelKind>) -> DockNode {
    match node {
        Node::Tabs { panels, active } => DockNode::Tabs { panels, active },
        Node::Stack { entries } => {
            DockNode::Stack { entries: entries.into_iter().map(|entry| StackEntry { panel: entry.panel, open: entry.open, height: entry.height }).collect() }
        }
        Node::Split { axis, size, first, second } => DockNode::Split {
            vertical: axis == SplitAxis::Vertical,
            size: match size {
                SharedSplitSize::Ratio(value) => SplitSize::Ratio(value),
                SharedSplitSize::FixedFirst(value) => SplitSize::FixedA(value),
                SharedSplitSize::FixedSecond(value) => SplitSize::FixedB(value),
            },
            a: Box::new(decode_node(*first)),
            b: Box::new(decode_node(*second)),
        },
    }
}

pub(crate) fn encode(root: &DockNode, floating: &[Floating]) -> Result<SharedLayout<PanelKind>, String> {
    if floating.len() > MAX_PANELS || floating.iter().any(|group| group.panels.len() > MAX_PANELS) {
        return Err("too many floating workspace panels".into());
    }
    let root = if matches!(root, DockNode::Tabs { panels, active: 0 } if panels.is_empty()) { None } else { Some(encode_node(root, 0, &mut 0)?) };
    Ok(SharedLayout {
        root,
        floating: floating.iter().map(|group| SharedFloating { panels: group.panels.clone(), active: group.active, rect: group.rect }).collect(),
    })
}

pub(crate) fn decode(layout: SharedLayout<PanelKind>) -> Layout {
    Layout {
        root: layout.root.map(decode_node).unwrap_or_else(|| DockNode::Tabs { panels: vec![], active: 0 }),
        floating: layout.floating.into_iter().map(|group| Floating { panels: group.panels, active: group.active, rect: group.rect }).collect(),
    }
}

pub(crate) fn apply(layout: &mut Layout, action: Action<PanelKind>) -> Result<(), String> {
    let mut shared = encode(&layout.root, &layout.floating)?;
    shared.apply(action).map_err(|error| error.to_string())?;
    let proposed = decode(shared);
    encode(&proposed.root, &proposed.floating)?;
    *layout = proposed;
    Ok(())
}

pub(crate) fn apply_tree(root: &mut DockNode, action: Action<PanelKind>) -> bool {
    let Ok(mut shared) = encode(root, &[]) else { return false };
    if shared.apply(action).is_err() {
        return false;
    }
    let proposed = decode(shared).root;
    if encode(&proposed, &[]).is_err() {
        return false;
    }
    *root = proposed;
    true
}

pub(crate) fn move_in_tree(root: &mut DockNode, panel: PanelKind, anchor: PanelKind, placement: Placement<PanelKind>) -> bool {
    let Ok(mut shared) = encode(root, &[]) else { return false };
    if panel == anchor || !root.contains(anchor) {
        return false;
    }
    // This legacy entry point also opens new viewer/ScriptUI panels. Keep that explicit,
    // separate from Move, and commit only once both shared transactions have succeeded.
    if !root.contains(panel) && shared.apply(Action::Open { panel, anchor: Some(anchor) }).is_err() {
        return false;
    }
    if shared.apply(Action::Move { panel, anchor, placement }).is_err() {
        return false;
    }
    let proposed = decode(shared).root;
    if encode(&proposed, &[]).is_err() {
        return false;
    }
    *root = proposed;
    true
}

// Legacy close methods repair selection indices. Do that on a bounded encoded copy,
// then retain shared validation for geometry, duplicates and all other invariants.
fn normalize_selection(node: &mut Node<PanelKind>) {
    match node {
        Node::Tabs { panels, active } => *active = (*active).min(panels.len().saturating_sub(1)),
        Node::Split { first, second, .. } => {
            normalize_selection(first);
            normalize_selection(second);
        }
        Node::Stack { .. } => {}
    }
}

pub(crate) fn close_tree(root: &mut DockNode, panel: PanelKind) {
    let Ok(mut shared) = encode(root, &[]) else { return };
    if let Some(node) = &mut shared.root {
        normalize_selection(node);
    }
    if shared.validate().is_err() {
        return;
    }
    // An absent close still repairs an invalid selection, as the legacy method did.
    if root.contains(panel) && shared.apply(Action::Close { panel }).is_err() {
        return;
    }
    *root = decode(shared).root;
}

pub(crate) fn unfloat(layout: &mut Layout, panel: PanelKind) -> bool {
    let Ok(mut shared) = encode(&layout.root, &layout.floating) else { return false };
    let mut found = false;
    for group in &mut shared.floating {
        if group.panels.contains(&panel) {
            group.active = group.active.min(group.panels.len().saturating_sub(1));
            found = true;
        }
    }
    if !found || shared.apply(Action::Close { panel }).is_err() {
        return false;
    }
    *layout = decode(shared);
    true
}
