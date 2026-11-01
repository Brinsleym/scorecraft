//! The metronome (feature-map PRAC-1, spec `docs/spec/performance.md`): start and stop, tempo
//! and beats per bar, tap tempo, and where the beat is now, for the shell's pulse. A playing
//! list's entry tempo sets it (LIST-3).

use serde_json::{Value, json};

use scorecraft_practice::{MAX_BPM, METERS, MIN_BPM};

use crate::commands::{
    CommandSpec, always, bad, opt_bool, opt_f64, opt_int, params_object, reject_unknown_keys,
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
            "practice.metronome.toggle",
            "Metronome",
            "Practice",
            r#"{"on":bool?}"#,
            false,
            toggle as _,
        ),
        spec(
            "practice.metronome.tap",
            "Tap Tempo",
            "Practice",
            "{}",
            false,
            tap as _,
        ),
        spec(
            "practice.metronome.set",
            "Metronome Tempo",
            "",
            r#"{"bpm":20..300?,"beats":2|3|4|6?}"#,
            false,
            set as _,
        ),
        spec(
            "practice.metronome.state",
            "Metronome State",
            "",
            "{}",
            true,
            state as _,
        ),
    ]
}

pub(crate) fn state_json(session: &Session) -> Value {
    let m = &session.metronome;
    let beat = m.beat_at(session.now_ms());
    json!({
        "running": m.running(),
        "bpm": m.bpm(),
        "beats": m.beats(),
        "beat": beat.map(|b| json!({
            "count": b.count,
            "inBar": b.in_bar + 1,
            "accent": b.accent,
            "phase": b.phase,
            "nextMs": b.next_ms,
        })),
    })
}

fn toggle(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "practice.metronome.toggle";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.metronome.running());
    let now = session.now_ms();
    if on {
        session.metronome.start(now);
    } else {
        session.metronome.stop();
    }
    Ok(state_json(session))
}

fn tap(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "practice.metronome.tap";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    let now = session.now_ms();
    let set = session.metronome.tap(now);
    let mut v = state_json(session);
    crate::commands::set(&mut v, "tapped", json!(set));
    Ok(v)
}

fn set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "practice.metronome.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["bpm", "beats"])?;
    let bpm = opt_f64(ID, p, "bpm")?;
    let beats = opt_int(ID, p, "beats", 1, 16)?;
    if let Some(b) = bpm
        && !(MIN_BPM..=MAX_BPM).contains(&b)
    {
        return Err(bad(
            ID,
            format!("`bpm` must be from {MIN_BPM} to {MAX_BPM}"),
        ));
    }
    if let Some(b) = beats
        && !METERS.iter().any(|m| u64::from(*m) == b)
    {
        return Err(bad(ID, "`beats` must be 2, 3, 4 or 6".into()));
    }
    let now = session.now_ms();
    if let Some(b) = bpm {
        session.metronome.set_bpm(b, now);
    }
    if let Some(b) = beats.and_then(|b| u8::try_from(b).ok()) {
        session.metronome.set_beats(b);
    }
    Ok(state_json(session))
}

fn state(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "practice.metronome.state";
    reject_unknown_keys(ID, params_object(ID, params)?, &[])?;
    Ok(state_json(session))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
    use std::sync::{Arc, Mutex};

    use super::*;

    #[test]
    fn perf_8_the_metronome_runs_on_the_session_clock() {
        let clock = Arc::new(Mutex::new(10_000u64));
        let c = clock.clone();
        let mut s = Session::new();
        s.set_clock(Box::new(move || *c.lock().unwrap()));
        let r = s
            .dispatch("practice.metronome.set", &json!({ "bpm": 120, "beats": 3 }))
            .unwrap();
        assert_eq!(
            (r["bpm"].as_f64(), r["running"].as_bool()),
            (Some(120.0), Some(false))
        );
        s.dispatch("practice.metronome.toggle", &json!({})).unwrap();
        *clock.lock().unwrap() += 1_100;
        let st = s.dispatch("practice.metronome.state", &json!({})).unwrap();
        assert_eq!(st["beat"]["inBar"], 3);
        for t in [0u64, 400, 400, 400] {
            *clock.lock().unwrap() += t;
            s.dispatch("practice.metronome.tap", &json!({})).unwrap();
        }
        assert_eq!(
            s.dispatch("practice.metronome.state", &json!({})).unwrap()["bpm"],
            150.0
        );
        for bad in [
            json!({ "bpm": 10 }),
            json!({ "beats": 5 }),
            json!({ "tempo": 1 }),
        ] {
            assert!(s.dispatch("practice.metronome.set", &bad).is_err(), "{bad}");
        }
        let r = s
            .dispatch("practice.metronome.toggle", &json!({ "on": false }))
            .unwrap();
        assert!(r["beat"].is_null());
    }
}
