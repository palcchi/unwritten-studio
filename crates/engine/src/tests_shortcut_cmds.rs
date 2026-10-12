//! The keyboard-only commands: time navigation, the work area, selection and layer timing.

use serde_json::json;

use crate::Session;

/// A 10 s, 30 fps comp with three solids (#1 on top), each trimmed to `spans` (seconds).
fn session(spans: [(f64, f64); 3]) -> (Session, Vec<u64>) {
    let mut s = Session::default();
    s.execute("comp.new", json!({"name": "Main", "width": 100, "height": 100, "frameRate": 30, "duration": 10})).unwrap();
    for _ in 0..3 {
        s.execute("layer.newSolid", json!({"color": "#ff0000"})).unwrap();
    }
    let ids: Vec<u64> = s.active_comp().unwrap().layers.iter().map(|l| l.id.0).collect();
    for (id, (a, b)) in ids.iter().zip(spans) {
        s.execute("layer.timing", json!({"layers": [id], "in": a, "out": b})).unwrap();
    }
    (s, ids)
}

fn secs(s: &Session) -> f64 {
    s.time().seconds()
}

#[test]
fn time_goes_to_the_work_area_and_to_in_and_out_points() {
    let (mut s, ids) = session([(1.0, 3.0), (2.0, 6.0), (0.0, 10.0)]);
    s.execute("comp.workArea", json!({"start": 2.0, "end": 4.0})).unwrap();
    s.execute("time.workAreaStart", json!({})).unwrap();
    assert_eq!(secs(&s), 2.0);
    s.execute("time.workAreaEnd", json!({})).unwrap();
    let frame = 1.0 / 30.0;
    assert!((secs(&s) - (4.0 - frame)).abs() < 1e-9);
    // In and Out points of the selected layers (an Out point is the layer's last frame).
    s.execute("layer.select", json!({"layers": [ids[0], ids[1]]})).unwrap();
    s.execute("time.set", json!({"time": 0.0})).unwrap();
    let mut seen = vec![];
    for _ in 0..5 {
        s.execute("time.nextInOut", json!({})).unwrap();
        seen.push(secs(&s));
    }
    let want = [1.0, 2.0, 3.0 - frame, 6.0 - frame, 6.0 - frame];
    assert!(seen.iter().zip(want).all(|(a, b)| (a - b).abs() < 1e-9), "{seen:?}");
    s.execute("time.previousInOut", json!({})).unwrap();
    assert!((secs(&s) - (3.0 - frame)).abs() < 1e-9);
    // With no layer selected: every layer's points.
    s.execute("edit.deselectAll", json!({})).unwrap();
    s.execute("time.set", json!({"time": 8.0})).unwrap();
    s.execute("time.nextInOut", json!({})).unwrap();
    assert!((secs(&s) - (10.0 - frame)).abs() < 1e-9);
}
