//! Text notes (feature-map ANNO-8; spec `docs/spec/ink.md`): plain text on the page, wrapped at
//! 86% of the page's width and kept inside the page, 8–36 pt (12 pt by default). Placing,
//! editing and moving are one undo step each; an edit that empties a note deletes it.

use serde_json::{Map, Value, json};

use scorecraft_doc::{
    Annotation, AnnotationId, AnnotationKind, PageKey, Score, TextNote, TextStyle,
};
use scorecraft_geom::{Point, Rect};
use scorecraft_input::PointerKind;
use scorecraft_ops::{Op, OpError, Transaction};
use scorecraft_text::{Face, FaceKind, MAX_TEXT_CHARS, NOTE_MAX_WIDTH, fit_inside, layout};

use crate::annotate_cmds::{markup_ready, opt_color, state_json, writable};
use crate::commands::{
    CommandSpec, always, bad, opt_bool, opt_f64, opt_int, opt_str, params_object,
    reject_unknown_keys, req_id, truncate,
};
use crate::markup::{MarkupTool, TEXT_SIZES, hit};
use crate::{EngineError, Session};

pub fn specs() -> Vec<CommandSpec> {
    vec![
        CommandSpec {
            id: "annotate.tool.text",
            label: "Text",
            menu: "Markup>Tools",
            shortcut: None,
            params: r##"{"size":8..36?,"color":"#rrggbb"?}"##,
            ours: true,
            enabled: always,
            run: tool_text,
        },
        CommandSpec {
            id: "annotate.text.add",
            label: "Add Text",
            menu: "",
            shortcut: None,
            params: r#"{"text":string,"x":number,"y":number,"space":"view|page"="view","page":int?,"device":"pen|mouse|touch"="pen"}"#,
            ours: false,
            enabled: markup_ready,
            run: text_add,
        },
        CommandSpec {
            id: "annotate.text.edit",
            label: "Edit Text",
            menu: "",
            shortcut: None,
            params: r##"{"annotation":id,"text":string?,"size":8..36?,"color":"#rrggbb"?,"bold":bool?,"italic":bool?,"background":"#rrggbbaa|null"?,"x":number?,"y":number?,"space":"view|page"="view"}"##,
            ours: false,
            enabled: markup_ready,
            run: text_edit,
        },
    ]
}

/// Chooses the text tool, optionally with a size and colour for new notes.
fn tool_text(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.tool.text";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["size", "color"])?;
    let size = text_size(ID, p)?;
    let color = opt_color(ID, p, "color")?;
    let m = &mut session.markup;
    m.tool = MarkupTool::Text;
    if let Some(s) = size {
        m.text_size = s;
    }
    if let Some(c) = color {
        m.apply_color(c);
    }
    Ok(state_json(session))
}

fn text_size(id: &str, p: Option<&Map<String, Value>>) -> Result<Option<f32>, EngineError> {
    match opt_f64(id, p, "size")? {
        None => Ok(None),
        Some(s) if TEXT_SIZES.contains(s as f32) => Ok(Some(s as f32)),
        Some(_) => Err(bad(
            id,
            format!(
                "`size` must be from {} to {} points",
                TEXT_SIZES.min, TEXT_SIZES.max
            ),
        )),
    }
}

/// The note's text: at most [`MAX_TEXT_CHARS`] characters; `None` when it is blank.
fn clean_text(id: &str, raw: &str) -> Result<Option<String>, EngineError> {
    if raw.chars().count() > MAX_TEXT_CHARS {
        return Err(bad(
            id,
            format!("a note holds at most {MAX_TEXT_CHARS} characters"),
        ));
    }
    let t = raw.trim_end().trim_start_matches('\n');
    Ok((!t.trim().is_empty()).then(|| t.to_owned()))
}

/// The visible area of a file page in its own PDF space.
pub(crate) fn page_rect(session: &Session, file_page: usize) -> Result<Rect, EngineError> {
    let open = session
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled("annotate.text".into()))?;
    let g = open.view.document().page(file_page)?;
    Ok(scorecraft_render::ink::page_box(g).visible())
}

/// Lays `text` out in `style` with its top-left corner at `at`, kept inside `page`.
pub(crate) fn place(
    text: &str,
    style: &TextStyle,
    at: Point,
    page: Rect,
) -> Result<Rect, EngineError> {
    let face = Face::get(FaceKind::Note, style.bold, style.italic)
        .ok_or_else(|| EngineError::Unsupported("the built-in text font".into()))?;
    let laid = layout(
        &face,
        text,
        f64::from(style.size),
        page.width() * NOTE_MAX_WIDTH,
    );
    let rect = Rect::new(at.x, at.y - laid.height, at.x + laid.width, at.y);
    Ok(fit_inside(rect, page))
}

/// Where a point given in `space` lands: score page, file page and page-space position.
pub(crate) fn locate(
    session: &mut Session,
    id: &str,
    p: Option<&Map<String, Value>>,
    x: f64,
    y: f64,
) -> Result<(usize, usize, Point), EngineError> {
    let space = opt_str(id, p, "space")?.unwrap_or("view");
    let page_param = opt_int(id, p, "page", 1, u64::from(u32::MAX))?;
    session.refresh_reader();
    let open = session
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled(id.to_owned()))?;
    match space {
        "view" => {
            if page_param.is_some() {
                return Err(bad(id, "`page` goes with `space: page` only".into()));
            }
            let h = hit(&open.reader, open.view.document(), x, y)
                .ok_or_else(|| bad(id, "the point is not on a page".into()))?;
            let at = h
                .to_page(x, y)
                .ok_or_else(|| bad(id, "the point is not on a page".into()))?;
            Ok((h.page, h.file_page, at))
        }
        "page" => {
            let n = page_param
                .ok_or_else(|| bad(id, "`page` is required with `space: page`".into()))?;
            let index =
                usize::try_from(n - 1).map_err(|_| bad(id, "`page` is too large".into()))?;
            let shown = open.reader.shown.get(index).ok_or_else(|| {
                bad(
                    id,
                    format!("`page` must be from 1 to {}", open.reader.shown.len()),
                )
            })?;
            let at = Point::new(x, y);
            if !at.is_finite() || x.abs() > 1.0e6 || y.abs() > 1.0e6 {
                return Err(bad(id, "the point is too far outside the page".into()));
            }
            Ok((index, shown.file_page, at))
        }
        other => Err(bad(
            id,
            format!("`space` must be view or page, got {}", truncate(other, 32)),
        )),
    }
}

pub(crate) fn required_f64(
    id: &str,
    p: Option<&Map<String, Value>>,
    key: &str,
) -> Result<f64, EngineError> {
    opt_f64(id, p, key)?.ok_or_else(|| bad(id, format!("`{key}` is required")))
}

/// Places a new note with its top-left corner at the tapped point (ANNO-8).
fn text_add(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.text.add";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["text", "x", "y", "space", "page", "device"])?;
    let raw = crate::commands::req_str(ID, p, "text")?;
    let text = clean_text(ID, raw)?.ok_or_else(|| bad(ID, "a note needs some text".into()))?;
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
        // Type with Touch is off: the tap turns or pans instead.
        return Ok(json!({ "added": false, "reason": "penOnly" }));
    }
    let (page, file_page, at) = locate(session, ID, p, x, y)?;
    let bounds = page_rect(session, file_page)?;
    let style = TextStyle {
        size: session.markup.text_size,
        color: session.markup.text_color,
        ..TextStyle::default()
    };
    let rect = place(&text, &style, at, bounds)?;
    let score_id = session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let markup_layer = {
        let s = library_score(session, score_id)?;
        let layer = session
            .markup
            .layer(s)
            .ok_or_else(|| OpError::Missing("layer".into()))?;
        writable(s, layer)?;
        (layer, s.annotations_on(page_key(file_page)?).len())
    };
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    let id: AnnotationId = l.ids.id(now);
    l.hlc = l.hlc.tick(now);
    let annotation = Annotation {
        id,
        layer: markup_layer.0,
        stamp: l.hlc,
        kind: AnnotationKind::Text(TextNote { rect, text, style }),
    };
    l.commit(
        "Add Text",
        Transaction::single(Op::InsertAnnotation {
            score: score_id,
            page: page_key(file_page)?,
            index: markup_layer.1,
            annotation: Box::new(annotation),
        }),
    )?;
    Ok(json!({
        "added": true,
        "annotation": id.to_string(),
        "page": page + 1,
        "filePage": file_page + 1,
        "rect": [rect.x0, rect.y0, rect.x1, rect.y1],
    }))
}

pub(crate) fn page_key(file_page: usize) -> Result<PageKey, EngineError> {
    u32::try_from(file_page)
        .map(PageKey)
        .map_err(|_| EngineError::Unsupported("a page past 4 billion".into()))
}

pub(crate) fn library_score(
    session: &Session,
    id: scorecraft_doc::ScoreId,
) -> Result<&Score, EngineError> {
    session
        .library()
        .and_then(|l| l.scores.get(&id))
        .ok_or(OpError::NoScore(id).into())
}

/// Changes a note's text, style or place. Empty text deletes it.
fn text_edit(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.text.edit";
    let p = params_object(ID, params)?;
    reject_unknown_keys(
        ID,
        p,
        &[
            "annotation",
            "text",
            "size",
            "color",
            "bold",
            "italic",
            "background",
            "x",
            "y",
            "space",
        ],
    )?;
    let id: AnnotationId = req_id(ID, p, "annotation")?;
    let score_id = session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let (page, mut annotation) = {
        let s = library_score(session, score_id)?;
        s.annotations
            .iter()
            .find_map(|(page, list)| list.iter().find(|a| a.id == id).map(|a| (*page, a.clone())))
            .ok_or_else(|| OpError::Missing(format!("annotation {id}")))?
    };
    writable(library_score(session, score_id)?, annotation.layer)?;
    let AnnotationKind::Text(mut note) = annotation.kind.clone() else {
        return Err(bad(ID, "that marking is not a text note".into()));
    };
    if let Some(raw) = opt_str(ID, p, "text")? {
        match clean_text(ID, raw)? {
            Some(t) => note.text = t,
            None => {
                // An emptied note is deleted.
                session.lib_mut(ID)?.commit(
                    "Delete Text",
                    Transaction::single(Op::RemoveAnnotation {
                        score: score_id,
                        page,
                        annotation: id,
                    }),
                )?;
                return Ok(json!({ "deleted": true, "annotation": id.to_string() }));
            }
        }
    }
    if let Some(s) = text_size(ID, p)? {
        note.style.size = s;
    }
    if let Some(c) = opt_color(ID, p, "color")? {
        note.style.color = c;
    }
    if let Some(b) = opt_bool(ID, p, "bold")? {
        note.style.bold = b;
    }
    if let Some(i) = opt_bool(ID, p, "italic")? {
        note.style.italic = i;
    }
    match p.and_then(|m| m.get("background")) {
        None => {}
        Some(Value::Null) => note.style.background = None,
        Some(_) => note.style.background = opt_color(ID, p, "background")?,
    }
    // Moved: the new top-left corner, which must be on the same page.
    let mut at = Point::new(note.rect.x0, note.rect.y1);
    match (opt_f64(ID, p, "x")?, opt_f64(ID, p, "y")?) {
        (Some(x), Some(y)) => {
            if opt_str(ID, p, "space")? == Some("page") {
                // Page space is the note's own page.
                if !(x.is_finite() && y.is_finite() && x.abs() <= 1.0e6 && y.abs() <= 1.0e6) {
                    return Err(bad(ID, "the point is too far outside the page".into()));
                }
                at = Point::new(x, y);
            } else {
                let (_, file_page, to) = locate(session, ID, p, x, y)?;
                if page_key(file_page)? != page {
                    return Err(bad(ID, "a note stays on its page".into()));
                }
                at = to;
            }
        }
        (None, None) => {}
        _ => return Err(bad(ID, "give both `x` and `y` to move a note".into())),
    }
    let file_page = usize::try_from(page.0).map_err(|_| bad(ID, "page out of range".into()))?;
    let bounds = page_rect(session, file_page)?;
    note.rect = place(&note.text, &note.style, at, bounds)?;
    let now = session.now_ms();
    let l = session.lib_mut(ID)?;
    l.hlc = l.hlc.tick(now);
    annotation.stamp = l.hlc;
    let rect = note.rect;
    annotation.kind = AnnotationKind::Text(note);
    l.commit(
        "Edit Text",
        Transaction::single(Op::SetAnnotation {
            score: score_id,
            page,
            annotation: Box::new(annotation),
        }),
    )?;
    Ok(json!({
        "deleted": false,
        "annotation": id.to_string(),
        "rect": [rect.x0, rect.y0, rect.x1, rect.y1],
    }))
}

#[cfg(test)]
mod tests;
