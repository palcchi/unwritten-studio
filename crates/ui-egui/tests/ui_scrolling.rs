//! Long menus and lists scroll instead of running off the window, with a scroll bar to drag
//! (egui_kittest, UI logic only).

use effectcraft_engine::Session;
use effectcraft_ui_egui::EffectcraftApp;
use egui::{Event, Pos2, Rect, pos2, vec2};
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use serde_json::json;

fn harness(w: f32, h: f32) -> Harness<'static, EffectcraftApp> {
    let mut s = Session::default();
    s.execute("file.openDemoProject", json!({})).unwrap();
    let mut h = Harness::builder().with_size(vec2(w, h)).build_eframe(|_| EffectcraftApp::new(s));
    h.run_steps(3);
    h
}

fn rect(h: &Harness<'_, EffectcraftApp>, id: &str) -> Rect {
    let e = h.state().auto.find(id).unwrap_or_else(|| panic!("no {id}"));
    Rect::from_min_size(pos2(e.rect[0], e.rect[1]), vec2(e.rect[2], e.rect[3]))
}

fn click_at(h: &mut Harness<'_, EffectcraftApp>, p: Pos2) {
    h.input_mut().events.push(Event::PointerMoved(p));
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: true, modifiers: Default::default() });
    h.step();
    h.input_mut().events.push(Event::PointerButton { pos: p, button: egui::PointerButton::Primary, pressed: false, modifiers: Default::default() });
    h.run_steps(2);
}

fn wheel(h: &mut Harness<'_, EffectcraftApp>, at: Pos2, dy: f32) {
    h.input_mut().events.push(Event::PointerMoved(at));
    h.input_mut().events.push(Event::MouseWheel {
        unit: egui::MouseWheelUnit::Point,
        delta: vec2(0.0, dy),
        modifiers: Default::default(),
        phase: egui::TouchPhase::Move,
    });
    h.run_steps(20);
}

/// Edit is taller than a short window: it scrolls (with the wheel, and a scroll bar that always
/// shows) so its last entries can be reached and clicked, instead of running off the bottom of
/// the window (#269).
#[test]
fn a_menu_taller_than_the_window_scrolls() {
    let mut h = harness(1200.0, 520.0);
    assert!(!h.ctx.global_style().spacing.scroll.floating, "scroll bars show without hovering the list");
    let edit = rect(&h, "menu.Edit");
    click_at(&mut h, edit.center());
    let last = h.query_by_label_contains("Keyboard Shortcuts").expect("the Edit menu is open").rect();
    assert!(last.max.y > 520.0, "Edit is taller than the window ({last:?})");
    let first = h.query_by_label_contains("Quick Apply").expect("Quick Apply").rect();
    wheel(&mut h, first.center(), -2000.0);
    let last = h.query_by_label_contains("Keyboard Shortcuts").expect("still open").rect();
    assert!(last.min.y >= 0.0 && last.max.y <= 520.0, "scrolled into the window ({last:?})");
    click_at(&mut h, last.center());
    assert_eq!(h.state().dialog, Some(effectcraft_ui_egui::Dialog::Shortcuts), "the entry takes the click");
}
