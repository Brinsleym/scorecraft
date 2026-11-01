//! Score Shortcuts (feature-map NAV-1) and Score Snips (PAGE-7), spec
//! `docs/spec/performance.md`. A shortcut is a tappable rectangle on a page that jumps to
//! another page of the score; a snip shows a rectangle of one page at a place on another. Both
//! are annotations on the active layer, in source-page space, each change one undo step.

use serde_json::{Map, Value, json};

use scorecraft_doc::{Annotation, AnnotationId, AnnotationKind, PageArea, PageKey, Shortcut, Snip};
use scorecraft_geom::{Point, Rect};
use scorecraft_ops::{Op, OpError, Transaction};
use scorecraft_text::fit_inside;

use crate::annotate_cmds::{markup_ready, writable};
use crate::commands::{
    CommandSpec, bad, opt_int, opt_str, params_object, reject_unknown_keys, req_id,
};
use crate::text_cmds::{library_score, locate, page_key, page_rect, required_f64};
use crate::{EngineError, Session};

pub fn specs() -> Vec<CommandSpec> {
    let spec = |id, label, menu, params, ours, run| CommandSpec {
        id,
        label,
        menu,
        shortcut: None,
        params,
        ours,
        enabled: markup_ready,
        run,
    };
    vec![
        spec(
            "nav.shortcut.add",
            "Add Shortcut",
            "Markup",
            r#"{"x0":number,"y0":number,"x1":number,"y1":number,"space":"view|page"="view","page":int?,"target":int}"#,
            false,
            shortcut_add as _,
        ),
        spec(
            "nav.shortcut.follow",
            "Follow Shortcut",
            "",
            r#"{"x":number,"y":number,"space":"view|page"="view","page":int?}"#,
            false,
            shortcut_follow as _,
        ),
        spec(
            "page.snip.create",
            "Snip",
            "Markup",
            r#"{"x0":number,"y0":number,"x1":number,"y1":number,"space":"view|page"="view","page":int?,"toX":number,"toY":number,"toPage":int?}"#,
            false,
            snip_create as _,
        ),
        spec(
            "page.snip.place",
            "Move Snip",
            "",
            r#"{"annotation":id,"x":number,"y":number,"space":"view|page"="view","page":int?,"width":number?}"#,
            false,
            snip_place as _,
        ),
        spec(
            "annotate.delete",
            "Delete Marking",
            "",
            r#"{"annotation":id}"#,
            true,
            delete as _,
        ),
    ]
}

fn open_score(session: &Session, id: &str) -> Result<scorecraft_doc::ScoreId, EngineError> {
    session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(id.to_owned()))
}

/// The rectangle from two corners given in `space`, on one page.
fn rect_on_page(
    session: &mut Session,
    id: &str,
    p: Option<&Map<String, Value>>,
) -> Result<(usize, Rect), EngineError> {
    let (x0, y0) = (required_f64(id, p, "x0")?, required_f64(id, p, "y0")?);
    let (x1, y1) = (required_f64(id, p, "x1")?, required_f64(id, p, "y1")?);
    let (_, fp, a) = locate(session, id, p, x0, y0)?;
    let (_, other, b) = locate(session, id, p, x1, y1)?;
    if fp != other {
        return Err(bad(id, "the rectangle must be on one page".into()));
    }
    let r = Rect::new(a.x, a.y, b.x, b.y);
    if r.width() < 1.0 || r.height() < 1.0 {
        return Err(bad(id, "the rectangle needs some size".into()));
    }
    Ok((fp, r))
}

/// Inserts `kind` on `file_page` on the active layer, as one undo step.
fn insert(
    session: &mut Session,
    id: &str,
    label: &str,
    file_page: usize,
    kind: AnnotationKind,
) -> Result<AnnotationId, EngineError> {
    let score_id = open_score(session, id)?;
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
    let l = session.lib_mut(id)?;
    let aid: AnnotationId = l.ids.id(now);
    l.hlc = l.hlc.tick(now);
    l.commit(
        label,
        Transaction::single(Op::InsertAnnotation {
            score: score_id,
            page: key,
            index,
            annotation: Box::new(Annotation {
                id: aid,
                layer,
                stamp: l.hlc,
                kind,
            }),
        }),
    )?;
    Ok(aid)
}

fn shortcut_add(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "nav.shortcut.add";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["x0", "y0", "x1", "y1", "space", "page", "target"])?;
    let target = opt_int(ID, p, "target", 1, u64::from(u32::MAX))?
        .ok_or_else(|| bad(ID, "`target` (a page of the score) is required".into()))?;
    session.refresh_reader();
    let shown = session
        .score
        .as_ref()
        .map(|o| o.reader.shown.clone())
        .unwrap_or_default();
    let to = usize::try_from(target - 1)
        .ok()
        .and_then(|i| shown.get(i))
        .ok_or_else(|| bad(ID, format!("`target` must be from 1 to {}", shown.len())))?
        .file_page;
    let (fp, rect) = rect_on_page(session, ID, p)?;
    let at = PageArea {
        page: page_key(fp)?,
        rect,
    };
    let aid = insert(
        session,
        ID,
        "Add Shortcut",
        fp,
        AnnotationKind::Shortcut(Shortcut {
            at,
            target: page_key(to)?,
            target_rect: None,
        }),
    )?;
    Ok(json!({ "annotation": aid.to_string(), "filePage": fp + 1, "target": target }))
}

/// Jumps when the point is on a shortcut on a visible layer; otherwise nothing happens and the
/// shell turns or toggles as usual.
fn shortcut_follow(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "nav.shortcut.follow";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["x", "y", "space", "page"])?;
    let (x, y) = (required_f64(ID, p, "x")?, required_f64(ID, p, "y")?);
    let Ok((_, fp, at)) = locate(session, ID, p, x, y) else {
        return Ok(json!({ "followed": false }));
    };
    let score_id = open_score(session, ID)?;
    let target = {
        let s = library_score(session, score_id)?;
        let key = page_key(fp)?;
        s.annotations_on(key)
            .iter()
            .rev()
            .filter(|a| s.layer(a.layer).is_some_and(|l| l.visible))
            .find_map(|a| match &a.kind {
                AnnotationKind::Shortcut(sc) if contains(sc.at.rect, at) => Some(sc.target),
                _ => None,
            })
    };
    let Some(target) = target else {
        return Ok(json!({ "followed": false }));
    };
    let index = session.score.as_ref().and_then(|o| {
        o.reader
            .shown
            .iter()
            .position(|s| u32::try_from(s.file_page).is_ok_and(|f| PageKey(f) == target))
    });
    let Some(index) = index else {
        // The target page is hidden or outside the range: nothing to jump to.
        return Ok(json!({ "followed": false, "reason": "targetHidden" }));
    };
    let mut v = session.dispatch("view.page.goto", &json!({ "page": index + 1 }))?;
    crate::commands::set(&mut v, "followed", json!(true));
    Ok(v)
}

fn contains(r: Rect, p: Point) -> bool {
    p.x >= r.x0 && p.x <= r.x1 && p.y >= r.y0 && p.y <= r.y1
}

/// The snip's place: `from`'s shape at `width` points wide (its own width by default), its
/// top-left corner at `top_left`, inside `page`.
fn place_rect(from: Rect, top_left: Point, page: Rect, width: Option<f64>) -> Rect {
    let w = width.unwrap_or(from.width());
    let h = w * from.height() / from.width().max(1e-9);
    fit_inside(
        Rect::new(top_left.x, top_left.y - h, top_left.x + w, top_left.y),
        page,
    )
}

/// Where the snip goes: (`toX`, `toY`) in the same space, on `toPage` in page space.
fn destination(
    session: &mut Session,
    id: &str,
    p: Option<&Map<String, Value>>,
    xk: &str,
    yk: &str,
    page_key_name: &str,
) -> Result<(usize, Point), EngineError> {
    let (x, y) = (required_f64(id, p, xk)?, required_f64(id, p, yk)?);
    let space = opt_str(id, p, "space")?.unwrap_or("view");
    let mut sub = Map::new();
    sub.insert("space".into(), json!(space));
    if space == "page" {
        let page = p
            .and_then(|m| m.get(page_key_name))
            .cloned()
            .ok_or_else(|| {
                bad(
                    id,
                    format!("`{page_key_name}` is required with `space: page`"),
                )
            })?;
        sub.insert("page".into(), page);
    }
    let (_, fp, at) = locate(session, id, Some(&sub), x, y)?;
    Ok((fp, at))
}

fn snip_create(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.snip.create";
    let p = params_object(ID, params)?;
    reject_unknown_keys(
        ID,
        p,
        &[
            "x0", "y0", "x1", "y1", "space", "page", "toX", "toY", "toPage",
        ],
    )?;
    let (from_page, from) = rect_on_page(session, ID, p)?;
    let (to_page, top_left) = destination(session, ID, p, "toX", "toY", "toPage")?;
    let bounds = page_rect(session, to_page)?;
    let to = place_rect(from, top_left, bounds, None);
    let aid = insert(
        session,
        ID,
        "Snip",
        to_page,
        AnnotationKind::Snip(Snip {
            from: PageArea {
                page: page_key(from_page)?,
                rect: from,
            },
            to: PageArea {
                page: page_key(to_page)?,
                rect: to,
            },
        }),
    )?;
    Ok(json!({
        "annotation": aid.to_string(),
        "from": from_page + 1,
        "filePage": to_page + 1,
        "rect": [to.x0, to.y0, to.x1, to.y1],
    }))
}

fn snip_place(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "page.snip.place";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["annotation", "x", "y", "space", "page", "width"])?;
    let aid: AnnotationId = req_id(ID, p, "annotation")?;
    // Resizing keeps the snip's shape: 10% to 4× its source's width.
    let width = crate::commands::opt_f64(ID, p, "width")?;
    let score_id = open_score(session, ID)?;
    let (page, mut a) = find(session, score_id, aid)?;
    let (fp, top_left) = destination(session, ID, p, "x", "y", "page")?;
    if page_key(fp)? != page {
        return Err(bad(ID, "a snip stays on its page".into()));
    }
    let bounds = page_rect(session, fp)?;
    let AnnotationKind::Snip(s) = &mut a.kind else {
        return Err(bad(ID, "that marking is not a snip".into()));
    };
    if let Some(w) = width {
        let fw = s.from.rect.width();
        if !(w.is_finite() && w >= fw * 0.1 && w <= fw * 4.0) {
            return Err(bad(
                ID,
                "`width` must be from 10% to 4 times the snipped width".into(),
            ));
        }
    }
    s.to.rect = place_rect(s.from.rect, top_left, bounds, width);
    let rect = s.to.rect;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    l.hlc = l.hlc.tick(now);
    a.stamp = l.hlc;
    l.commit(
        "Move Snip",
        Transaction::single(Op::SetAnnotation {
            score: score_id,
            page,
            annotation: Box::new(a),
        }),
    )?;
    Ok(json!({ "annotation": aid.to_string(), "rect": [rect.x0, rect.y0, rect.x1, rect.y1] }))
}

/// The page and copy of annotation `aid`, on a layer that can be changed.
fn find(
    session: &Session,
    score: scorecraft_doc::ScoreId,
    aid: AnnotationId,
) -> Result<(PageKey, Annotation), EngineError> {
    let s = library_score(session, score)?;
    let found = s
        .annotations
        .iter()
        .find_map(|(pg, list)| list.iter().find(|a| a.id == aid).map(|a| (*pg, a.clone())))
        .ok_or_else(|| OpError::Missing(format!("annotation {aid}")))?;
    writable(s, found.1.layer)?;
    Ok(found)
}

fn delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["annotation"])?;
    let aid: AnnotationId = req_id(ID, p, "annotation")?;
    let score_id = open_score(session, ID)?;
    let (page, a) = find(session, score_id, aid)?;
    let label = match a.kind {
        AnnotationKind::Shortcut(_) => "Delete Shortcut",
        AnnotationKind::Snip(_) => "Delete Snip",
        _ => "Delete Marking",
    };
    session.lib_mut(ID)?.commit(
        label,
        Transaction::single(Op::RemoveAnnotation {
            score: score_id,
            page,
            annotation: aid,
        }),
    )?;
    Ok(json!({ "deleted": aid.to_string() }))
}

#[cfg(test)]
mod tests;
