//! Turning by key, pedal, tap and swipe (feature-map TURN-1..4; spec `docs/spec/page-turning.md`).
//! Frontends pass raw input here so the same rules hold in the app, the web build, the CLI and
//! for agents; the mapped reader command does the turn.

use serde_json::{Map, Value, json};

use scorecraft_input::{
    Action, Chord, Gesture, PROFILES, PedalMap, Profile, Zone, classify, tap_zone,
};

/// The clock's time in milliseconds, for the wheel's rest.
fn now_ms(s: &Session) -> u64 {
    (s.clock)()
}

use crate::commands::{
    CommandSpec, always, bad, opt_bool, opt_f64, opt_str, params_object, reject_unknown_keys,
    req_str,
};
use crate::{EngineError, Session};

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, ours, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours,
        enabled: always,
        run,
    };
    vec![
        spec(
            "input.pedal.map",
            "Pedals and Keys…",
            "Settings",
            r#"{"profile":"default|leftRight|upDown|pageUpDown"?,"map":{key:"next|previous|first|last"}?,"set":{key:"next|previous|first|last"|null}?}"#,
            false,
            pedal_map as _,
        ),
        spec(
            "input.key",
            "Key or Pedal Press",
            "",
            r#"{"key":string,"repeat":bool=false}"#,
            true,
            key as _,
        ),
        spec(
            "input.tap",
            "Tap the Reader",
            "",
            r#"{"x":number,"y":number?}"#,
            true,
            tap as _,
        ),
        spec(
            "input.swipe",
            "Swipe the Reader",
            "",
            r#"{"dx":number,"dy":number=0}"#,
            true,
            swipe as _,
        ),
        spec(
            "input.swipe.set",
            "Swipe to Turn Pages",
            "Settings",
            r#"{"on":bool?}"#,
            true,
            swipe_set as _,
        ),
        spec(
            "input.wheel",
            "Scroll the Reader",
            "",
            r#"{"dy":number}"#,
            true,
            wheel as _,
        ),
        spec(
            "input.secondaryClick",
            "Right-Click the Reader",
            "",
            "{}",
            true,
            secondary_click as _,
        ),
    ]
}

fn map_json(map: &PedalMap) -> Value {
    json!({
        "map": serde_json::to_value(map).unwrap_or(Value::Null),
        "profiles": PROFILES.iter().map(|p| json!({ "id": p.id, "label": p.label })).collect::<Vec<_>>(),
    })
}

fn parse_entries(
    id: &str,
    v: &Map<String, Value>,
    allow_null: bool,
) -> Result<Vec<(Chord, Option<Action>)>, EngineError> {
    if v.len() > scorecraft_input::MAX_ENTRIES {
        return Err(bad(
            id,
            format!("more than {} keys", scorecraft_input::MAX_ENTRIES),
        ));
    }
    v.iter()
        .map(|(k, a)| {
            let chord = Chord::parse(k).ok_or_else(|| {
                bad(
                    id,
                    format!("`{}` is not a key", crate::commands::truncate(k, 40)),
                )
            })?;
            let action =
                match a {
                    Value::Null if allow_null => None,
                    Value::String(s) => Some(Action::from_id(s).ok_or_else(|| {
                        bad(id, "actions are next, previous, first or last".into())
                    })?),
                    _ => return Err(bad(id, "actions are next, previous, first or last".into())),
                };
            Ok((chord, action))
        })
        .collect()
}

/// Reads or changes the pedal and key map (TURN-4). `profile` starts from a profile, `map`
/// replaces the whole map, `set` changes single keys (`null` unmaps). Applied in that order.
fn pedal_map(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.pedal.map";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["profile", "map", "set"])?;
    let obj = |key: &str| -> Result<Option<&Map<String, Value>>, EngineError> {
        match p.and_then(|m| m.get(key)) {
            None | Some(Value::Null) => Ok(None),
            Some(Value::Object(o)) => Ok(Some(o)),
            Some(_) => Err(bad(ID, format!("`{key}` must be an object of key: action"))),
        }
    };
    let mut map = s.pedal_map.clone();
    if let Some(id) = opt_str(ID, p, "profile")? {
        let profile: &Profile =
            Profile::by_id(id).ok_or_else(|| bad(ID, "no such pedal profile".into()))?;
        map = PedalMap::profile(profile);
    }
    if let Some(m) = obj("map")? {
        map = PedalMap::empty();
        for (c, a) in parse_entries(ID, m, false)? {
            map.set(c, a);
        }
    }
    if let Some(m) = obj("set")? {
        for (c, a) in parse_entries(ID, m, true)? {
            if !map.set(c, a) {
                return Err(bad(
                    ID,
                    format!(
                        "the map holds at most {} keys",
                        scorecraft_input::MAX_ENTRIES
                    ),
                ));
            }
        }
    }
    s.pedal_map = map;
    Ok(map_json(&s.pedal_map))
}

/// Runs a reader action. With no score open nothing turns and `handled` is false.
fn perform(s: &mut Session, action: Option<Action>) -> Result<Value, EngineError> {
    let Some(action) = action else {
        return Ok(json!({ "handled": false }));
    };
    if !s.has_score() {
        return Ok(json!({ "handled": false, "action": action.id() }));
    }
    let (command, to) = action.command();
    let params = match to {
        Some(to) => json!({ "to": to }),
        None => json!({}),
    };
    let state = s.dispatch(command, &params)?;
    Ok(json!({ "handled": true, "action": action.id(), "command": command, "state": state }))
}

fn key(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.key";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["key", "repeat"])?;
    let text = req_str(ID, p, "key")?;
    let chord = Chord::parse(text).ok_or_else(|| bad(ID, "`key` is not a key".into()))?;
    let repeat = opt_bool(ID, p, "repeat")?.unwrap_or(false);
    let now = s.now_ms();
    let mut out = if s.key_gate.press(&chord, repeat, now) {
        let action = s.pedal_map.action(&chord);
        perform(s, action)?
    } else {
        json!({ "handled": false, "dropped": true })
    };
    crate::commands::set(&mut out, "key", json!(chord.to_string()));
    Ok(out)
}

fn finite(id: &str, p: Option<&Map<String, Value>>, key: &str) -> Result<Option<f64>, EngineError> {
    match opt_f64(id, p, key)? {
        Some(v) if v.is_finite() && v.abs() <= 1e7 => Ok(Some(v)),
        Some(_) => Err(bad(id, format!("`{key}` is out of range"))),
        None => Ok(None),
    }
}

/// A tap at `x`, `y` (frontend pixels from the reader area's top left). The left half turns
/// back, the right half forward; the band along the top returns `zone: "controls"` for the
/// frontend to show or hide its controls.
fn tap(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.tap";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["x", "y"])?;
    let x = finite(ID, p, "x")?.ok_or_else(|| bad(ID, "`x` is required".into()))?;
    let y = finite(ID, p, "y")?;
    let (width, height) = s.score.as_ref().map_or((0.0, 0.0), |o| {
        (o.reader.viewport.width, o.reader.viewport.height)
    });
    let zone: Zone = tap_zone(x, y, width, height);
    let mut out = perform(s, zone.action())?;
    crate::commands::set(&mut out, "zone", json!(zone.id()));
    Ok(out)
}

/// A pointer movement of `dx`, `dy` from press to release. Zoomed in, it pans instead.
fn swipe(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.swipe";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["dx", "dy"])?;
    let dx = finite(ID, p, "dx")?.ok_or_else(|| bad(ID, "`dx` is required".into()))?;
    let dy = finite(ID, p, "dy")?.unwrap_or(0.0);
    let zoomed = s.score.as_ref().is_some_and(|o| o.reader.zoom.factor > 1.0);
    if zoomed {
        let state = s.dispatch("view.pan", &json!({ "dx": dx, "dy": dy }))?;
        return Ok(json!({ "handled": true, "gesture": "pan", "state": state }));
    }
    let g = classify(dx, dy);
    let action = if s.swipe_turns { g.action() } else { None };
    let mut out = perform(s, action)?;
    let name = match g {
        Gesture::Tap => "tap",
        Gesture::SwipeLeft => "swipeLeft",
        Gesture::SwipeRight => "swipeRight",
        Gesture::Other => "other",
    };
    crate::commands::set(&mut out, "gesture", json!(name));
    Ok(out)
}

/// Reads or sets *Swipe to turn pages*; without `on` it reads.
fn swipe_set(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.swipe.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    if let Some(on) = opt_bool(ID, p, "on")? {
        s.swipe_turns = on;
    }
    Ok(json!({ "on": s.swipe_turns }))
}

/// `dy` pixels of scroll-wheel movement over the reader (positive: wheel down). Zoomed in, it
/// pans; otherwise wheel travel turns one page per step (down: forward), then rests briefly.
fn wheel(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.wheel";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["dy"])?;
    let dy = finite(ID, p, "dy")?.ok_or_else(|| bad(ID, "`dy` is required".into()))?;
    let zoomed = s.score.as_ref().is_some_and(|o| o.reader.zoom.factor > 1.0);
    if zoomed {
        let state = s.dispatch("view.pan", &json!({ "dx": 0, "dy": -dy }))?;
        return Ok(json!({ "handled": true, "gesture": "pan", "state": state }));
    }
    let now = now_ms(s);
    let action = s.wheel_gate.wheel(dy, now);
    perform(s, action)
}

/// A right click on the reader turns back a page.
fn secondary_click(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.secondaryClick";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    perform(s, Some(Action::Previous))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};

    use super::*;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

    /// A session with a clock that moves 1 s per reading, so the debounce never drops presses.
    fn session(pages: usize) -> Session {
        let mut s = Session::new();
        let t = Arc::new(AtomicU64::new(0));
        s.set_clock(Box::new(move || t.fetch_add(1000, Ordering::Relaxed)));
        let bytes = PdfBuilder::new().pages(pages, PageSpec::default()).build();
        s.open_score_bytes(Arc::new(bytes), None, "test.pdf")
            .unwrap();
        s
    }

    fn page(r: &Value) -> Option<u64> {
        r["state"]["page"].as_u64()
    }

    #[test]
    fn default_keys_turn_and_repeats_are_dropped() {
        let mut s = session(4);
        let r = s
            .dispatch("input.key", &json!({ "key": "PageDown" }))
            .unwrap();
        assert_eq!((r["action"].as_str(), page(&r)), (Some("next"), Some(2)));
        let r = s
            .dispatch("input.key", &json!({ "key": "PageDown", "repeat": true }))
            .unwrap();
        assert_eq!(r["dropped"], true);
        let r = s.dispatch("input.key", &json!({ "key": " " })).unwrap();
        assert_eq!(page(&r), Some(3));
        let r = s
            .dispatch("input.key", &json!({ "key": "shift+space" }))
            .unwrap();
        assert_eq!(page(&r), Some(2));
        let r = s.dispatch("input.key", &json!({ "key": "End" })).unwrap();
        assert_eq!(page(&r), Some(4));
        let r = s.dispatch("input.key", &json!({ "key": "Q" })).unwrap();
        assert_eq!(r["handled"], false);
    }

    #[test]
    fn bouncing_pedal_turns_once() {
        let mut s = Session::new();
        s.set_clock(Box::new(|| 5_000));
        let bytes = PdfBuilder::new().pages(3, PageSpec::default()).build();
        s.open_score_bytes(Arc::new(bytes), None, "t.pdf").unwrap();
        s.dispatch("input.key", &json!({ "key": "PageDown" }))
            .unwrap();
        let r = s
            .dispatch("input.key", &json!({ "key": "PageDown" }))
            .unwrap();
        assert_eq!(r["dropped"], true);
    }

    #[test]
    fn remapping_keys() {
        let mut s = session(3);
        let r = s
            .dispatch("input.pedal.map", &json!({ "profile": "pageUpDown" }))
            .unwrap();
        assert_eq!(
            r["map"],
            json!({ "PageDown": "next", "PageUp": "previous" })
        );
        let r = s
            .dispatch(
                "input.pedal.map",
                &json!({ "set": { "B": "next", "PageUp": null } }),
            )
            .unwrap();
        assert_eq!(r["map"], json!({ "PageDown": "next", "B": "next" }));
        let r = s.dispatch("input.key", &json!({ "key": "b" })).unwrap();
        assert_eq!(page(&r), Some(2));
        let r = s
            .dispatch("input.key", &json!({ "key": "ArrowRight" }))
            .unwrap();
        assert_eq!(r["handled"], false);
        let r = s
            .dispatch("input.pedal.map", &json!({ "map": { "Enter": "last" } }))
            .unwrap();
        assert_eq!(r["map"], json!({ "Enter": "last" }));
        // Reading leaves it alone.
        let r = s.dispatch("input.pedal.map", &json!({})).unwrap();
        assert_eq!(r["map"], json!({ "Enter": "last" }));
        assert_eq!(r["profiles"].as_array().unwrap().len(), 4);
    }

    #[test]
    fn taps_and_swipes() {
        let mut s = session(3);
        s.dispatch("view.viewport.set", &json!({ "width": 900, "height": 600 }))
            .unwrap();
        let r = s
            .dispatch("input.tap", &json!({ "x": 850, "y": 300 }))
            .unwrap();
        assert_eq!((r["zone"].as_str(), page(&r)), (Some("forward"), Some(2)));
        // Two halves: just right of the middle turns forward.
        let r = s
            .dispatch("input.tap", &json!({ "x": 460, "y": 300 }))
            .unwrap();
        assert_eq!((r["zone"].as_str(), page(&r)), (Some("forward"), Some(3)));
        // The band along the top shows or hides the controls.
        let r = s
            .dispatch("input.tap", &json!({ "x": 450, "y": 20 }))
            .unwrap();
        assert_eq!(
            (r["zone"].as_str(), r["handled"].as_bool()),
            (Some("controls"), Some(false))
        );
        let r = s.dispatch("input.tap", &json!({ "x": 20 })).unwrap();
        assert_eq!(page(&r), Some(2));
        let r = s.dispatch("input.tap", &json!({ "x": 20 })).unwrap();
        assert_eq!(page(&r), Some(1));
        let r = s
            .dispatch("input.swipe", &json!({ "dx": -200, "dy": 10 }))
            .unwrap();
        assert_eq!(
            (r["gesture"].as_str(), page(&r)),
            (Some("swipeLeft"), Some(2))
        );
        let r = s.dispatch("input.swipe", &json!({ "dx": 30 })).unwrap();
        assert_eq!(r["handled"], false);
        // *Swipe to turn pages* off: a swipe does nothing.
        let r = s
            .dispatch("input.swipe.set", &json!({ "on": false }))
            .unwrap();
        assert_eq!(r["on"], false);
        let r = s.dispatch("input.swipe", &json!({ "dx": 200 })).unwrap();
        assert_eq!(
            (r["handled"].as_bool(), r["gesture"].as_str()),
            (Some(false), Some("swipeRight"))
        );
        assert_eq!(
            s.dispatch("input.swipe.set", &json!({})).unwrap()["on"],
            false
        );
        s.dispatch("input.swipe.set", &json!({ "on": true }))
            .unwrap();
        s.dispatch("view.zoom.set", &json!({ "zoom": 3 })).unwrap();
        let r = s.dispatch("input.swipe", &json!({ "dx": -200 })).unwrap();
        assert_eq!((r["gesture"].as_str(), page(&r)), (Some("pan"), Some(2)));
    }

    #[test]
    fn the_wheel_and_right_click_turn() {
        let clock = Arc::new(AtomicU64::new(1_000));
        let c = clock.clone();
        let mut s = session(3);
        s.set_clock(Box::new(move || c.load(Ordering::SeqCst)));
        s.dispatch("view.viewport.set", &json!({ "width": 900, "height": 600 }))
            .unwrap();
        let r = s.dispatch("input.wheel", &json!({ "dy": 30 })).unwrap();
        assert_eq!(r["handled"], false);
        let r = s.dispatch("input.wheel", &json!({ "dy": 40 })).unwrap();
        assert_eq!(page(&r), Some(2));
        // Resting after a turn.
        let r = s.dispatch("input.wheel", &json!({ "dy": 200 })).unwrap();
        assert_eq!(r["handled"], false);
        clock.fetch_add(1_000, Ordering::SeqCst);
        let r = s.dispatch("input.wheel", &json!({ "dy": -80 })).unwrap();
        assert_eq!(page(&r), Some(1));
        let r = s.dispatch("input.secondaryClick", &json!({})).unwrap();
        assert_eq!(r["action"], "previous");
        s.dispatch("view.page.goto", &json!({ "page": 3 })).unwrap();
        let r = s.dispatch("input.secondaryClick", &json!({})).unwrap();
        assert_eq!(page(&r), Some(2));
        // Enter and Backspace turn by default.
        clock.fetch_add(1_000, Ordering::SeqCst);
        let r = s.dispatch("input.key", &json!({ "key": "Enter" })).unwrap();
        assert_eq!(page(&r), Some(3));
        clock.fetch_add(1_000, Ordering::SeqCst);
        let r = s
            .dispatch("input.key", &json!({ "key": "Backspace" }))
            .unwrap();
        assert_eq!(page(&r), Some(2));
        // Zoomed in, the wheel pans.
        s.dispatch("view.zoom.set", &json!({ "zoom": 3 })).unwrap();
        let r = s.dispatch("input.wheel", &json!({ "dy": 100 })).unwrap();
        assert_eq!((r["gesture"].as_str(), page(&r)), (Some("pan"), Some(2)));
    }

    #[test]
    fn without_a_score_nothing_turns() {
        let mut s = Session::new();
        let r = s
            .dispatch("input.key", &json!({ "key": "PageDown" }))
            .unwrap();
        assert_eq!(r["handled"], false);
        let r = s.dispatch("input.tap", &json!({ "x": 10 })).unwrap();
        assert_eq!(r["handled"], false);
    }

    #[test]
    fn bad_params_are_errors() {
        let mut s = session(2);
        for (id, p) in [
            ("input.key", json!({})),
            ("input.key", json!({ "key": "Hyper+X" })),
            ("input.key", json!({ "key": 5 })),
            ("input.tap", json!({})),
            ("input.tap", json!({ "x": 1e300 })),
            ("input.swipe", json!({ "dy": 3 })),
            ("input.pedal.map", json!({ "profile": "harp" })),
            ("input.pedal.map", json!({ "set": { "X": "jump" } })),
            ("input.pedal.map", json!({ "map": { "X": null } })),
            ("input.pedal.map", json!({ "map": [] })),
            ("input.pedal.map", json!({ "colour": 1 })),
        ] {
            assert_eq!(
                s.dispatch(id, &p).unwrap_err().code(),
                "bad_params",
                "{id} {p}"
            );
        }
    }
}
