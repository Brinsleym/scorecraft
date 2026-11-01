//! Markup commands (spec `docs/spec/ink.md`): choosing tools, colours and widths (ANNO-3..7),
//! *Draw with Touch* (ANNO-14), and `annotate.stroke`, which applies the active tool to one
//! pen-down to pen-up gesture (ANNO-1, ANNO-2, ANNO-6, ANNO-7). Every marking is data in the
//! library in source-page PDF space; the imported file is never touched (ANNO-15). Each gesture
//! is one undo step in the score's history (ANNO-13).

use serde_json::{Map, Value, json};

use scorecraft_doc::{
    Annotation, AnnotationId, AnnotationKind, Color, InkPoint, Layer, PageKey, Score, ScoreId,
    Stroke, Tool,
};
use scorecraft_geom::Point;
use scorecraft_ink::{ERASER_WIDTHS, Erased, InkError, MAX_POINTS, ToolStyle, erase, touches};
use scorecraft_input::PointerKind;
use scorecraft_ops::{Op, OpError, Transaction};

use crate::commands::{
    CommandSpec, always, bad, opt_bool, opt_f64, opt_int, opt_str, params_object,
    reject_unknown_keys, truncate,
};
use crate::markup::{EraserMode, MarkupTool, hit, library_score};
use crate::{EngineError, Session};

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
            "annotate.tool.pen",
            "Pen",
            "Markup>Tools",
            r##"{"color":"#rrggbb|#rrggbbaa"?,"width":number?}"##,
            false,
            always,
            tool_pen as _,
        ),
        spec(
            "annotate.tool.highlighter",
            "Highlighter",
            "Markup>Tools",
            r##"{"color":"#rrggbb"?,"width":number?}"##,
            false,
            always,
            tool_highlighter as _,
        ),
        spec(
            "annotate.tool.whiteout",
            "Whiteout",
            "Markup>Tools",
            r#"{"width":number?}"#,
            false,
            always,
            tool_whiteout as _,
        ),
        spec(
            "annotate.tool.strokeEraser",
            "Eraser (Whole Strokes)",
            "Markup>Tools",
            r#"{"width":number?}"#,
            false,
            always,
            tool_stroke_eraser as _,
        ),
        spec(
            "annotate.tool.pixelEraser",
            "Eraser (Part of a Stroke)",
            "Markup>Tools",
            r#"{"width":number?}"#,
            false,
            always,
            tool_pixel_eraser as _,
        ),
        spec(
            "annotate.style.set",
            "Colour and Width",
            "",
            r##"{"color":"#rrggbb|#rrggbbaa"?,"width":number?}"##,
            true,
            always,
            style_set as _,
        ),
        spec(
            "input.pen.only",
            "Draw with Touch",
            "Markup",
            r#"{"on":bool?}"#,
            false,
            always,
            pen_only as _,
        ),
        spec(
            "annotate.swatch.select",
            "Swatch",
            "",
            r#"{"index":1..8}"#,
            true,
            always,
            swatch_select as _,
        ),
        spec(
            "annotate.swatch.set",
            "Palette Colour",
            "",
            r##"{"color":"#rrggbb","index":1..8?}"##,
            true,
            always,
            swatch_set as _,
        ),
        spec(
            "annotate.pen.preset.set",
            "Save Preset",
            "Markup>Presets",
            r#"{"name":string}"#,
            false,
            always,
            preset_set as _,
        ),
        spec(
            "annotate.pen.preset.apply",
            "Use Preset",
            "Markup>Presets",
            r#"{"name":string}"#,
            true,
            always,
            preset_apply as _,
        ),
        spec(
            "annotate.pen.preset.delete",
            "Delete Preset",
            "Markup>Presets",
            r#"{"name":string}"#,
            true,
            always,
            preset_delete as _,
        ),
        spec(
            "annotate.highlighter.roundEnds",
            "Round Highlighter Ends",
            "Settings>Markup",
            r#"{"on":bool?}"#,
            true,
            always,
            highlighter_round_ends as _,
        ),
        spec(
            "annotate.mode.set",
            "Markup",
            "Markup",
            r#"{"on":bool?}"#,
            true,
            always,
            mode_set as _,
        ),
        spec(
            "input.touch.lock",
            "Touch Lock",
            "Markup",
            r#"{"on":bool?}"#,
            false,
            markup_mode,
            touch_lock as _,
        ),
        spec(
            "annotate.ruler.show",
            "Ruler",
            "Markup",
            r#"{"on":bool?}"#,
            false,
            markup_ready,
            ruler_show as _,
        ),
        spec(
            "annotate.ruler.set",
            "Move Ruler",
            "",
            r#"{"y":number}"#,
            false,
            markup_ready,
            ruler_set as _,
        ),
        spec(
            "annotate.shapes",
            "Shapes",
            "Markup",
            r#"{"on":bool?}"#,
            false,
            always,
            shapes as _,
        ),
        spec(
            "annotate.pen.button",
            "Pen Button",
            "",
            "{}",
            false,
            always,
            pen_button as _,
        ),
        spec(
            "annotate.pen.buttonTool",
            "Pen Button Tool",
            "Settings>Markup",
            r#"{"tool":"pen|highlighter|eraser|text|stamp|select"}"#,
            false,
            always,
            pen_button_tool as _,
        ),
        spec(
            "annotate.stroke",
            "Draw",
            "",
            r#"{"points":[[x,y,pressure?,t?]],"space":"view|page"="view","page":int?,"device":"pen|eraser|mouse|touch"="pen"}"#,
            false,
            markup_ready,
            stroke as _,
        ),
        spec(
            "annotate.page.list",
            "Markings on a Page",
            "",
            r#"{"page":int?}"#,
            true,
            markup_ready,
            page_list as _,
        ),
        spec(
            "annotate.state",
            "Markup State",
            "",
            "{}",
            true,
            always,
            state as _,
        ),
    ]
}

/// Enabled while a score from the library is open: markings are stored in the library.
pub fn markup_ready(s: &Session) -> bool {
    s.score.as_ref().and_then(|o| o.library_score).is_some() && s.library.is_some()
}

/// Optional colour param.
pub(crate) fn opt_color(
    id: &str,
    p: Option<&Map<String, Value>>,
    key: &str,
) -> Result<Option<Color>, EngineError> {
    opt_str(id, p, key)?
        .map(|s| {
            s.parse::<Color>()
                .map_err(|e| bad(id, format!("`{key}`: {e}")))
        })
        .transpose()
}

/// Optional width param inside `range`.
fn opt_width(
    id: &str,
    p: Option<&Map<String, Value>>,
    range: scorecraft_ink::WidthRange,
) -> Result<Option<f32>, EngineError> {
    match opt_f64(id, p, "width")? {
        None => Ok(None),
        Some(w) if range.contains(w as f32) => Ok(Some(w as f32)),
        Some(_) => Err(bad(
            id,
            format!("`width` must be from {} to {} points", range.min, range.max),
        )),
    }
}

pub(crate) fn state_json(s: &Session) -> Value {
    let layer = s
        .score
        .as_ref()
        .and_then(|o| library_score(s.library(), o.library_score))
        .and_then(|sc| s.markup.layer(sc));
    s.markup.to_json(layer)
}

/// Selects a stroke tool and applies optional colour and width to it.
fn choose(
    session: &mut Session,
    id: &str,
    params: &Value,
    tool: Tool,
    with_color: bool,
) -> Result<Value, EngineError> {
    let p = params_object(id, params)?;
    let keys: &[&str] = if with_color {
        &["color", "width"]
    } else {
        &["width"]
    };
    reject_unknown_keys(id, p, keys)?;
    let color = if with_color {
        opt_color(id, p, "color")?
    } else {
        None
    };
    let width = opt_width(id, p, ToolStyle::widths(tool))?;
    let m = &mut session.markup;
    m.tool = match tool {
        Tool::Highlighter => MarkupTool::Highlighter,
        _ => MarkupTool::Pen,
    };
    if tool == Tool::Whiteout {
        m.whiteout_on = true;
    } else if tool == Tool::Pen && color.is_none() && m.whiteout_on {
        // Choosing the pen again after whiteout returns it to ink.
        m.whiteout_on = false;
    }
    if let Some(c) = color {
        m.apply_color(c);
        select_matching_swatch(m, c);
    }
    if let Some(w) = width {
        style_mut(m, tool).width = w;
    }
    Ok(state_json(session))
}

fn style_mut(m: &mut crate::markup::Markup, tool: Tool) -> &mut ToolStyle {
    match tool {
        Tool::Pen => &mut m.pen,
        Tool::Highlighter => &mut m.highlighter,
        Tool::Whiteout => &mut m.whiteout,
    }
}

/// Marks the swatch holding `c`, if one does, as the active tool's selected swatch.
fn select_matching_swatch(m: &mut crate::markup::Markup, c: Color) {
    let rgb = Color::rgb(c.r, c.g, c.b);
    if let Some((swatches, selected, _)) = m.swatches_mut()
        && let Some(i) = swatches.iter().position(|s| *s == rgb)
    {
        *selected = i;
    }
}

fn tool_pen(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    choose(session, "annotate.tool.pen", params, Tool::Pen, true)
}

fn tool_highlighter(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    choose(
        session,
        "annotate.tool.highlighter",
        params,
        Tool::Highlighter,
        true,
    )
}

fn tool_whiteout(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    choose(
        session,
        "annotate.tool.whiteout",
        params,
        Tool::Whiteout,
        false,
    )
}

fn choose_eraser(
    session: &mut Session,
    id: &str,
    params: &Value,
    mode: EraserMode,
) -> Result<Value, EngineError> {
    let p = params_object(id, params)?;
    reject_unknown_keys(id, p, &["width"])?;
    let width = opt_width(id, p, ERASER_WIDTHS)?;
    let m = &mut session.markup;
    m.tool = MarkupTool::Eraser;
    m.eraser = mode;
    if let Some(w) = width {
        m.eraser_width = w;
    }
    Ok(state_json(session))
}

fn tool_stroke_eraser(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    choose_eraser(
        session,
        "annotate.tool.strokeEraser",
        params,
        EraserMode::Stroke,
    )
}

fn tool_pixel_eraser(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    choose_eraser(
        session,
        "annotate.tool.pixelEraser",
        params,
        EraserMode::Pixel,
    )
}

/// The width slider and any colour: they change the active tool's style. Agents may give any
/// colour here; the toolbar offers the swatches and the palette (`annotate.swatch.*`).
fn style_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.style.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["color", "width"])?;
    let color = opt_color(ID, p, "color")?;
    let m = &mut session.markup;
    let width = match m.tool {
        MarkupTool::Eraser => {
            if color.is_some() {
                return Err(bad(ID, "the eraser has no colour".into()));
            }
            let w = opt_width(ID, p, ERASER_WIDTHS)?;
            if let Some(w) = w {
                m.eraser_width = w;
            }
            w
        }
        // With the stamp tool the slider is the stamp size (ANNO-9).
        MarkupTool::Stamp => {
            let w = opt_width(ID, p, crate::markup::STAMP_SIZES)?;
            if let Some(w) = w {
                m.stamp_size = w;
            }
            if let Some(c) = color {
                m.apply_color(c);
                select_matching_swatch(m, c);
            }
            w
        }
        // With the text tool the slider is the text size (ANNO-8).
        MarkupTool::Text => {
            let w = opt_width(ID, p, crate::markup::TEXT_SIZES)?;
            if let Some(w) = w {
                m.text_size = w;
            }
            if let Some(c) = color {
                m.apply_color(c);
                select_matching_swatch(m, c);
            }
            w
        }
        MarkupTool::Select => {
            return Err(bad(
                ID,
                "the rectangle tool has no colour or width; recolour a selection with annotate.select.recolor".into(),
            ));
        }
        _ => {
            let tool = m.drawing().map_or(Tool::Pen, |(t, _)| t);
            let w = opt_width(ID, p, ToolStyle::widths(tool))?;
            if let Some(w) = w {
                style_mut(m, tool).width = w;
            }
            if let Some(c) = color {
                m.apply_color(c);
                select_matching_swatch(m, c);
            }
            w
        }
    };
    if color.is_none() && width.is_none() {
        return Err(bad(ID, "give `color` or `width`".into()));
    }
    Ok(state_json(session))
}

/// Tapping a swatch: the active tool takes its colour (ANNO-4).
fn swatch_select(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.swatch.select";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["index"])?;
    let i = swatch_index(ID, p)?.ok_or_else(|| bad(ID, "`index` is required".into()))?;
    let m = &mut session.markup;
    let (swatches, selected, _) = m
        .swatches_mut()
        .ok_or_else(|| bad(ID, "the eraser has no colour".into()))?;
    let color = swatches.get(i).copied().unwrap_or(Color::BLACK);
    *selected = i;
    m.apply_color(color);
    Ok(state_json(session))
}

/// Picking from the active tool's palette: its selected swatch (or swatch `index`) becomes that
/// colour, and the tool takes it (ANNO-4). Only that palette's colours are accepted.
fn swatch_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.swatch.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["color", "index"])?;
    let color = opt_color(ID, p, "color")?.ok_or_else(|| bad(ID, "`color` is required".into()))?;
    let color = Color::rgb(color.r, color.g, color.b);
    let index = swatch_index(ID, p)?;
    let m = &mut session.markup;
    let tool = m.tool.id();
    let (swatches, selected, palette) = m
        .swatches_mut()
        .ok_or_else(|| bad(ID, "the eraser has no swatches".into()))?;
    if !palette.contains(&color) {
        return Err(bad(
            ID,
            format!("{color} is not one of the {tool}'s palette colours"),
        ));
    }
    let i = index.unwrap_or(*selected);
    *selected = i;
    if let Some(sw) = swatches.get_mut(i) {
        *sw = color;
    }
    m.apply_color(color);
    Ok(state_json(session))
}

/// Longest preset name.
const MAX_PRESET_NAME: usize = 64;

fn preset_name(id: &str, p: Option<&Map<String, Value>>) -> Result<String, EngineError> {
    let raw = crate::commands::req_str(id, p, "name")?;
    crate::commands::check_len(id, "name", raw, MAX_PRESET_NAME)?;
    let n = raw.trim();
    if n.is_empty() {
        return Err(bad(id, "`name` can't be empty".into()));
    }
    Ok(n.to_owned())
}

/// Saves the active drawing tool's colour and width under `name` (ANNO-4), replacing a preset
/// of the same name.
fn preset_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.pen.preset.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name"])?;
    let name = preset_name(ID, p)?;
    let m = &mut session.markup;
    let (tool, style) = m
        .drawing()
        .ok_or_else(|| bad(ID, "choose the pen or the highlighter first".into()))?;
    let preset = crate::markup::Preset {
        name: name.clone(),
        tool,
        color: style.color,
        width: style.width,
    };
    match m.presets.iter_mut().find(|q| q.name == name) {
        Some(slot) => *slot = preset,
        None => {
            if m.presets.len() >= crate::markup::MAX_PRESETS {
                return Err(bad(
                    ID,
                    format!("at most {} presets are kept", crate::markup::MAX_PRESETS),
                ));
            }
            m.presets.push(preset);
        }
    }
    Ok(state_json(session))
}

/// Switches to a preset's tool, colour and width in one action.
fn preset_apply(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.pen.preset.apply";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name"])?;
    let name = preset_name(ID, p)?;
    let m = &mut session.markup;
    let preset = m
        .presets
        .iter()
        .find(|q| q.name == name)
        .cloned()
        .ok_or_else(|| OpError::Missing(format!("preset “{}”", truncate(&name, 64))))?;
    m.tool = match preset.tool {
        Tool::Highlighter => MarkupTool::Highlighter,
        _ => MarkupTool::Pen,
    };
    m.whiteout_on = preset.tool == Tool::Whiteout;
    let style = style_mut(m, preset.tool);
    style.color = preset.color;
    style.width = ToolStyle::widths(preset.tool).clamp(preset.width);
    Ok(state_json(session))
}

fn preset_delete(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.pen.preset.delete";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["name"])?;
    let name = preset_name(ID, p)?;
    let before = session.markup.presets.len();
    session.markup.presets.retain(|q| q.name != name);
    if session.markup.presets.len() == before {
        return Err(OpError::Missing(format!("preset “{}”", truncate(&name, 64))).into());
    }
    Ok(state_json(session))
}

/// The *Round highlighter ends* setting (ANNO-3): new highlighter strokes get a round nib.
fn highlighter_round_ends(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.highlighter.roundEnds";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.markup.highlighter_round);
    session.markup.highlighter_round = on;
    Ok(state_json(session))
}

/// Optional 1-based swatch number, as a 0-based index.
fn swatch_index(id: &str, p: Option<&Map<String, Value>>) -> Result<Option<usize>, EngineError> {
    Ok(opt_int(id, p, "index", 1, 8)?.and_then(|n| usize::try_from(n - 1).ok()))
}

/// Markup mode on or off (no params toggles). Leaving it hides the ruler.
fn mode_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.mode.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.markup.mode);
    session.markup.mode = on;
    if !on {
        session.markup.ruler = None;
    }
    Ok(state_json(session))
}

/// Touch lock (the padlock), in markup mode only (ANNO-14).
fn touch_lock(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.touch.lock";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.markup.touch_lock);
    session.markup.touch_lock = on;
    Ok(json!({ "touchLock": on }))
}

fn markup_mode(s: &Session) -> bool {
    s.markup.mode
}

/// Shows or hides the ruler (ANNO-17): a band across the whole width of the reader, centred
/// vertically. It lies on the screen, so it stays put while pages turn, zoom and pan.
fn ruler_show(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.ruler.show";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(session.markup.ruler.is_none());
    session.markup.ruler = if on {
        let open = session
            .score
            .as_ref()
            .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
        Some(open.reader.viewport.height / 2.0)
    } else {
        None
    };
    Ok(state_json(session))
}

/// Moves the ruler up or down: `y` is the band's centre in viewport pixels, kept on screen. It
/// stays horizontal; turning it is ANNO-18.
fn ruler_set(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.ruler.set";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["y"])?;
    let y = opt_f64(ID, p, "y")?.ok_or_else(|| bad(ID, "`y` is required".into()))?;
    let open = session
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let height = open.reader.viewport.height;
    session.markup.ruler = Some(y.clamp(0.0, height.max(0.0)));
    Ok(state_json(session))
}

/// *Draw with Touch*: `on` sets pen-only drawing, no params toggles it.
fn pen_only(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "input.pen.only";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.markup.pen_only);
    session.markup.pen_only = on;
    Ok(json!({ "penOnly": on, "drawWithTouch": !on }))
}

/// Shapes on or off (ANNO-16); toggles without `on`.
fn shapes(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.shapes";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["on"])?;
    let on = opt_bool(ID, p, "on")?.unwrap_or(!session.markup.shapes);
    session.markup.shapes = on;
    Ok(state_json(session))
}

/// The stylus barrel button or double-tap (ANNO-11): switches to the button's tool, or back.
fn pen_button(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.pen.button";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    session.markup.press_button();
    Ok(state_json(session))
}

/// Which tool the pen button switches to (the eraser by default).
fn pen_button_tool(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.pen.buttonTool";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["tool"])?;
    let name = opt_str(ID, p, "tool")?.ok_or_else(|| bad(ID, "`tool` is required".into()))?;
    let tool = MarkupTool::from_id(name).ok_or_else(|| {
        bad(
            ID,
            format!(
                "`tool` must be pen, highlighter, eraser, text, stamp or select, got {}",
                truncate(name, 32)
            ),
        )
    })?;
    let m = &mut session.markup;
    m.button_tool = tool;
    m.button_return = None;
    Ok(state_json(session))
}

fn state(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.state";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &[])?;
    Ok(state_json(session))
}

/// `[x, y, pressure?, t?]` samples. Pressure is ignored (full) for mouse and touch.
fn parse_points(
    id: &str,
    p: Option<&Map<String, Value>>,
    with_pressure: bool,
) -> Result<Vec<(f64, f64, f32, u32)>, EngineError> {
    let items = match p.and_then(|m| m.get("points")) {
        Some(Value::Array(a)) => a,
        None | Some(Value::Null) => return Err(bad(id, "`points` is required".into())),
        Some(_) => return Err(bad(id, "`points` must be a list of [x, y] samples".into())),
    };
    if items.len() > MAX_POINTS {
        return Err(InkError::TooManyPoints {
            count: items.len(),
            max: MAX_POINTS,
        }
        .into());
    }
    if items.is_empty() {
        return Err(InkError::Empty.into());
    }
    items
        .iter()
        .map(|v| {
            let nums = v
                .as_array()
                .filter(|a| (2..=4).contains(&a.len()))
                .ok_or_else(|| {
                    bad(
                        id,
                        "each sample is [x, y], [x, y, pressure] or [x, y, pressure, t]".into(),
                    )
                })?;
            let num = |i: usize| -> Result<Option<f64>, EngineError> {
                match nums.get(i) {
                    None | Some(Value::Null) => Ok(None),
                    Some(n) => n
                        .as_f64()
                        .filter(|x| x.is_finite())
                        .map(Some)
                        .ok_or_else(|| bad(id, "sample values must be finite numbers".into())),
                }
            };
            let (x, y) = (num(0)?.unwrap_or(0.0), num(1)?.unwrap_or(0.0));
            let pressure = if with_pressure {
                num(2)?.unwrap_or(1.0).clamp(0.0, 1.0) as f32
            } else {
                1.0
            };
            let t = num(3)?.unwrap_or(0.0).clamp(0.0, f64::from(u32::MAX)) as u32;
            Ok((x, y, pressure, t))
        })
        .collect()
}

/// What a gesture changed, as `annotate.stroke` returns it.
struct Done {
    label: &'static str,
    tx: Transaction,
    result: Value,
}

/// Applies the active tool to one gesture.
fn stroke(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.stroke";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["points", "space", "page", "device"])?;
    let device_id = opt_str(ID, p, "device")?.unwrap_or("pen");
    let device = PointerKind::from_id(device_id).ok_or_else(|| {
        bad(
            ID,
            format!(
                "`device` must be pen, eraser, mouse or touch, got {}",
                truncate(device_id, 32)
            ),
        )
    })?;
    let space = opt_str(ID, p, "space")?.unwrap_or("view");
    if !matches!(space, "view" | "page") {
        return Err(bad(
            ID,
            format!("`space` must be view or page, got {}", truncate(space, 32)),
        ));
    }
    let mut raw = parse_points(ID, p, device.is_pen())?;
    let page_param = opt_int(ID, p, "page", 1, u64::from(u32::MAX))?;
    if device == PointerKind::Touch && session.markup.mode && session.markup.touch_lock {
        // Touch lock: every touch is ignored in markup mode.
        return Ok(json!({ "drawn": false, "reason": "touchLock" }));
    }
    if !device.is_pen() && session.markup.pen_only {
        // Only a pen marks up (ANNO-14); the shell treats this gesture as a turn or pan.
        return Ok(json!({ "drawn": false, "reason": "penOnly" }));
    }
    // A stroke drawn on screen that starts at a ruler edge is ruled along it (ANNO-17). The ruler
    // lies on the screen, so only view-space strokes meet it; erasers are never ruled.
    let erasing = device == PointerKind::Eraser || session.markup.tool == MarkupTool::Eraser;
    let mut ruled = false;
    if space == "view"
        && !erasing
        && let Some(y) = session.markup.ruler
    {
        let pts: Vec<InkPoint> = raw
            .iter()
            .map(|r| InkPoint {
                x: r.0 as f32,
                y: r.1 as f32,
                pressure: r.2,
                tilt: 0.0,
                twist: 0.0,
                t: r.3,
            })
            .collect();
        if let Some(along) = crate::markup::rule_on_screen(y, &pts) {
            raw = along
                .iter()
                .map(|q| (f64::from(q.x), f64::from(q.y), q.pressure, q.t))
                .collect();
            ruled = true;
        }
    }
    // Where on which page: map the samples into the page's PDF space.
    session.refresh_reader();
    let open = session
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let score_id = open
        .library_score
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let (page, file_page, pts): (usize, usize, Vec<Point>) = if space == "view" {
        if page_param.is_some() {
            return Err(bad(ID, "`page` goes with `space: page` only".into()));
        }
        let (x0, y0) = raw.first().map(|s| (s.0, s.1)).unwrap_or((0.0, 0.0));
        let h = hit(&open.reader, open.view.document(), x0, y0)
            .ok_or_else(|| bad(ID, "the gesture doesn't start on a page".into()))?;
        let pts = raw
            .iter()
            .map(|s| {
                h.to_page(s.0, s.1)
                    .unwrap_or(Point::new(f64::NAN, f64::NAN))
            })
            .collect();
        (h.page, h.file_page, pts)
    } else {
        let n =
            page_param.ok_or_else(|| bad(ID, "`page` is required with `space: page`".into()))?;
        let index = usize::try_from(n - 1).map_err(|_| bad(ID, "`page` is too large".into()))?;
        let shown = open.reader.shown.get(index).ok_or_else(|| {
            bad(
                ID,
                format!("`page` must be from 1 to {}", open.reader.shown.len()),
            )
        })?;
        (
            index,
            shown.file_page,
            raw.iter().map(|s| Point::new(s.0, s.1)).collect(),
        )
    };
    let key =
        PageKey(u32::try_from(file_page).map_err(|_| bad(ID, "the page is out of range".into()))?);
    let now = session.now_ms();
    let markup = session.markup.clone();
    let l = session.lib_mut(ID)?;
    let score = l
        .lib
        .scores
        .get(&score_id)
        .ok_or(OpError::NoScore(score_id))?;
    let done = if erasing {
        // The stylus's eraser end always erases whole strokes (ANNO-11).
        let mode = if device == PointerKind::Eraser {
            EraserMode::Stroke
        } else {
            markup.eraser
        };
        let radius = f64::from(markup.eraser_width) / 2.0;
        let finite: Vec<Point> = pts.into_iter().filter(|p| p.is_finite()).collect();
        erase_gesture(
            score,
            key,
            &finite,
            radius,
            mode,
            &markup.curve,
            &mut || {
                l.hlc = l.hlc.tick(now);
                (l.ids.id(now), l.hlc)
            },
        )?
    } else {
        let (tool, style) = markup.drawing().ok_or_else(|| {
            bad(
                ID,
                format!("the {} tool doesn't draw strokes", markup.tool.id()),
            )
        })?;
        let layer = markup
            .layer(score)
            .ok_or_else(|| OpError::Missing("layer".into()))?;
        writable(score, layer)?;
        let samples: Vec<InkPoint> = raw
            .iter()
            .zip(&pts)
            .map(|(r, q)| InkPoint {
                x: q.x as f32,
                y: q.y as f32,
                pressure: r.2,
                tilt: 0.0,
                twist: 0.0,
                t: r.3,
            })
            .collect();
        let mut points = scorecraft_ink::prepare(&samples)?;
        // Shapes (ANNO-16): a ruled stroke is already straight.
        let mut shape = None;
        if markup.shapes
            && !ruled
            && let Some((found, straight)) = scorecraft_ink::straighten(&points)
        {
            points = straight;
            shape = Some(found.id());
        }
        let count = points.len();
        let id: AnnotationId = l.ids.id(now);
        l.hlc = l.hlc.tick(now);
        let stamp = l.hlc;
        let index = score.annotations_on(key).len();
        let annotation = Annotation {
            id,
            layer,
            stamp,
            kind: AnnotationKind::Stroke(Stroke {
                tool,
                color: style.color,
                width: style.width,
                nib: markup.nib(tool),
                points,
            }),
        };
        Done {
            label: match tool {
                Tool::Pen => "Draw",
                Tool::Highlighter => "Highlight",
                Tool::Whiteout => "Whiteout",
            },
            tx: Transaction::single(Op::InsertAnnotation {
                score: score_id,
                page: key,
                index,
                annotation: Box::new(annotation),
            }),
            result: json!({
                "tool": match tool {
                    Tool::Pen => "pen",
                    Tool::Highlighter => "highlighter",
                    Tool::Whiteout => "whiteout",
                },
                "annotation": id.to_string(),
                "points": count,
                "shape": shape,
            }),
        }
    };
    let changed = !done.tx.is_empty();
    if changed {
        l.commit(done.label, done.tx)?;
    }
    let mut result = done.result;
    crate::commands::set(&mut result, "drawn", json!(changed));
    crate::commands::set(&mut result, "page", json!(page + 1));
    crate::commands::set(&mut result, "filePage", json!(file_page + 1));
    Ok(result)
}

/// Refuses to change a hidden or locked layer (spec ANNO-10, INK-10).
pub(crate) fn writable(score: &Score, layer: scorecraft_doc::LayerId) -> Result<(), EngineError> {
    let l: &Layer = score
        .layer(layer)
        .ok_or_else(|| OpError::Missing(format!("layer {layer}")))?;
    if l.locked {
        return Err(OpError::Invalid(format!(
            "layer “{}” is locked; unlock it to draw on it",
            truncate(&l.name, 64)
        ))
        .into());
    }
    if !l.visible {
        return Err(OpError::Invalid(format!(
            "layer “{}” is hidden; show it to draw on it",
            truncate(&l.name, 64)
        ))
        .into());
    }
    Ok(())
}

/// One eraser gesture on one page: every touched stroke on a visible, unlocked layer.
fn erase_gesture(
    score: &Score,
    page: PageKey,
    path: &[Point],
    radius: f64,
    mode: EraserMode,
    curve: &scorecraft_ink::PressureCurve,
    next: &mut dyn FnMut() -> (AnnotationId, scorecraft_doc::Hlc),
) -> Result<Done, EngineError> {
    let open_layer = |id| {
        score
            .layer(id)
            .is_some_and(|l: &Layer| l.visible && !l.locked)
    };
    let list = score.annotations_on(page);
    let mut ops = Vec::new();
    let (mut removed, mut added) = (0usize, 0usize);
    // From the top of the list down, so inserting pieces never shifts an index still to come.
    for (index, a) in list.iter().enumerate().rev() {
        let AnnotationKind::Stroke(s) = &a.kind else {
            continue;
        };
        if !open_layer(a.layer) {
            continue;
        }
        let remove = Op::RemoveAnnotation {
            score: score.id,
            page,
            annotation: a.id,
        };
        match mode {
            EraserMode::Stroke => {
                if touches(s, path, radius, curve) {
                    ops.push(remove);
                    removed += 1;
                }
            }
            EraserMode::Pixel => match erase(s, path, radius, curve) {
                Erased::Untouched => {}
                Erased::Removed => {
                    ops.push(remove);
                    removed += 1;
                }
                Erased::Split(pieces) => {
                    ops.push(remove);
                    removed += 1;
                    for (k, piece) in pieces.into_iter().enumerate() {
                        let (id, stamp) = next();
                        ops.push(Op::InsertAnnotation {
                            score: score.id,
                            page,
                            index: index + k,
                            annotation: Box::new(Annotation {
                                id,
                                layer: a.layer,
                                stamp,
                                kind: AnnotationKind::Stroke(piece),
                            }),
                        });
                        added += 1;
                    }
                }
            },
        }
    }
    Ok(Done {
        label: "Erase",
        tx: Transaction::new(ops),
        result: json!({
            "tool": "eraser",
            "mode": mode.id(),
            "erased": removed,
            "added": added,
        }),
    })
}

/// Every marking on a score page, for agents and tests.
fn page_list(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "annotate.page.list";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["page"])?;
    session.refresh_reader();
    let open = session
        .score
        .as_ref()
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let count = open.reader.shown.len();
    let index = match opt_int(ID, p, "page", 1, (count as u64).max(1))? {
        Some(n) => usize::try_from(n - 1).map_err(|_| bad(ID, "`page` is too large".into()))?,
        None => open.reader.reader.page(),
    };
    let shown = open
        .reader
        .shown
        .get(index)
        .ok_or_else(|| bad(ID, "the score shows no pages".into()))?;
    let file_page = shown.file_page;
    let score_id: Option<ScoreId> = open.library_score;
    let score = library_score(session.library(), score_id)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let key = PageKey(u32::try_from(file_page).unwrap_or(u32::MAX));
    let annotations = serde_json::to_value(score.annotations_on(key))
        .map_err(|e| EngineError::Io(format!("listing markings: {e}")))?;
    Ok(json!({
        "page": index + 1,
        "filePage": file_page + 1,
        "annotations": annotations,
    }))
}

#[cfg(test)]
mod tests;
