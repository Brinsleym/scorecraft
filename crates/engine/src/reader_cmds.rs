//! Reader commands: layouts, turns, go to page, zoom and pan (feature-map READ-2, READ-3,
//! READ-5, READ-6, READ-12, READ-15, TURN-1..3, TURN-8, NAV-5; spec `docs/spec/page-turning.md`).
//! Every command returns the reader state (`view.state`) so an agent sees what is on screen.

use serde_json::{Map, Value, json};

use scorecraft_geom::Size;
use scorecraft_layout::{Fit, Layout, TurnMode};

use crate::commands::{
    CommandSpec, bad, opt_bool, opt_f64, opt_int, opt_str, params_object, reject_unknown_keys,
};
use crate::reader::{MAX_PIXELS_PER_POINT, MAX_VIEWPORT, ReaderState};
use crate::score_cmds::score_open;
use crate::{EngineError, Session};

/// Deepest outline path accepted by `view.outline.goto`.
const MAX_OUTLINE_DEPTH: usize = 64;

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, shortcut, params, ours, run| CommandSpec {
        id,
        label,
        menu,
        shortcut,
        params,
        ours,
        enabled: score_open,
        run,
    };
    vec![
        spec(
            "view.turn.next",
            "Next Page",
            "View>Turn",
            Some("Right"),
            "{}",
            false,
            turn_next as _,
        ),
        spec(
            "view.turn.previous",
            "Previous Page",
            "View>Turn",
            Some("Left"),
            "{}",
            false,
            turn_previous as _,
        ),
        spec(
            "view.page.goto",
            "Go to Page…",
            "View>Go To",
            Some("Ctrl+G"),
            r#"{"page":int?,"to":"first|last"?}"#,
            false,
            page_goto as _,
        ),
        spec(
            "view.layout.single",
            "Single Page",
            "View>Layout",
            None,
            "{}",
            false,
            layout_single as _,
        ),
        spec(
            "view.layout.twoPage",
            "Two Pages",
            "View>Layout",
            None,
            r#"{"firstAlone":bool=false}"#,
            false,
            layout_two_page as _,
        ),
        spec(
            "view.turn.halfPage",
            "Half-Page Turns",
            "View>Turn",
            None,
            r#"{"on":bool=true,"split":number?,"snap":bool=false}"#,
            false,
            half_page as _,
        ),
        spec(
            "view.turn.alternating",
            "Alternating Turns",
            "View>Turn",
            None,
            r#"{"on":bool=true}"#,
            false,
            alternating as _,
        ),
        spec(
            "view.zoom.set",
            "Zoom",
            "View>Zoom",
            None,
            r#"{"zoom":number?,"fit":"page|width"?}"#,
            false,
            zoom_set as _,
        ),
        spec(
            "view.zoom.landscapeFit",
            "Landscape Page Zoom",
            "View>Zoom",
            None,
            r#"{"on":bool=true}"#,
            false,
            landscape_fit as _,
        ),
        spec(
            "view.pan",
            "Pan",
            "",
            None,
            r#"{"dx":number=0,"dy":number=0}"#,
            false,
            pan as _,
        ),
        spec(
            "view.outline.goto",
            "Go to Contents Entry",
            "View>Go To",
            None,
            r#"{"path":[int]}"#,
            false,
            outline_goto as _,
        ),
        spec(
            "view.viewport.set",
            "Set Reader Area",
            "",
            None,
            r#"{"width":number,"height":number,"pixelsPerPoint":number=1}"#,
            true,
            viewport_set as _,
        ),
        spec(
            "view.state",
            "Reader State",
            "",
            None,
            "{}",
            true,
            state as _,
        ),
    ]
}

type Params<'a> = Option<&'a Map<String, Value>>;

/// Validates params, applies `change` to the reader, pre-renders what comes next and returns the
/// state with `extra` merged in.
fn with_reader(
    session: &mut Session,
    id: &str,
    params: &Value,
    keys: &[&str],
    change: impl FnOnce(&mut ReaderState, Params<'_>) -> Result<Value, EngineError>,
) -> Result<Value, EngineError> {
    let p = params_object(id, params)?;
    reject_unknown_keys(id, p, keys)?;
    let r = session.reader_mut(id)?;
    let extra = change(r, p)?;
    let mut out = r.to_json();
    if let (Value::Object(out), Value::Object(extra)) = (&mut out, extra) {
        out.extend(extra);
    }
    session.prefetch_reader();
    Ok(out)
}

/// A turn always shows the top of the new view.
fn after_turn(r: &mut ReaderState) {
    r.zoom.pan.1 = 0.0;
}

fn turn_next(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let v = with_reader(s, "view.turn.next", params, &[], |r, _| {
        let turned = r.reader.turn_next();
        if turned {
            after_turn(r);
        }
        Ok(json!({ "turned": turned }))
    })?;
    // Past the last page of a setlist entry: the next entry (LIST-4).
    if v.get("turned") == Some(&json!(false))
        && let Some(next) = crate::play_cmds::continue_turn(s, crate::play_cmds::Way::Forward)?
    {
        return Ok(next);
    }
    Ok(v)
}

fn turn_previous(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    let v = with_reader(s, "view.turn.previous", params, &[], |r, _| {
        let turned = r.reader.turn_previous();
        if turned {
            after_turn(r);
        }
        Ok(json!({ "turned": turned }))
    })?;
    if v.get("turned") == Some(&json!(false))
        && let Some(prev) = crate::play_cmds::continue_turn(s, crate::play_cmds::Way::Back)?
    {
        return Ok(prev);
    }
    Ok(v)
}

fn page_goto(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.page.goto";
    with_reader(s, ID, params, &["page", "to"], |r, p| {
        let count = r.shown.len();
        let page = match (
            opt_int(ID, p, "page", 1, count.max(1) as u64)?,
            opt_str(ID, p, "to")?,
        ) {
            (Some(n), None) => usize::try_from(n - 1).unwrap_or(0),
            (None, Some("first")) => 0,
            (None, Some("last")) => count.saturating_sub(1),
            (None, Some(_)) => return Err(bad(ID, "`to` must be first or last".into())),
            (Some(_), Some(_)) => return Err(bad(ID, "give `page` or `to`, not both".into())),
            (None, None) => return Err(bad(ID, "`page` or `to` is required".into())),
        };
        r.reader.goto(page);
        after_turn(r);
        Ok(json!({}))
    })
}

fn layout_single(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    with_reader(s, "view.layout.single", params, &[], |r, _| {
        r.reader.set_layout(Layout::Single);
        Ok(json!({}))
    })
}

fn layout_two_page(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.layout.twoPage";
    with_reader(s, ID, params, &["firstAlone"], |r, p| {
        let first_alone = opt_bool(ID, p, "firstAlone")?.unwrap_or(false);
        r.reader.set_layout(Layout::TwoPage { first_alone });
        Ok(json!({}))
    })
}

/// Turns a mode on, or back to full turns if it is the mode in use.
fn set_mode(r: &mut ReaderState, mode: TurnMode, on: bool) {
    if on {
        r.reader.set_mode(mode);
    } else if r.reader.mode() == mode {
        r.reader.set_mode(TurnMode::Full);
    }
}

fn half_page(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.turn.halfPage";
    // With `snap`, the split moves to the gap between systems on the page on screen (G2).
    let po = params_object(ID, params)?;
    let snapped = if opt_bool(ID, po, "snap")?.unwrap_or(false) {
        let wanted = match opt_f64(ID, po, "split")? {
            Some(x) => Some(x),
            None => s
                .reader_state()
                .and_then(|st| st.get("split").and_then(Value::as_f64)),
        };
        wanted.map(|w| snap_on_screen(s, w))
    } else {
        None
    };
    with_reader(s, ID, params, &["on", "split", "snap"], |r, p| {
        let on = opt_bool(ID, p, "on")?.unwrap_or(true);
        if let Some(split) = snapped.or(opt_f64(ID, p, "split")?) {
            let (lo, hi) = scorecraft_layout::SPLIT_RANGE;
            if !(lo..=hi).contains(&split) {
                return Err(bad(ID, format!("`split` must be from {lo} to {hi}")));
            }
            r.reader.set_split(split);
        }
        set_mode(r, TurnMode::HalfPage, on);
        Ok(json!({}))
    })
}

/// `split` snapped to the gap between systems on the page on screen, from a small render of it;
/// unchanged when the page is turned sideways by the score or can't be rendered.
fn snap_on_screen(s: &mut Session, split: f64) -> f64 {
    let Some(open) = s.score.as_mut() else {
        return split;
    };
    let page = open.reader.reader.page();
    let Some(shown) = open.reader.shown.get(page).copied() else {
        return split;
    };
    if shown.rotation != scorecraft_geom::Rotation::R0 {
        return split;
    }
    let Ok(bm) = open.view.page(shown.file_page, 0.5) else {
        return split;
    };
    let (w, h) = (bm.width as usize, bm.height as usize);
    if w == 0 {
        return split;
    }
    let rows: Vec<f32> = (0..h)
        .map(|y| {
            let row = bm.words.get(y * w..(y + 1) * w).unwrap_or(&[]);
            let dark = row
                .iter()
                .filter(|px| {
                    let [r, g, b, _] = px.to_ne_bytes();
                    u32::from(r) + u32::from(g) + u32::from(b) < 3 * 140
                })
                .count();
            dark as f32 / w as f32
        })
        .collect();
    scorecraft_layout::snap_split(split, &rows)
}

fn alternating(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.turn.alternating";
    with_reader(s, ID, params, &["on"], |r, p| {
        let on = opt_bool(ID, p, "on")?.unwrap_or(true);
        set_mode(r, TurnMode::Alternating, on);
        Ok(json!({}))
    })
}

fn zoom_set(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.zoom.set";
    with_reader(s, ID, params, &["zoom", "fit"], |r, p| {
        if let Some(fit) = opt_str(ID, p, "fit")? {
            r.zoom.fit = match fit {
                "page" => Fit::Page,
                "width" => Fit::Width,
                _ => return Err(bad(ID, "`fit` must be page or width".into())),
            };
        }
        if let Some(z) = opt_f64(ID, p, "zoom")? {
            let (lo, hi) = scorecraft_layout::ZOOM_RANGE;
            if !(lo..=hi).contains(&z) {
                return Err(bad(ID, format!("`zoom` must be from {lo} to {hi}")));
            }
            r.zoom.factor = z;
        }
        Ok(json!({}))
    })
}

fn landscape_fit(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.zoom.landscapeFit";
    with_reader(s, ID, params, &["on"], |r, p| {
        let on = opt_bool(ID, p, "on")?.unwrap_or(true);
        r.zoom.fit = if on { Fit::Width } else { Fit::Page };
        r.zoom.pan.1 = 0.0;
        Ok(json!({}))
    })
}

fn pan(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.pan";
    with_reader(s, ID, params, &["dx", "dy"], |r, p| {
        let dx = opt_f64(ID, p, "dx")?.unwrap_or(0.0);
        let dy = opt_f64(ID, p, "dy")?.unwrap_or(0.0);
        if !(dx.abs() <= MAX_VIEWPORT * 8.0 && dy.abs() <= MAX_VIEWPORT * 8.0) {
            return Err(bad(
                ID,
                "`dx` and `dy` must be finite and reasonable".into(),
            ));
        }
        let overflow = r.frame().overflow;
        r.zoom.pan_by(dx, dy, overflow);
        Ok(json!({}))
    })
}

fn outline_goto(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.outline.goto";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path"])?;
    let path =
        crate::commands::opt_int_list(ID, p, "path", (0, u64::from(u32::MAX)), MAX_OUTLINE_DEPTH)?
            .ok_or_else(|| bad(ID, "`path` is required".into()))?;
    let open = s
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let mut entries = open.view.document().outline();
    let mut found = None;
    for &i in &path {
        let e = usize::try_from(i)
            .ok()
            .and_then(|i| entries.get(i))
            .ok_or_else(|| bad(ID, "`path` names no contents entry".into()))?;
        found = Some(e);
        entries = &e.children;
    }
    let entry = found.ok_or_else(|| bad(ID, "`path` must not be empty".into()))?;
    let title = entry.title.clone();
    let file_page = entry
        .page
        .ok_or_else(|| bad(ID, format!("`{title}` doesn't go to a page of this score")))?;
    with_reader(s, ID, params, &["path"], |r, _| {
        let page = r
            .shown
            .iter()
            .position(|p| p.file_page == file_page)
            .ok_or_else(|| bad(ID, format!("`{title}` goes to a hidden page")))?;
        r.reader.goto(page);
        after_turn(r);
        Ok(json!({ "entry": title }))
    })
}

fn viewport_set(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "view.viewport.set";
    with_reader(
        s,
        ID,
        params,
        &["width", "height", "pixelsPerPoint"],
        |r, p| {
            let side = |key: &str| -> Result<f64, EngineError> {
                let v =
                    opt_f64(ID, p, key)?.ok_or_else(|| bad(ID, format!("`{key}` is required")))?;
                if v > 0.0 && v <= MAX_VIEWPORT {
                    Ok(v)
                } else {
                    Err(bad(
                        ID,
                        format!("`{key}` must be above 0 and at most {MAX_VIEWPORT}"),
                    ))
                }
            };
            let size = Size {
                width: side("width")?,
                height: side("height")?,
            };
            let ppp = opt_f64(ID, p, "pixelsPerPoint")?.unwrap_or(1.0);
            if !(ppp > 0.0 && ppp <= MAX_PIXELS_PER_POINT) {
                return Err(bad(
                    ID,
                    format!("`pixelsPerPoint` must be above 0 and at most {MAX_PIXELS_PER_POINT}"),
                ));
            }
            r.viewport = size;
            r.pixels_per_point = ppp;
            Ok(json!({}))
        },
    )
}

fn state(s: &mut Session, params: &Value) -> Result<Value, EngineError> {
    with_reader(s, "view.state", params, &[], |_, _| Ok(json!({})))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

    fn session(pages: usize) -> Session {
        let mut s = Session::new();
        let bytes = PdfBuilder::new().pages(pages, PageSpec::default()).build();
        s.open_score_bytes(Arc::new(bytes), None, "test.pdf")
            .unwrap();
        s
    }

    fn run(s: &mut Session, id: &str, p: Value) -> Value {
        s.dispatch(id, &p)
            .unwrap_or_else(|e| panic!("{id} {p}: {e}"))
    }

    #[test]
    fn perf_15_the_half_page_split_snaps_between_systems() {
        let mut s = Session::new();
        let bytes = PdfBuilder::new()
            .pages(2, PageSpec::default().with_staves(4))
            .build();
        s.open_score_bytes(Arc::new(bytes), None, "four.pdf")
            .unwrap();
        let plain = run(&mut s, "view.turn.halfPage", json!({ "split": 0.42 }));
        assert_eq!(plain["split"], 0.42, "without snap, as asked");
        let snapped = run(
            &mut s,
            "view.turn.halfPage",
            json!({ "split": 0.42, "snap": true }),
        );
        let x = snapped["split"].as_f64().unwrap();
        assert!(
            x != 0.42 && (x - 0.42).abs() <= scorecraft_layout::MAX_SNAP,
            "{x}"
        );
        // Asking again from the snapped place keeps it.
        let again = run(&mut s, "view.turn.halfPage", json!({ "snap": true }));
        assert!((again["split"].as_f64().unwrap() - x).abs() < 1e-9);
    }

    #[test]
    fn disabled_without_a_score() {
        let mut s = Session::new();
        for c in specs() {
            assert_eq!(s.dispatch(c.id, &json!({})).unwrap_err().code(), "disabled");
        }
    }

    #[test]
    fn turns_forward_and_back_and_stops_at_the_ends() {
        let mut s = session(3);
        let r = run(&mut s, "view.turn.previous", json!({}));
        assert_eq!(
            (r["turned"].as_bool(), r["page"].as_u64()),
            (Some(false), Some(1))
        );
        let r = run(&mut s, "view.turn.next", json!({}));
        assert_eq!(
            (r["turned"].as_bool(), r["page"].as_u64()),
            (Some(true), Some(2))
        );
        run(&mut s, "view.turn.next", json!({}));
        let r = run(&mut s, "view.turn.next", json!({}));
        assert_eq!(
            (r["turned"].as_bool(), r["page"].as_u64()),
            (Some(false), Some(3))
        );
    }

    #[test]
    fn two_page_and_half_page_views() {
        let mut s = session(5);
        let r = run(&mut s, "view.layout.twoPage", json!({}));
        assert_eq!(r["onScreen"], json!([1, 2]));
        assert_eq!(r["slots"].as_array().unwrap().len(), 2);
        let r = run(&mut s, "view.layout.twoPage", json!({ "firstAlone": true }));
        assert_eq!(
            (r["onScreen"].clone(), r["firstAlone"].clone()),
            (json!([1]), json!(true))
        );
        run(&mut s, "view.layout.single", json!({}));
        run(&mut s, "view.turn.halfPage", json!({ "split": 0.4 }));
        let r = run(&mut s, "view.turn.next", json!({}));
        assert_eq!(
            (r["half"].as_bool(), r["page"].as_u64()),
            (Some(true), Some(1))
        );
        assert_eq!(r["onScreen"], json!([1, 2]));
        assert_eq!(r["slots"][0]["src"], json!([0.0, 0.0, 1.0, 0.4]));
        let r = run(&mut s, "view.turn.halfPage", json!({ "on": false }));
        assert_eq!(
            (r["mode"].as_str(), r["half"].as_bool()),
            (Some("full"), Some(false))
        );
    }

    #[test]
    fn alternating_turns() {
        let mut s = session(4);
        run(&mut s, "view.layout.twoPage", json!({}));
        run(&mut s, "view.turn.alternating", json!({}));
        let r = run(&mut s, "view.turn.next", json!({}));
        assert_eq!(r["onScreen"], json!([2, 3]));
        assert_eq!(r["slots"][0]["page"], 3);
    }

    #[test]
    fn goto_and_zoom_and_pan() {
        let mut s = session(9);
        let r = run(&mut s, "view.page.goto", json!({ "page": 7 }));
        assert_eq!(r["page"], 7);
        let r = run(&mut s, "view.page.goto", json!({ "to": "last" }));
        assert_eq!(r["page"], 9);
        let r = run(&mut s, "view.page.goto", json!({ "to": "first" }));
        assert_eq!(r["page"], 1);
        run(
            &mut s,
            "view.viewport.set",
            json!({ "width": 800, "height": 600, "pixelsPerPoint": 2 }),
        );
        let r = run(&mut s, "view.zoom.set", json!({ "zoom": 2 }));
        assert!(r["overflow"][1].as_f64().unwrap() > 0.0);
        let r = run(&mut s, "view.pan", json!({ "dy": -100000 }));
        assert_eq!(r["zoom"]["pan"][1], 1.0);
        let r = run(&mut s, "view.turn.next", json!({}));
        assert_eq!(r["zoom"]["pan"][1], 0.0);
        let r = run(&mut s, "view.zoom.landscapeFit", json!({}));
        assert_eq!(r["zoom"]["fit"], "width");
    }

    #[test]
    fn frames_carry_bitmaps_and_turns_hit_the_prefetch() {
        let mut s = session(3);
        run(
            &mut s,
            "view.viewport.set",
            json!({ "width": 300, "height": 400 }),
        );
        let f = s.reader_frame(true).unwrap();
        assert_eq!(f.len(), 1);
        assert!(f[0].bitmap.is_some());
        run(&mut s, "view.turn.next", json!({}));
        s.score
            .as_ref()
            .unwrap()
            .view
            .wait_for_prefetch(std::time::Duration::from_secs(20));
        let f = s.reader_frame(false).unwrap();
        assert_eq!(f[0].file_page, 1);
        assert!(f[0].bitmap.is_some(), "the next page was pre-rendered");
    }

    #[test]
    fn bad_params_are_errors() {
        let mut s = session(2);
        for (id, p) in [
            ("view.turn.next", json!({ "half": true })),
            ("view.page.goto", json!({})),
            ("view.page.goto", json!({ "page": 3 })),
            ("view.page.goto", json!({ "page": 0 })),
            ("view.page.goto", json!({ "to": "middle" })),
            ("view.page.goto", json!({ "page": 1, "to": "first" })),
            ("view.layout.twoPage", json!({ "firstAlone": 1 })),
            ("view.turn.halfPage", json!({ "split": 0.95 })),
            ("view.turn.halfPage", json!({ "split": "half" })),
            ("view.zoom.set", json!({ "zoom": 0.5 })),
            ("view.zoom.set", json!({ "zoom": 100 })),
            ("view.zoom.set", json!({ "fit": "height" })),
            ("view.pan", json!({ "dx": 1e300 })),
            ("view.outline.goto", json!({})),
            ("view.outline.goto", json!({ "path": [0] })),
            ("view.outline.goto", json!({ "path": [] })),
            ("view.viewport.set", json!({ "width": 0, "height": 10 })),
            ("view.viewport.set", json!({ "width": 10 })),
            (
                "view.viewport.set",
                json!({ "width": 10, "height": 10, "pixelsPerPoint": 99 }),
            ),
        ] {
            let e = s.dispatch(id, &p).unwrap_err();
            assert_eq!(e.code(), "bad_params", "{id} {p}");
        }
    }
}
