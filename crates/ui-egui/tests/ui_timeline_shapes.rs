//! Timeline rows for shape layers: the Contents "Add:" menu and the Classic 3D "Change
//! Renderer…" row (#206).

use effectcraft_engine::Session;
use effectcraft_engine::project::LayerId;
use effectcraft_ui_egui::{Dialog, EffectcraftApp};
use egui::{Event, Modifiers, PointerButton, Pos2, pos2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

fn harness(s: Session) -> Harness<'static, EffectcraftApp> {
    let mut h = Harness::builder().with_size(egui::vec2(1600.0, 1000.0)).build_eframe(|_| EffectcraftApp::new(s));
    h.run_steps(3);
    h
}

fn session() -> Session {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "Main", "width": 320, "height": 180, "duration": 4})).unwrap();
    s
}

fn center(h: &Harness<'_, EffectcraftApp>, id: &str) -> Pos2 {
    let e = h.state().auto.find(id).unwrap_or_else(|| panic!("no {id}"));
    pos2(e.rect[0] + e.rect[2] / 2.0, e.rect[1] + e.rect[3] / 2.0)
}

fn click_at(h: &mut Harness<'_, EffectcraftApp>, p: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(p));
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: true, modifiers: Modifiers::NONE });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: PointerButton::Primary, pressed: false, modifiers: Modifiers::NONE });
    h.run_steps(3);
}

fn click(h: &mut Harness<'_, EffectcraftApp>, id: &str) {
    let p = center(h, id);
    click_at(h, p);
}

fn open_layer(h: &mut Harness<'_, EffectcraftApp>, id: u64) {
    h.state_mut().ui.timeline.open_layers.insert(id);
    h.run_steps(2);
}

#[test]
fn shape_contents_add_menu_adds_path_operations() {
    let mut s = session();
    let l = s.execute("layer.newShape", json!({"kind": "ellipse"})).unwrap()["layer"].as_u64().unwrap();
    let contents = s.active_comp().unwrap().layer(LayerId(l)).unwrap().props.sub("contents").unwrap().uid;
    let mut h = harness(s);
    open_layer(&mut h, l);
    click(&mut h, &format!("timeline.group.{contents}.add"));
    let entry = h.get_by_label("Trim Paths").rect();
    assert!(entry.max.y <= 1000.0, "the whole menu fits in the window: {entry:?}");
    click_at(&mut h, entry.center());
    let layer = h.state().session.active_comp().unwrap().layer(LayerId(l)).unwrap().clone();
    let items: Vec<&str> = layer.props.sub("contents").unwrap().groups().map(|g| g.match_id.as_str()).collect();
    assert_eq!(items, ["group", "trim"]);
}

#[test]
fn classic_3d_shape_layers_offer_change_renderer() {
    let mut s = session();
    let l = s.execute("layer.newShape", json!({"kind": "rect"})).unwrap()["layer"].as_u64().unwrap();
    s.execute("layer.setSwitch", json!({"layers": [l], "switch": "threeD", "value": true})).unwrap();
    let mut h = harness(s);
    open_layer(&mut h, l);
    click(&mut h, &format!("timeline.layer.{l}.changeRenderer"));
    assert_eq!(h.state().dialog, Some(Dialog::CompSettings));
    assert!(h.state().auto.find("dialog.comp.renderer").is_some(), "on the 3D Renderer tab");
    // Advanced 3D shows the extrusion options instead.
    h.state_mut().dialog = None;
    h.state_mut().session.execute("comp.renderer", json!({"renderer": "advanced3d"})).unwrap();
    h.run_steps(3);
    assert!(h.state().auto.find(&format!("timeline.layer.{l}.changeRenderer")).is_none());
}

