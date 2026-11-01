//! Covers for the library window (feature-map LIB-14, spec `docs/spec/library.md`): each score's
//! first shown page, small. [`Session::cover_job`] takes what a cover needs out of the library,
//! and [`CoverJob::render`] draws it with no session, so the shell can render covers on a worker
//! thread while the grid scrolls.

use std::sync::Arc;

use base64::Engine as _;
use serde_json::{Value, json};

use scorecraft_doc::ScoreId;
use scorecraft_geom::Rotation;
use scorecraft_pdf::{Bitmap, Document};

use crate::commands::{CommandSpec, opt_int, params_object, reject_unknown_keys, req_id};
use crate::library::library_open;
use crate::{EngineError, Session};

/// Cover sizes: the longest side in pixels.
pub const MIN_COVER: u32 = 32;
pub const MAX_COVER: u32 = 1024;

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "library.cover",
        label: "Cover",
        menu: "",
        shortcut: None,
        params: r#"{"score":id,"size":32..1024=256}"#,
        ours: true,
        enabled: library_open,
        run: cover,
    }]
}

/// What a cover needs: the score's file and its first shown page.
#[derive(Clone)]
pub struct CoverJob {
    pub score: ScoreId,
    bytes: Arc<Vec<u8>>,
    file_page: usize,
    rotation: Rotation,
}

impl std::fmt::Debug for CoverJob {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CoverJob")
            .field("score", &self.score)
            .field("file_page", &self.file_page)
            .finish_non_exhaustive()
    }
}

impl Session {
    /// The cover job for library score `score` (an id as text).
    pub fn cover_job(&mut self, score: &str) -> Result<CoverJob, EngineError> {
        const ID: &str = "library.cover";
        let id: ScoreId = score
            .parse()
            .map_err(|e: String| crate::commands::bad(ID, format!("`score`: {e}")))?;
        let l = self.lib_mut(ID)?;
        let s = l
            .lib
            .scores
            .get(&id)
            .ok_or(scorecraft_ops::OpError::NoScore(id))?;
        let first = s
            .visible_pages()
            .first()
            .and_then(|i| s.pages.get(*i))
            .map(|p| (p.source.0, p.rotation))
            .unwrap_or((0, Rotation::R0));
        let blob = s.source.blob;
        let bytes = Arc::new(l.store.blob(blob)?);
        Ok(CoverJob {
            score: id,
            bytes,
            file_page: usize::try_from(first.0).unwrap_or(0),
            rotation: first.1,
        })
    }
}

impl CoverJob {
    /// The cover with its longest side `max_side` pixels (clamped to 32–1024), turned as the
    /// score turns the page.
    pub fn render(&self, max_side: u32) -> Result<Bitmap, EngineError> {
        let max_side = max_side.clamp(MIN_COVER, MAX_COVER);
        let mut doc = Document::open(self.bytes.clone())?;
        let g = doc.page(self.file_page)?;
        let (w, h) = (
            f64::from(g.crop[2] - g.crop[0]).abs(),
            f64::from(g.crop[3] - g.crop[1]).abs(),
        );
        let (w, h) = if g.rotation % 180 == 90 {
            (h, w)
        } else {
            (w, h)
        };
        let longest = w.max(h).max(1.0);
        let scale = (f64::from(max_side) / longest) as f32;
        let bm = doc.render(self.file_page, scale)?;
        Ok(turn(bm, self.rotation))
    }
}

/// `bm` turned clockwise by `r`.
fn turn(bm: Bitmap, r: Rotation) -> Bitmap {
    let (w, h) = (bm.width, bm.height);
    let at = |x: u32, y: u32| {
        bm.words
            .get((y as usize) * (w as usize) + x as usize)
            .copied()
            .unwrap_or(0)
    };
    let (nw, nh) = match r {
        Rotation::R0 => return bm,
        Rotation::R180 => (w, h),
        Rotation::R90 | Rotation::R270 => (h, w),
    };
    let mut words = Vec::with_capacity((nw as usize) * (nh as usize));
    for y in 0..nh {
        for x in 0..nw {
            words.push(match r {
                Rotation::R90 => at(y, h - 1 - x),
                Rotation::R180 => at(w - 1 - x, h - 1 - y),
                _ => at(w - 1 - y, x),
            });
        }
    }
    Bitmap {
        width: nw,
        height: nh,
        words,
    }
}

fn cover(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "library.cover";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["score", "size"])?;
    let id: ScoreId = req_id(ID, p, "score")?;
    let size = opt_int(ID, p, "size", u64::from(MIN_COVER), u64::from(MAX_COVER))?.unwrap_or(256);
    let job = session.cover_job(&id.to_string())?;
    let bm = job.render(u32::try_from(size).unwrap_or(256))?;
    let png = scorecraft_codecs::png::encode_premultiplied(bm.width, bm.height, bm.bytes())
        .map_err(EngineError::Io)?;
    Ok(json!({
        "score": id.to_string(),
        "width": bm.width,
        "height": bm.height,
        "png": base64::engine::general_purpose::STANDARD.encode(&png),
    }))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]
    use super::*;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

    #[test]
    fn libs_9_covers_are_small_first_pages_turned_like_the_score() {
        let dir = std::env::temp_dir().join(format!("scorecraft-cover-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("c.pdf");
        std::fs::write(
            &path,
            PdfBuilder::new().pages(2, PageSpec::default()).build(),
        )
        .unwrap();
        let mut s = Session::new();
        s.create_memory_library().unwrap();
        let r = s
            .dispatch(
                "library.import.files",
                &json!({ "paths": [path.to_string_lossy()] }),
            )
            .unwrap();
        let id = r["imported"][0]["score"].as_str().unwrap().to_owned();
        let job = s.cover_job(&id).unwrap();
        let bm = job.render(200).unwrap();
        assert_eq!(bm.height, 200, "portrait: the height is the longest side");
        assert!(bm.width < 200 && bm.width > 100);
        s.dispatch(
            "page.rotate",
            &json!({ "score": id, "page": 1, "degrees": 90 }),
        )
        .unwrap();
        let turned = s.cover_job(&id).unwrap().render(200).unwrap();
        assert_eq!((turned.width, turned.height), (bm.height, bm.width));
        let v = s
            .dispatch("library.cover", &json!({ "score": id, "size": 64 }))
            .unwrap();
        assert_eq!(
            v["height"]
                .as_u64()
                .unwrap()
                .max(v["width"].as_u64().unwrap()),
            64
        );
        assert!(s.cover_job("nope").is_err());
        assert!(
            s.dispatch("library.cover", &json!({ "score": id, "size": 5000 }))
                .is_err()
        );
    }
}
