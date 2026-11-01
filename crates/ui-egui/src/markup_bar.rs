//! The markup toolbar (spec `docs/spec/ink.md`, "The markup toolbar"): *Draw with Touch*, touch
//! lock, the tools, the eraser's modes, the stamps, eight swatches, the palette, the width
//! slider, the selection's actions, undo and the markup settings. It reads `annotate.state` and
//! returns the commands to run; it changes nothing itself.

use egui::{Color32, Rect, Sense, Stroke, Ui, vec2};
use serde_json::{Value, json};

use crate::ink::{InkCanvas, parse_color};
use crate::theme::Tokens;

/// A command the bar asks the shell to run.
pub type Run = Vec<(&'static str, Value)>;

/// Shell-side markup settings the bar changes.
pub struct BarSettings<'a> {
    pub show_ruler_button: &'a mut bool,
    /// Set when a shell setting changed and should be saved.
    pub changed: &'a mut bool,
}

fn s<'a>(state: &'a Value, path: &str) -> &'a str {
    state.pointer(path).and_then(Value::as_str).unwrap_or("")
}

fn b(state: &Value, path: &str) -> bool {
    state
        .pointer(path)
        .and_then(Value::as_bool)
        .unwrap_or(false)
}

fn swatch(ui: &mut Ui, color: Color32, selected: bool, size: f32) -> bool {
    let (rect, r) = ui.allocate_exact_size(vec2(size, size), Sense::click());
    let p = ui.painter();
    p.rect_filled(rect.shrink(3.0), 3.0, color);
    p.rect_stroke(
        rect.shrink(3.0),
        3.0,
        Stroke::new(1.0, Color32::from_gray(120)),
        egui::StrokeKind::Middle,
    );
    if selected {
        p.rect_stroke(
            rect,
            4.0,
            Stroke::new(2.0, Color32::from_rgb(0x30, 0x70, 0xe0)),
            egui::StrokeKind::Inside,
        );
    }
    r.clicked()
}

/// The eraser command for its current mode.
fn eraser_cmd(state: &Value) -> &'static str {
    if s(state, "/eraser/mode") == "pixel" {
        "annotate.tool.pixelEraser"
    } else {
        "annotate.tool.strokeEraser"
    }
}

/// The width slider's range and value for the active tool, and the label it shows.
fn slider(state: &Value) -> Option<(f64, f64, f64, &'static str)> {
    let range = |p: &str| {
        let r = state.pointer(p)?.as_array()?;
        Some((r.first()?.as_f64()?, r.get(1)?.as_f64()?))
    };
    let num = |p: &str| state.pointer(p).and_then(Value::as_f64);
    let (lo, hi, v, label) = match s(state, "/tool") {
        "pen" if b(state, "/whiteoutOn") => {
            let (lo, hi) = range("/whiteout/widths")?;
            (lo, hi, num("/whiteout/width")?, "Width")
        }
        "pen" => {
            let (lo, hi) = range("/pen/widths")?;
            (lo, hi, num("/pen/width")?, "Width")
        }
        "highlighter" => {
            let (lo, hi) = range("/highlighter/widths")?;
            (lo, hi, num("/highlighter/width")?, "Width")
        }
        "eraser" => {
            let (lo, hi) = range("/eraser/widths")?;
            (lo, hi, num("/eraser/width")?, "Size")
        }
        "text" => {
            let (lo, hi) = range("/text/sizes")?;
            (lo, hi, num("/text/size")?, "Text size")
        }
        "stamp" => {
            let (lo, hi) = range("/stamp/sizes")?;
            (lo, hi, num("/stamp/size")?, "Size")
        }
        _ => return None,
    };
    Some((lo, hi, v, label))
}

/// The bar, one row. `score` is the open library score, for undo.
pub fn bar(
    ui: &mut Ui,
    tokens: &Tokens,
    state: &Value,
    score: Option<&str>,
    ink: &mut InkCanvas,
    settings: BarSettings<'_>,
    run: &mut Run,
) {
    let tool = s(state, "/tool").to_owned();
    let size = tokens.button * 0.8;
    ui.horizontal_centered(|ui| {
        let touch = b(state, "/drawWithTouch");
        let label = s(state, "/touchLabel");
        if ui
            .selectable_label(
                touch,
                if label.is_empty() {
                    "Draw with Touch"
                } else {
                    label
                },
            )
            .on_hover_text("Let the mouse and touch draw too")
            .clicked()
        {
            run.push(("input.pen.only", json!({ "on": touch })));
        }
        let lock = b(state, "/touchLock");
        if ui
            .selectable_label(lock, "Touch Lock")
            .on_hover_text("Ignore every touch on the page")
            .clicked()
        {
            run.push(("input.touch.lock", json!({ "on": !lock })));
        }
        ui.separator();
        for (id, label, cmd) in [
            ("pen", "Pen", "annotate.tool.pen"),
            ("highlighter", "Highlighter", "annotate.tool.highlighter"),
            ("eraser", "Eraser", eraser_cmd(state)),
            ("text", "Text", "annotate.tool.text"),
            ("stamp", "Stamp", "annotate.tool.stamp"),
            ("select", "Select", "annotate.tool.select"),
        ] {
            if ui.selectable_label(tool == id, label).clicked() {
                run.push((cmd, json!({})));
            }
            if id == "eraser" && tool == "eraser" {
                let mode = s(state, "/eraser/mode");
                if ui.selectable_label(mode == "stroke", "Stroke").clicked() {
                    run.push(("annotate.tool.strokeEraser", json!({})));
                }
                if ui.selectable_label(mode == "pixel", "Pixel").clicked() {
                    run.push(("annotate.tool.pixelEraser", json!({})));
                }
            }
        }
        if *settings.show_ruler_button {
            let on = state.get("ruler").is_some_and(|r| !r.is_null());
            if ui.selectable_label(on, "Ruler").clicked() {
                run.push(("annotate.ruler.show", json!({ "on": !on })));
            }
        }
        ui.separator();
        if tool == "stamp" {
            let chosen = s(state, "/stamp/stamp");
            for (id, label) in [
                ("pp", "pp"),
                ("p", "p"),
                ("mp", "mp"),
                ("mf", "mf"),
                ("f", "f"),
                ("ff", "ff"),
                ("crescendo", "<"),
                ("diminuendo", ">"),
            ] {
                let text = egui::RichText::new(label).italics().strong();
                if ui.selectable_label(chosen == id, text).clicked() {
                    run.push(("annotate.tool.stamp", json!({ "stamp": id })));
                }
            }
        } else if let Some(swatches) = state.get("swatches").and_then(Value::as_array) {
            let selected = state.get("swatch").and_then(Value::as_u64).unwrap_or(0);
            for (i, c) in swatches.iter().enumerate() {
                let color = c.as_str().and_then(parse_color).unwrap_or(Color32::BLACK);
                if swatch(ui, color, selected == i as u64 + 1, size) {
                    run.push(("annotate.swatch.select", json!({ "index": i + 1 })));
                }
            }
            if let Some(palette) = state.get("palette").and_then(Value::as_array) {
                ui.menu_button("Palette", |ui| {
                    ui.label("Replaces the selected swatch");
                    egui::Grid::new("palette")
                        .spacing([2.0, 2.0])
                        .show(ui, |ui| {
                            for (i, c) in palette.iter().enumerate() {
                                let hex = c.as_str().unwrap_or("#000000");
                                let color = parse_color(hex).unwrap_or(Color32::BLACK);
                                if swatch(ui, color, false, size) {
                                    run.push(("annotate.swatch.set", json!({ "color": hex })));
                                    ui.close();
                                }
                                if i % 6 == 5 {
                                    ui.end_row();
                                }
                            }
                        });
                });
            }
        }
        if let Some((lo, hi, v, label)) = slider(state) {
            let mut w = v;
            let r = ui.add(egui::Slider::new(&mut w, lo..=hi).text(label));
            if r.changed() && (w - v).abs() > 1e-9 {
                run.push(("annotate.style.set", json!({ "width": w })));
            }
        }
        let selected = state
            .pointer("/selection/annotations")
            .and_then(Value::as_array)
            .is_some_and(|a| !a.is_empty());
        if tool == "select" && selected && ink.selection.is_some() {
            ui.separator();
            if ui
                .selectable_label(ink.copy_next, "Copy")
                .on_hover_text("The next drag copies the selection")
                .clicked()
            {
                ink.copy_next = !ink.copy_next;
            }
            if ui.button("Smaller").clicked() {
                run.push(("annotate.select.scale", json!({ "factor": 0.8 })));
                ink.selection = ink.selection.map(|r| scale(r, 0.8));
            }
            if ui.button("Larger").clicked() {
                run.push(("annotate.select.scale", json!({ "factor": 1.25 })));
                ink.selection = ink.selection.map(|r| scale(r, 1.25));
            }
            if ui
                .button("Recolour")
                .on_hover_text("In the pen's colour")
                .clicked()
            {
                let c: String = s(state, "/pen/color").chars().take(7).collect();
                run.push(("annotate.select.recolor", json!({ "color": c })));
            }
            if ui.button("Delete").clicked() {
                run.push(("annotate.select.delete", json!({})));
                ink.selection = None;
            }
        }
        if tool == "select"
            && let Some(sel) = ink.selection
        {
            ui.separator();
            if ink.shortcut_target == 0 {
                ink.shortcut_target = 1;
            }
            ui.add(
                egui::DragValue::new(&mut ink.shortcut_target)
                    .range(1..=9999)
                    .prefix("to page "),
            );
            if ui
                .button("Shortcut")
                .on_hover_text("Tapping this rectangle jumps to that page")
                .clicked()
            {
                run.push((
                    "nav.shortcut.add",
                    json!({
                        "x0": sel.min.x, "y0": sel.min.y, "x1": sel.max.x, "y1": sel.max.y,
                        "target": ink.shortcut_target,
                    }),
                ));
                ink.selection = None;
            }
            if ui
                .selectable_label(ink.snip_armed, "Snip")
                .on_hover_text("Tap where the copied bars go")
                .clicked()
            {
                ink.snip_armed = !ink.snip_armed;
            }
        }
        ui.separator();
        if let Some(id) = score {
            if ui.button("Undo").clicked() {
                run.push(("edit.undo", json!({ "score": id })));
            }
            if ui.button("Redo").clicked() {
                run.push(("edit.redo", json!({ "score": id })));
            }
        }
        ui.menu_button("Settings", |ui| {
            if ui
                .checkbox(settings.show_ruler_button, "Show ruler button")
                .changed()
            {
                *settings.changed = true;
            }
            let mut round = b(state, "/highlighterRoundEnds");
            if ui.checkbox(&mut round, "Round highlighter ends").changed() {
                run.push(("annotate.highlighter.roundEnds", json!({ "on": round })));
            }
            let mut shapes = b(state, "/shapes");
            if ui
                .checkbox(&mut shapes, "Shapes: straighten lines and circles")
                .changed()
            {
                run.push(("annotate.shapes", json!({ "on": shapes })));
            }
            ui.label("Pen button switches to");
            let current = s(state, "/penButton").to_owned();
            for (id, label) in [
                ("eraser", "Eraser"),
                ("highlighter", "Highlighter"),
                ("pen", "Pen"),
                ("text", "Text"),
                ("select", "Select"),
            ] {
                if ui.radio(current == id, label).clicked() {
                    run.push(("annotate.pen.buttonTool", json!({ "tool": id })));
                }
            }
        });
    });
}

fn scale(r: Rect, f: f32) -> Rect {
    Rect::from_center_size(r.center(), r.size() * f)
}
