//! Export with ink (feature-map EXP-2, spec `docs/spec/ink.md`): the open library score as a new
//! PDF, its pages in the score's order with its rotation, crop, hidden pages and page range
//! applied, and the chosen layers' markings flattened or as PDF annotations. The imported file
//! is never changed (ANNO-15).

use std::sync::Arc;

use serde_json::{Value, json};

use scorecraft_doc::{AnnotationKind, LayerId, PageKey, Score};
use scorecraft_geom::Point;
use scorecraft_ink::PressureCurve;
use scorecraft_pdf::{ExportMark, ExportMode, ExportPage, InkFill, MarkKind, export_pdf};
use scorecraft_render::Filter;
use scorecraft_render::ink::{Fill, fills, hairpin_lines, is_drawn};

use crate::annotate_cmds::markup_ready;
use crate::commands::{
    CommandSpec, bad, opt_str, params_object, reject_unknown_keys, req_str, truncate,
};
use crate::text_cmds::library_score;
use crate::{EngineError, Session};

/// The most layers one export names.
const MAX_LAYERS: usize = 256;

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "score.export.pdf",
        label: "Export PDF…",
        menu: "File>Export",
        shortcut: None,
        params: r#"{"path":path,"mode":"flatten|annotations"="flatten","layers":[id]?,"password":string?}"#,
        ours: false,
        enabled: markup_ready,
        run: export,
    }]
}

fn to_fill(f: Fill) -> InkFill {
    InkFill {
        polygons: f
            .polygons
            .into_iter()
            .map(|p| p.into_iter().map(|q| [q.x, q.y]).collect())
            .collect(),
        rgb: [f.color.r, f.color.g, f.color.b],
        alpha: f64::from(f.alpha),
        multiply: f.multiply,
    }
}

fn path(points: impl IntoIterator<Item = Point>) -> Vec<[f64; 2]> {
    points.into_iter().map(|q| [q.x, q.y]).collect()
}

/// One marking as the export writes it: its fills (the CPU renderer's geometry, on white paper)
/// and what kind of annotation it becomes.
fn export_mark(kind: &AnnotationKind, curve: &PressureCurve) -> Option<ExportMark> {
    let filled: Vec<InkFill> = fills(kind, Filter::Normal, curve)
        .into_iter()
        .map(to_fill)
        .collect();
    let (kind, rgb) = match kind {
        AnnotationKind::Stroke(s) => (
            MarkKind::Ink {
                paths: vec![path(
                    s.points
                        .iter()
                        .map(|q| Point::new(f64::from(q.x), f64::from(q.y))),
                )],
                width: f64::from(s.width),
            },
            s.color,
        ),
        AnnotationKind::Hairpin(h) => (
            MarkKind::Ink {
                paths: hairpin_lines(h)
                    .map(|ls| ls.iter().map(|l| path(*l)).collect())
                    .unwrap_or_default(),
                width: f64::from(h.width),
            },
            h.color,
        ),
        AnnotationKind::Text(t) => (
            MarkKind::Text {
                text: t.text.clone(),
                size: f64::from(t.style.size),
            },
            t.style.color,
        ),
        AnnotationKind::Stamp(st) => (
            MarkKind::Stamp {
                text: st.text.clone(),
            },
            st.color,
        ),
        AnnotationKind::Snip(sn) => {
            let r = |x: scorecraft_geom::Rect| [x.x0, x.y0, x.x1, x.y1];
            return Some(ExportMark {
                kind: MarkKind::Snip {
                    page: usize::try_from(sn.from.page.0).ok()?,
                    from: r(sn.from.rect),
                    to: r(sn.to.rect),
                },
                fills: Vec::new(),
                rgb: [0, 0, 0],
            });
        }
        AnnotationKind::Shortcut(_) => return None,
    };
    if filled.is_empty() {
        return None;
    }
    Some(ExportMark {
        kind,
        fills: filled,
        rgb: [rgb.r, rgb.g, rgb.b],
    })
}

/// The pages of `score` to export with the markings of `layers` (bottom layer first, each in
/// the order its markings were made).
fn pages(score: &Score, layers: &[LayerId], curve: &PressureCurve) -> Vec<ExportPage> {
    score
        .visible_pages()
        .into_iter()
        .filter_map(|i| {
            let p = score.pages.get(i)?;
            let file_page = usize::try_from(p.source.0).ok()?;
            let list = score.annotations_on(PageKey(p.source.0));
            let mut marks = Vec::new();
            for layer in score.layers.iter().filter(|l| layers.contains(&l.id)) {
                for a in list
                    .iter()
                    .filter(|a| a.layer == layer.id && is_drawn(&a.kind))
                {
                    marks.extend(export_mark(&a.kind, curve));
                }
            }
            Some(ExportPage {
                file_page,
                rotate: i64::from(p.rotation.quarters()) * 90,
                crop: p.crop.map(|c| [c.x0, c.y0, c.x1, c.y1]),
                marks,
            })
        })
        .collect()
}

fn export(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.export.pdf";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path", "mode", "layers", "password"])?;
    let out_path = req_str(ID, p, "path")?;
    let mode = match opt_str(ID, p, "mode")?.unwrap_or("flatten") {
        "flatten" => ExportMode::Flatten,
        "annotations" => ExportMode::Annotations,
        other => {
            return Err(bad(
                ID,
                format!(
                    "`mode` must be flatten or annotations, got {}",
                    truncate(other, 32)
                ),
            ));
        }
    };
    let password = opt_str(ID, p, "password")?;
    let score_id = session
        .score
        .as_ref()
        .and_then(|o| o.library_score)
        .ok_or_else(|| EngineError::Disabled(ID.to_owned()))?;
    let score = library_score(session, score_id)?.clone();
    let layers: Vec<LayerId> = match p.and_then(|m| m.get("layers")) {
        None | Some(Value::Null) => score
            .layers
            .iter()
            .filter(|l| l.visible)
            .map(|l| l.id)
            .collect(),
        Some(Value::Array(a)) => {
            if a.len() > MAX_LAYERS {
                return Err(bad(ID, format!("at most {MAX_LAYERS} layers")));
            }
            a.iter()
                .map(|v| {
                    let id: LayerId = v
                        .as_str()
                        .ok_or_else(|| bad(ID, "`layers` holds layer ids".into()))?
                        .parse()
                        .map_err(|e: String| bad(ID, format!("`layers`: {e}")))?;
                    score
                        .layer(id)
                        .map(|_| id)
                        .ok_or_else(|| bad(ID, format!("the score has no layer {id}")))
                })
                .collect::<Result<_, _>>()?
        }
        Some(_) => return Err(bad(ID, "`layers` must be a list of layer ids".into())),
    };
    let curve = session.markup.curve;
    let list = pages(&score, &layers, &curve);
    let marks: usize = list.iter().map(|p| p.marks.len()).sum();
    let bytes = {
        let l = session.lib_mut(ID)?;
        Arc::new(l.store.blob(score.source.blob)?)
    };
    let pdf = export_pdf(bytes, password, &list, mode)?;
    write_atomic(out_path, &pdf)?;
    Ok(json!({
        "path": truncate(out_path, 256),
        "pages": list.len(),
        "markings": marks,
        "mode": match mode {
            ExportMode::Flatten => "flatten",
            ExportMode::Annotations => "annotations",
        },
        "bytes": pdf.len(),
    }))
}

/// Writes beside `path`, then renames: an interrupted export never leaves half a file.
fn write_atomic(path: &str, data: &[u8]) -> Result<(), EngineError> {
    let shown = truncate(path, 256);
    if path.trim().is_empty() {
        return Err(EngineError::Io("the export needs a file path".into()));
    }
    let target = std::path::Path::new(path);
    if target.is_dir() {
        return Err(EngineError::Io(format!("{shown} is a folder")));
    }
    let tmp = target.with_extension("pdf.part");
    std::fs::write(&tmp, data).map_err(|e| EngineError::Io(format!("{shown}: {e}")))?;
    std::fs::rename(&tmp, target).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        EngineError::Io(format!("{shown}: {e}"))
    })
}

#[cfg(test)]
mod tests;
