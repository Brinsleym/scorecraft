//! Stamps (feature-map ANNO-9, ScoreCraft's own; spec `docs/spec/ink.md`): dynamics pp to ff set
//! in Edwin Bold Italic, and crescendo and diminuendo hairpins with a handle at each end and one
//! in the middle. Placing and every handle drag is one undo step.

use serde_json::{Map, Value, json};

use scorecraft_doc::{Annotation, AnnotationId, AnnotationKind, Hairpin, Stamp};
use scorecraft_geom::{Point, Rect, Vec2};
use scorecraft_input::PointerKind;
use scorecraft_ops::{Op, OpError, Transaction};
use scorecraft_text::{Face, FaceKind, fit_inside};

use crate::annotate_cmds::{markup_ready, opt_color, state_json, writable};
use crate::commands::{
    CommandSpec, always, bad, opt_f64, opt_str, params_object, reject_unknown_keys, req_id,
    truncate,
};
use crate::markup::{MarkupTool, STAMP_SIZES, STAMPS};
use crate::text_cmds::{library_score, locate, page_key, page_rect, required_f64};
use crate::{EngineError, Session};

/// A tapped hairpin's length, in points.
const TAP_HAIRPIN: f64 = 72.0;

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "annotate.tool.stamp",
            label: "Stamp",
            menu: "Markup>Tools",
            shortcut: None,
            params: r#"{"stamp":"pp|p|mp|mf|f|ff|crescendo|diminuendo"?,"size":8..96?}"#,
            ours: true,
            enabled: always,
            run: tool_stamp,
        },
        CommandSpec {
            id: "annotate.stamp.place",
            label: "Place Stamp",
            menu: "",
            shortcut: None,
            params: r#"{"x":number,"y":number,"toX":number?,"toY":number?,"space":"view|page"="view","page":int?,"device":"pen|mouse|touch"="pen"}"#,
            ours: false,
            enabled: markup_ready,
            run: stamp_place,
        },
        CommandSpec {
            id: "annotate.stamp.edit",
            label: "Edit Stamp",
            menu: "",
            shortcut: None,
            params: r##"{"annotation":id,"handle":"start|end|middle"="middle","x":number?,"y":number?,"space":"view|page"="view","size":8..96?,"color":"#rrggbb"?}"##,
            ours: true,
            enabled: markup_ready,
            run: stamp_edit,
        },
    ]
}

fn stamp_size(id: &str, p: Option<&Map<String, Value>>) -> Result<Option<f32>, EngineError> {
    match opt_f64(id, p, "size")? {
        None => Ok(None),
        Some(s) if STAMP_SIZES.contains(s as f32) => Ok(Some(s as f32)),
        Some(_) => Err(bad(
            id,
            format!(
                "`size` must be from {} to {} points",
                STAMP_SIZES.min, STAMP_SIZES.max
            ),
        )),
    }
}

/// Chooses the stamp tool, optionally with a stamp and a size.
fn tool_stamp(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.tool.stamp";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["stamp", "size"])?;
    let choice = match opt_str(ID, p, "stamp")? {
        None => None,
        Some(s) => Some(STAMPS.iter().copied().find(|c| *c == s).ok_or_else(|| {
            bad(
                ID,
                format!(
                    "`stamp` must be one of {}, got {}",
                    STAMPS.join(", "),
                    truncate(s, 32)
                ),
            )
        })?),
    };
    let size = stamp_size(ID, p)?;
    let m = &mut session.markup;
    m.tool = MarkupTool::Stamp;
    if let Some(c) = choice {
        m.stamp = c;
    }
    if let Some(s) = size {
        m.stamp_size = s;
    }
    Ok(state_json(session))
}

/// The box a dynamic of `text` at `size` takes, centred on `c`.
pub(crate) fn dynamic_rect(text: &str, size: f64, c: Point) -> Result<Rect, EngineError> {
    let face = Face::get(FaceKind::Dynamic, false, false)
        .ok_or_else(|| EngineError::Unsupported("the built-in text font".into()))?;
    let half_w = face.advance(text, size) / 2.0 + size * 0.1;
    let half_h = size / 2.0;
    Ok(Rect::new(
        c.x - half_w,
        c.y - half_h,
        c.x + half_w,
        c.y + half_h,
    ))
}

/// A hairpin's opening and line width for a size setting: a third of the size, and a line that
/// thickens a little with it.
fn hairpin_shape(size: f64) -> (f32, f32) {
    ((size / 3.0) as f32, (size * 0.06).max(0.5) as f32)
}

fn clamp_point(p: Point, page: Rect) -> Point {
    Point::new(p.x.clamp(page.x0, page.x1), p.y.clamp(page.y0, page.y1))
}

/// Places the selected stamp: a dynamic centred on the tap, or a hairpin from (`x`, `y`) to
/// (`toX`, `toY`) (a tap makes one 72 pt long, opening to the right).
fn stamp_place(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.stamp.place";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["x", "y", "toX", "toY", "space", "page", "device"])?;
    let (x, y) = (required_f64(ID, p, "x")?, required_f64(ID, p, "y")?);
    let device_id = opt_str(ID, p, "device")?.unwrap_or("pen");
    let device = PointerKind::from_id(device_id)
        .filter(|d| *d != PointerKind::Eraser)
        .ok_or_else(|| bad(ID, "`device` must be pen, mouse or touch".into()))?;
    let m = &session.markup;
    if device == PointerKind::Touch && m.mode && m.touch_lock {
        return Ok(json!({ "added": false, "reason": "touchLock" }));
    }
    if !device.is_pen() && m.pen_only {
        return Ok(json!({ "added": false, "reason": "penOnly" }));
    }
    let (choice, size, color) = (m.stamp, f64::from(m.stamp_size), m.pen.color);
    let (page, file_page, at) = locate(session, ID, p, x, y)?;
    let bounds = page_rect(session, file_page)?;
    let kind = match choice {
        "crescendo" | "diminuendo" => {
            let to = match (opt_f64(ID, p, "toX")?, opt_f64(ID, p, "toY")?) {
                (Some(tx), Some(ty)) => {
                    let (_, other, to) = locate(session, ID, p, tx, ty)?;
                    if other != file_page {
                        return Err(bad(ID, "a hairpin stays on one page".into()));
                    }
                    to
                }
                (None, None) => at + Vec2::new(TAP_HAIRPIN, 0.0),
                _ => return Err(bad(ID, "give both `toX` and `toY`".into())),
            };
            let (from, to) = (clamp_point(at, bounds), clamp_point(to, bounds));
            if from.distance(to) < 1.0 {
                return Err(bad(ID, "a hairpin needs some length".into()));
            }
            let (opening, width) = hairpin_shape(size);
            AnnotationKind::Hairpin(Hairpin {
                from,
                to,
                crescendo: choice == "crescendo",
                opening,
                width,
                color,
            })
        }
        text => AnnotationKind::Stamp(Stamp {
            glyph: 0,
            text: text.to_owned(),
            size: size as f32,
            rect: fit_inside(dynamic_rect(text, size, at)?, bounds),
            color,
        }),
    };
    let score_id = session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let key = page_key(file_page)?;
    let (layer, index) = {
        let s = library_score(session, score_id)?;
        let layer = session
            .markup
            .layer(s)
            .ok_or_else(|| OpError::Missing("layer".into()))?;
        writable(s, layer)?;
        (layer, s.annotations_on(key).len())
    };
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let id: AnnotationId = l.ids.id(now);
    l.hlc = l.hlc.tick(now);
    l.commit(
        "Place Stamp",
        Transaction::single(Op::InsertAnnotation {
            score: score_id,
            page: key,
            index,
            annotation: Box::new(Annotation {
                id,
                layer,
                stamp: l.hlc,
                kind,
            }),
        }),
    )?;
    Ok(json!({
        "added": true,
        "annotation": id.to_string(),
        "stamp": choice,
        "page": page + 1,
        "filePage": file_page + 1,
    }))
}

/// Moves a stamp by a handle, resizes or recolours it: a dynamic by its middle; a hairpin's
/// `start` or `end` stretches it, its `middle` moves it whole.
fn stamp_edit(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.stamp.edit";
    let p = params_object(ID, params)?;
    reject_unknown_keys(
        ID,
        p,
        &["annotation", "handle", "x", "y", "space", "size", "color"],
    )?;
    let id: AnnotationId = req_id(ID, p, "annotation")?;
    let handle = opt_str(ID, p, "handle")?.unwrap_or("middle");
    if !matches!(handle, "start" | "end" | "middle") {
        return Err(bad(
            ID,
            format!(
                "`handle` must be start, end or middle, got {}",
                truncate(handle, 32)
            ),
        ));
    }
    let size = stamp_size(ID, p)?;
    let color = opt_color(ID, p, "color")?;
    let score_id = session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let (page, mut annotation) = {
        let s = library_score(session, score_id)?;
        let found = s
            .annotations
            .iter()
            .find_map(|(pg, list)| list.iter().find(|a| a.id == id).map(|a| (*pg, a.clone())))
            .ok_or_else(|| OpError::Missing(format!("annotation {id}")))?;
        writable(s, found.1.layer)?;
        found
    };
    let file_page = usize::try_from(page.0).map_err(|_| bad(ID, "page out of range".into()))?;
    let bounds = page_rect(session, file_page)?;
    // Where the handle was dragged to, on the stamp's own page.
    let to = match (opt_f64(ID, p, "x")?, opt_f64(ID, p, "y")?) {
        (Some(x), Some(y)) => {
            if opt_str(ID, p, "space")? == Some("page") {
                if !(x.is_finite() && y.is_finite() && x.abs() <= 1.0e6 && y.abs() <= 1.0e6) {
                    return Err(bad(ID, "the point is too far outside the page".into()));
                }
                Some(Point::new(x, y))
            } else {
                let (_, other, at) = locate(session, ID, p, x, y)?;
                if other != file_page {
                    return Err(bad(ID, "a stamp stays on its page".into()));
                }
                Some(at)
            }
        }
        (None, None) => None,
        _ => return Err(bad(ID, "give both `x` and `y`".into())),
    };
    if to.is_none() && size.is_none() && color.is_none() {
        return Err(bad(ID, "give a handle position, `size` or `color`".into()));
    }
    match &mut annotation.kind {
        AnnotationKind::Stamp(st) => {
            if handle != "middle" {
                return Err(bad(ID, "a dynamic has only its middle handle".into()));
            }
            let size = size.map_or(f64::from(st.size), f64::from);
            let centre = to.unwrap_or(st.rect.center());
            st.size = size as f32;
            st.rect = fit_inside(dynamic_rect(&st.text, size, centre)?, bounds);
            if let Some(c) = color {
                st.color = c;
            }
        }
        AnnotationKind::Hairpin(h) => {
            if let Some(to) = to.map(|q| clamp_point(q, bounds)) {
                match handle {
                    "start" => h.from = to,
                    "end" => h.to = to,
                    _ => {
                        let mid = h.from.lerp(h.to, 0.5);
                        let d = to - mid;
                        h.from = clamp_point(h.from + d, bounds);
                        h.to = clamp_point(h.to + d, bounds);
                    }
                }
                if h.from.distance(h.to) < 1.0 {
                    return Err(bad(ID, "a hairpin needs some length".into()));
                }
            }
            if let Some(s) = size {
                (h.opening, h.width) = hairpin_shape(f64::from(s));
            }
            if let Some(c) = color {
                h.color = c;
            }
        }
        _ => return Err(bad(ID, "that marking is not a stamp".into())),
    }
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    l.hlc = l.hlc.tick(now);
    annotation.stamp = l.hlc;
    let out = serde_json::to_value(&annotation)
        .map_err(|e| EngineError::Io(format!("describing the stamp: {e}")))?;
    l.commit(
        "Edit Stamp",
        Transaction::single(Op::SetAnnotation {
            score: score_id,
            page,
            annotation: Box::new(annotation),
        }),
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests;
