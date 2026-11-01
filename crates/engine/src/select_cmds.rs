//! Rectangle select (feature-map ANNO-12, spec `docs/spec/ink.md`): drag out a rectangle on one
//! page, then move, copy, delete, scale or recolour the markings mostly inside it. Each action is
//! one undo step. Snips (PAGE-7) and shortcuts (NAV-1) from the rectangle come with setlists (M6).

use serde_json::{Map, Value, json};

use scorecraft_doc::{Annotation, AnnotationKind, Color, Layer, PageKey, ScoreId, Tool};
use scorecraft_geom::{Point, Rect, Vec2};
use scorecraft_ink::{MAX_COORD, ToolStyle};
use scorecraft_ops::{Op, Transaction};
use scorecraft_text::fit_inside;

use crate::annotate_cmds::{markup_ready, opt_color, state_json, writable};
use crate::commands::{CommandSpec, always, bad, opt_f64, params_object, reject_unknown_keys};
use crate::markup::{MarkupTool, STAMP_SIZES, Selection, TEXT_SIZES};
use crate::stamp_cmds::dynamic_rect;
use crate::text_cmds::{library_score, locate, page_key, page_rect, place, required_f64};
use crate::{EngineError, Session};

/// The scale range (spec ANNO-12).
pub const MIN_SCALE: f64 = 0.1;
pub const MAX_SCALE: f64 = 10.0;

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, ours, enabled, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours,
        enabled,
        run,
    };
    vec![
        spec(
            "annotate.tool.select",
            "Rectangle Select",
            "Markup>Tools",
            "{}",
            false,
            always,
            tool_select as _,
        ),
        spec(
            "annotate.select.rect",
            "Select",
            "",
            r#"{"x0":number,"y0":number,"x1":number,"y1":number,"space":"view|page"="view","page":int?}"#,
            false,
            markup_ready,
            select_rect as _,
        ),
        spec(
            "annotate.select.clear",
            "Deselect",
            "",
            "{}",
            false,
            always,
            select_clear as _,
        ),
        spec(
            "annotate.select.delete",
            "Delete Selection",
            "",
            "{}",
            false,
            markup_ready,
            select_delete as _,
        ),
        spec(
            "annotate.select.move",
            "Move Selection",
            "",
            r#"{"x":number,"y":number,"space":"view|page"="view","page":int?}"#,
            false,
            markup_ready,
            select_move as _,
        ),
        spec(
            "annotate.select.copy",
            "Copy Selection",
            "",
            r#"{"x":number,"y":number,"space":"view|page"="view","page":int?}"#,
            false,
            markup_ready,
            select_copy as _,
        ),
        spec(
            "annotate.select.scale",
            "Scale Selection",
            "",
            r#"{"factor":0.1..10}"#,
            true,
            markup_ready,
            select_scale as _,
        ),
        spec(
            "annotate.select.recolor",
            "Recolour Selection",
            "",
            r##"{"color":"#rrggbb"}"##,
            true,
            markup_ready,
            select_recolor as _,
        ),
    ]
}

fn tool_select(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.tool.select";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    session.markup.tool = MarkupTool::Select;
    Ok(state_json(session))
}

fn holds(r: Rect, p: Point) -> bool {
    p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1
}

/// Whether a marking lies mostly inside `r`: more than half a stroke's samples, or the centre of
/// a note's, stamp's or hairpin's box. Snips and shortcuts are never selected here.
pub(crate) fn mostly_inside(kind: &AnnotationKind, r: Rect) -> bool {
    match kind {
        AnnotationKind::Stroke(s) => {
            let inside = s
                .points
                .iter()
                .filter(|q| holds(r, Point::new(f64::from(q.x), f64::from(q.y))))
                .count();
            inside * 2 > s.points.len()
        }
        AnnotationKind::Text(t) => holds(r, t.rect.center()),
        AnnotationKind::Stamp(st) => holds(r, st.rect.center()),
        AnnotationKind::Hairpin(h) => holds(r, h.from.lerp(h.to, 0.5)),
        AnnotationKind::Snip(_) | AnnotationKind::Shortcut(_) => false,
    }
}

fn open_score(session: &Session, id: &str) -> Result<ScoreId, EngineError> {
    session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(id.to_owned()))
}

fn selection_json(sel: &Selection, page: Option<usize>) -> Value {
    let count = sel.ids.len();
    json!({
        "count": count,
        "annotations": sel.ids.iter().map(ToString::to_string).collect::<Vec<_>>(),
        "page": page.map(|p| p + 1),
        "filePage": u64::from(sel.page.0) + 1,
        "rect": [sel.rect.x0, sel.rect.y0, sel.rect.x1, sel.rect.y1],
        // An empty selection offers only snip and shortcut, which come with setlists (M6).
        "actions": if count > 0 {
            vec!["move", "copy", "delete", "scale", "recolor"]
        } else {
            Vec::new()
        },
    })
}

/// Selects the markings mostly inside the rectangle from (`x0`, `y0`) to (`x1`, `y1`), on
/// visible, unlocked layers. Both corners must be on one page.
fn select_rect(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.select.rect";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["x0", "y0", "x1", "y1", "space", "page"])?;
    let (x0, y0) = (required_f64(ID, p, "x0")?, required_f64(ID, p, "y0")?);
    let (x1, y1) = (required_f64(ID, p, "x1")?, required_f64(ID, p, "y1")?);
    let score_id = open_score(session, ID)?;
    let (page, file_page, a) = locate(session, ID, p, x0, y0)?;
    let (_, other, b) = locate(session, ID, p, x1, y1)?;
    if other != file_page {
        return Err(bad(ID, "a selection stays on one page".into()));
    }
    let rect = Rect::new(a.x, a.y, b.x, b.y);
    let key = page_key(file_page)?;
    let s = library_score(session, score_id)?;
    let open = |a: &Annotation| {
        s.layer(a.layer)
            .is_some_and(|l: &Layer| l.visible && !l.locked)
    };
    let ids = s
        .annotations_on(key)
        .iter()
        .filter(|a| open(a) && mostly_inside(&a.kind, rect))
        .map(|a| a.id)
        .collect();
    let sel = Selection {
        score: score_id,
        page: key,
        rect,
        ids,
    };
    let out = selection_json(&sel, Some(page));
    session.markup.tool = MarkupTool::Select;
    session.markup.selection = Some(sel);
    Ok(out)
}

fn select_clear(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.select.clear";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    session.markup.selection = None;
    Ok(state_json(session))
}

/// The selection and the markings it still holds, in page order, each on a layer that can be
/// changed. Undo can take markings away under a selection, so missing ids are skipped; nothing
/// left is an error.
fn current(session: &Session, id: &str) -> Result<(Selection, Vec<Annotation>), EngineError> {
    let score_id = open_score(session, id)?;
    let sel = session
        .markup
        .selection
        .clone()
        .filter(|s| s.score == score_id)
        .ok_or_else(|| bad(id, "nothing is selected".into()))?;
    let s = library_score(session, score_id)?;
    let marks: Vec<Annotation> = s
        .annotations_on(sel.page)
        .iter()
        .filter(|a| sel.ids.contains(&a.id))
        .cloned()
        .collect();
    if marks.is_empty() {
        return Err(bad(id, "nothing is selected".into()));
    }
    for a in &marks {
        writable(s, a.layer)?;
    }
    Ok((sel, marks))
}

fn file_page_of(id: &str, key: PageKey) -> Result<usize, EngineError> {
    usize::try_from(key.0).map_err(|_| bad(id, "page out of range".into()))
}

/// Moves and scales one marking: `map` takes each point to its new place, `factor` scales sizes
/// and widths (kept in each tool's range), and notes and stamps are kept inside `bounds`.
fn transform(
    id: &str,
    kind: &mut AnnotationKind,
    map: &dyn Fn(Point) -> Point,
    factor: f64,
    bounds: Rect,
) -> Result<(), EngineError> {
    let max = f64::from(MAX_COORD);
    let far = |p: Point| !(p.is_finite() && p.x.abs() <= max && p.y.abs() <= max);
    match kind {
        AnnotationKind::Stroke(s) => {
            for q in &mut s.points {
                let to = map(Point::new(f64::from(q.x), f64::from(q.y)));
                if far(to) {
                    return Err(bad(
                        id,
                        "that would take the ink too far off the page".into(),
                    ));
                }
                (q.x, q.y) = (to.x as f32, to.y as f32);
            }
            let r = ToolStyle::widths(s.tool);
            s.width = ((f64::from(s.width) * factor) as f32).clamp(r.min, r.max);
        }
        AnnotationKind::Text(t) => {
            let top_left = map(Point::new(t.rect.x0, t.rect.y1));
            t.style.size =
                ((f64::from(t.style.size) * factor) as f32).clamp(TEXT_SIZES.min, TEXT_SIZES.max);
            t.rect = place(&t.text, &t.style, top_left, bounds)?;
        }
        AnnotationKind::Stamp(st) => {
            let centre = map(st.rect.center());
            if st.text.is_empty() || st.size <= 0.0 {
                let (hw, hh) = (
                    st.rect.width() * factor / 2.0,
                    st.rect.height() * factor / 2.0,
                );
                st.rect = fit_inside(
                    Rect::new(centre.x - hw, centre.y - hh, centre.x + hw, centre.y + hh),
                    bounds,
                );
            } else {
                st.size =
                    ((f64::from(st.size) * factor) as f32).clamp(STAMP_SIZES.min, STAMP_SIZES.max);
                st.rect = fit_inside(dynamic_rect(&st.text, f64::from(st.size), centre)?, bounds);
            }
        }
        AnnotationKind::Hairpin(h) => {
            let clamp = |p: Point| {
                Point::new(
                    p.x.clamp(bounds.x0, bounds.x1),
                    p.y.clamp(bounds.y0, bounds.y1),
                )
            };
            (h.from, h.to) = (clamp(map(h.from)), clamp(map(h.to)));
            if h.from.distance(h.to) < 1.0 {
                return Err(bad(id, "a hairpin needs some length".into()));
            }
            h.opening = ((f64::from(h.opening) * factor) as f32)
                .clamp(STAMP_SIZES.min / 3.0, STAMP_SIZES.max / 3.0);
            h.width = ((f64::from(h.width) * factor) as f32).clamp(0.5, 6.0);
        }
        AnnotationKind::Snip(_) | AnnotationKind::Shortcut(_) => {
            return Err(bad(id, "snips and shortcuts are not selectable".into()));
        }
    }
    Ok(())
}

/// Commits `tx` and makes the selection `sel`.
fn finish(
    session: &mut Session,
    id: &str,
    label: &str,
    tx: Transaction,
    sel: Selection,
    page: Option<usize>,
) -> Result<Value, EngineError> {
    session.lib_mut(id)?.commit(label, tx)?;
    let out = selection_json(&sel, page);
    session.markup.selection = Some(sel);
    Ok(out)
}

fn select_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.select.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    let (sel, marks) = current(session, ID)?;
    let ops = marks
        .iter()
        .map(|a| Op::RemoveAnnotation {
            score: sel.score,
            page: sel.page,
            annotation: a.id,
        })
        .collect();
    session
        .lib_mut(ID)?
        .commit("Delete Selection", Transaction::new(ops))?;
    session.markup.selection = None;
    Ok(json!({ "deleted": marks.len() }))
}

/// Moves (or copies) the selection so its rectangle's centre lands on the drop point, on the
/// same page or another. Copies get new ids; moved markings keep theirs.
fn drop_at(
    session: &mut Session,
    id: &str,
    params: &Value,
    copy: bool,
) -> Result<Value, EngineError> {
    let p = params_object(id, params)?;
    reject_unknown_keys(id, p, &["x", "y", "space", "page"])?;
    let (x, y) = (required_f64(id, p, "x")?, required_f64(id, p, "y")?);
    let (sel, marks) = current(session, id)?;
    let (page, file_page, at) = locate(session, id, p, x, y)?;
    let bounds = page_rect(session, file_page)?;
    let to = Point::new(
        at.x.clamp(bounds.x0, bounds.x1),
        at.y.clamp(bounds.y0, bounds.y1),
    );
    let d: Vec2 = to - sel.rect.center();
    let key = page_key(file_page)?;
    let same_page = key == sel.page;
    let mut changed = Vec::with_capacity(marks.len());
    for mut a in marks {
        transform(id, &mut a.kind, &|q| q + d, 1.0, bounds)?;
        changed.push(a);
    }
    let base = library_score(session, sel.score)?.annotations_on(key).len();
    let now = session.now_ms();
    let l = session.lib_mut(id)?;
    let mut ops = Vec::new();
    let mut ids = Vec::with_capacity(changed.len());
    for (k, mut a) in changed.into_iter().enumerate() {
        l.hlc = l.hlc.tick(now);
        a.stamp = l.hlc;
        if copy {
            a.id = l.ids.id(now);
        } else if same_page {
            ids.push(a.id);
            ops.push(Op::SetAnnotation {
                score: sel.score,
                page: key,
                annotation: Box::new(a),
            });
            continue;
        } else {
            ops.push(Op::RemoveAnnotation {
                score: sel.score,
                page: sel.page,
                annotation: a.id,
            });
        }
        ids.push(a.id);
        ops.push(Op::InsertAnnotation {
            score: sel.score,
            page: key,
            index: base + k,
            annotation: Box::new(a),
        });
    }
    let moved = Selection {
        score: sel.score,
        page: key,
        rect: Rect::new(
            sel.rect.x0 + d.x,
            sel.rect.y0 + d.y,
            sel.rect.x1 + d.x,
            sel.rect.y1 + d.y,
        ),
        ids,
    };
    let label = if copy {
        "Copy Selection"
    } else {
        "Move Selection"
    };
    finish(session, id, label, Transaction::new(ops), moved, Some(page))
}

fn select_move(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    drop_at(session, "annotate.select.move", params, false)
}

fn select_copy(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    drop_at(session, "annotate.select.copy", params, true)
}

/// Rewrites every selected marking with `edit`, in one undo step.
fn rewrite(
    session: &mut Session,
    id: &str,
    label: &str,
    sel: Selection,
    marks: Vec<Annotation>,
    edit: &dyn Fn(&mut AnnotationKind) -> Result<(), EngineError>,
) -> Result<Value, EngineError> {
    let mut changed = Vec::with_capacity(marks.len());
    for mut a in marks {
        edit(&mut a.kind)?;
        changed.push(a);
    }
    let now = session.now_ms();
    let l = session.lib_mut(id)?;
    let ops = changed
        .into_iter()
        .map(|mut a| {
            l.hlc = l.hlc.tick(now);
            a.stamp = l.hlc;
            Op::SetAnnotation {
                score: sel.score,
                page: sel.page,
                annotation: Box::new(a),
            }
        })
        .collect();
    finish(session, id, label, Transaction::new(ops), sel, None)
}

/// Scales the selection about its rectangle's centre.
fn select_scale(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.select.scale";
    let p: Option<&Map<String, Value>> = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["factor"])?;
    let factor = opt_f64(ID, p, "factor")?.ok_or_else(|| bad(ID, "`factor` is required".into()))?;
    if !(MIN_SCALE..=MAX_SCALE).contains(&factor) {
        return Err(bad(
            ID,
            format!("`factor` must be from {MIN_SCALE} to {MAX_SCALE}"),
        ));
    }
    let (mut sel, marks) = current(session, ID)?;
    let bounds = page_rect(session, file_page_of(ID, sel.page)?)?;
    let c = sel.rect.center();
    let map = move |q: Point| c + (q - c) * factor;
    let (a, b) = (
        map(Point::new(sel.rect.x0, sel.rect.y0)),
        map(Point::new(sel.rect.x1, sel.rect.y1)),
    );
    sel.rect = Rect::new(a.x, a.y, b.x, b.y);
    rewrite(session, ID, "Scale Selection", sel, marks, &|k| {
        transform(ID, k, &map, factor, bounds)
    })
}

/// Gives every selected marking `color`. Whiteout stays paper-coloured; the highlighter takes
/// the colour without alpha, as it always multiplies at full strength.
fn select_recolor(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.select.recolor";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["color"])?;
    let color = opt_color(ID, p, "color")?.ok_or_else(|| bad(ID, "`color` is required".into()))?;
    let rgb = Color::rgb(color.r, color.g, color.b);
    let (sel, marks) = current(session, ID)?;
    rewrite(session, ID, "Recolour Selection", sel, marks, &|k| {
        match k {
            AnnotationKind::Stroke(s) => match s.tool {
                Tool::Whiteout => {}
                Tool::Highlighter => s.color = ToolStyle::color_for(Tool::Highlighter, rgb),
                Tool::Pen => s.color = color,
            },
            AnnotationKind::Text(t) => t.style.color = rgb,
            AnnotationKind::Stamp(st) => st.color = rgb,
            AnnotationKind::Hairpin(h) => h.color = rgb,
            AnnotationKind::Snip(_) | AnnotationKind::Shortcut(_) => {}
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests;
