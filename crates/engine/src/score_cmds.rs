//! Opening a score (feature-map READ-1). One score is open at a time until the library lands.

use std::io::Read;
use std::sync::Arc;

use scorecraft_pdf::{Document, OutlineEntry};
use scorecraft_render::ScoreView;
use serde_json::{Value, json};

use crate::commands::{CommandSpec, always, opt_str, params_object, reject_unknown_keys};
use crate::{EngineError, Session};

/// The largest score file `score.open` reads.
pub const MAX_SCORE_BYTES: u64 = 1024 * 1024 * 1024;

/// Bitmap budget for the open score's page cache.
const CACHE_BUDGET: usize = scorecraft_render::DEFAULT_BUDGET;

/// An open score and where the reader is in it.
pub struct OpenScore {
    pub(crate) view: ScoreView,
    /// The file name shown in titles and errors.
    pub(crate) name: String,
    /// Which pages are shown, where the player is, layout and zoom.
    pub(crate) reader: crate::reader::ReaderState,
    /// The library score it was opened from, if any.
    pub(crate) library_score: Option<scorecraft_doc::ScoreId>,
}

pub fn specs() -> Vec<CommandSpec> {
    vec![CommandSpec {
        id: "score.open",
        label: "Open Score…",
        menu: "File",
        shortcut: Some("Ctrl+O"),
        params: r#"{"path":path?,"score":id?,"password":string?}"#,
        ours: false,
        enabled: always,
        run: open,
    }]
}

fn open(session: &mut Session, params: &Value) -> Result<Value, EngineError> {
    const ID: &str = "score.open";
    let p = params_object(ID, params)?;
    reject_unknown_keys(ID, p, &["path", "score", "password"])?;
    let password = opt_str(ID, p, "password")?;
    let score: Option<scorecraft_doc::ScoreId> = crate::commands::opt_id(ID, p, "score")?;
    match (opt_str(ID, p, "path")?, score) {
        (Some(path), None) => {
            let bytes = read_score(path)?;
            let name = std::path::Path::new(path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            session.open_score_bytes(Arc::new(bytes), password, &name)
        }
        (None, Some(id)) => session.open_library_score(id, password),
        (Some(_), Some(_)) => Err(crate::commands::bad(
            ID,
            "give `path` or `score`, not both".into(),
        )),
        (None, None) => Err(crate::commands::bad(
            ID,
            "`path` or `score` is required".into(),
        )),
    }
}

/// Reads a regular file of at most [`MAX_SCORE_BYTES`].
pub(crate) fn read_score(path: &str) -> Result<Vec<u8>, EngineError> {
    let shown = crate::commands::truncate(path, 256);
    let io = |e: std::io::Error| EngineError::Io(format!("{shown}: {e}"));
    let file = std::fs::File::open(path).map_err(io)?;
    let meta = file.metadata().map_err(io)?;
    if !meta.is_file() {
        return Err(EngineError::Io(format!("{shown}: not a regular file")));
    }
    if meta.len() > MAX_SCORE_BYTES {
        return Err(EngineError::Io(format!(
            "{shown}: larger than {MAX_SCORE_BYTES} bytes"
        )));
    }
    let mut bytes = Vec::new();
    file.take(MAX_SCORE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(io)?;
    if bytes.len() as u64 > MAX_SCORE_BYTES {
        return Err(EngineError::Io(format!(
            "{shown}: larger than {MAX_SCORE_BYTES} bytes"
        )));
    }
    Ok(bytes)
}

impl Session {
    /// Opens a score from its bytes, replacing the open one, and returns what `score.open`
    /// returns. For frontends that hold bytes rather than a path (the web build, drag and drop).
    pub fn open_score_bytes(
        &mut self,
        bytes: Arc<Vec<u8>>,
        password: Option<&str>,
        name: &str,
    ) -> Result<Value, EngineError> {
        let doc = Document::open_with_password(bytes, password)?;
        let info = score_info(&doc, name);
        #[allow(unused_mut)]
        let mut view = ScoreView::new(doc, CACHE_BUDGET);
        let reader = crate::reader::ReaderState::new(crate::reader::file_pages(view.document()));
        // Without a worker thread, pre-render requests wait for `run_prefetch`; the score still
        // opens and renders.
        #[cfg(not(target_arch = "wasm32"))]
        let _ = view.start_worker();
        // The ruler lies on a page of the score that was open.
        self.markup.ruler = None;
        self.markup.selection = None;
        self.score = Some(OpenScore {
            view,
            name: name.to_owned(),
            reader,
            library_score: None,
        });
        Ok(info)
    }

    /// Opens a library score in the reader and records when it was opened (LIB-15). The reader
    /// shows the file's pages until page layout lands (M3); page edits apply there.
    pub fn open_library_score(
        &mut self,
        id: scorecraft_doc::ScoreId,
        password: Option<&str>,
    ) -> Result<Value, EngineError> {
        const ID: &str = "score.open";
        let now = self.now_ms();
        let l = self.lib_mut(ID)?;
        let s = l
            .lib
            .scores
            .get(&id)
            .ok_or(scorecraft_ops::OpError::NoScore(id))?;
        let blob = s.source.blob;
        let name = s.display_title().to_owned();
        let bytes = l.store.blob(blob)?;
        let mut info = self.open_score_bytes(Arc::new(bytes), password, &name)?;
        if let Some(open) = self.score.as_mut() {
            open.library_score = Some(id);
        }
        self.refresh_reader();
        self.prefetch_reader();
        let l = self.lib_mut(ID)?;
        if let Some(s) = l.lib.scores.get_mut(&id) {
            s.opened_ms = Some(now);
        }
        // Not an undo step; a failed save here only loses the "recently opened" time.
        let saved = l.save_untracked(scorecraft_ops::Touched::Score(id)).is_ok();
        crate::commands::set(&mut info, "score", json!(id.to_string()));
        crate::commands::set(&mut info, "openedSaved", json!(saved));
        Ok(info)
    }

    /// Whether a score is open.
    pub fn has_score(&self) -> bool {
        self.score.is_some()
    }
}

/// Enabled while a score is open.
pub fn score_open(session: &Session) -> bool {
    session.has_score()
}

fn score_info(doc: &Document, name: &str) -> Value {
    let m = doc.metadata();
    let pages: Vec<Value> = doc
        .pages()
        .iter()
        .map(|g| {
            json!({
                "width": g.width,
                "height": g.height,
                "rotation": g.rotation,
                "label": g.label,
            })
        })
        .collect();
    json!({
        "name": name,
        "pageCount": doc.page_count(),
        "title": m.title,
        "composer": m.author,
        "subject": m.subject,
        "keywords": m.keywords,
        "creator": m.creator,
        "producer": m.producer,
        "pdfVersion": m.pdf_version,
        "encrypted": m.encrypted,
        "fileSize": m.file_size,
        "outline": doc.outline().iter().map(outline_json).collect::<Vec<_>>(),
        "pages": pages,
    })
}

/// Outline entries with 1-based page numbers, as the reader shows them.
fn outline_json(e: &OutlineEntry) -> Value {
    json!({
        "title": e.title,
        "page": e.page.map(|p| p.saturating_add(1)),
        "children": e.children.iter().map(outline_json).collect::<Vec<_>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

    fn sample(dir: &std::path::Path) -> String {
        let bytes = PdfBuilder::new()
            .title("Prelude")
            .composer("F. Chopin")
            .pages(3, PageSpec::default())
            .build();
        let path = dir.join("prelude.pdf");
        std::fs::write(&path, bytes).unwrap();
        path.to_string_lossy().into_owned()
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("scorecraft-{name}-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn opens_a_score_and_reports_it() {
        let dir = temp_dir("open");
        let mut s = Session::new();
        assert_eq!(s.is_enabled("view.render"), Some(false));
        let info = s
            .dispatch("score.open", &json!({ "path": sample(&dir) }))
            .unwrap();
        assert_eq!(info["pageCount"], 3);
        assert_eq!(info["title"], "Prelude");
        assert_eq!(info["composer"], "F. Chopin");
        assert_eq!(info["name"], "prelude.pdf");
        assert_eq!(info["pages"][0]["width"], 612.0);
        assert_eq!(info["pages"][2]["label"], "3");
        assert!(s.has_score());
        assert_eq!(s.is_enabled("view.render"), Some(true));
    }

    #[test]
    fn bad_files_are_errors() {
        let dir = temp_dir("bad");
        let mut s = Session::new();
        let missing = dir.join("nope.pdf").to_string_lossy().into_owned();
        let e = s
            .dispatch("score.open", &json!({ "path": missing }))
            .unwrap_err();
        assert_eq!(e.code(), "io_error");
        let not_file = dir.to_string_lossy().into_owned();
        let e = s
            .dispatch("score.open", &json!({ "path": not_file }))
            .unwrap_err();
        assert_eq!(e.code(), "io_error");
        let text = dir.join("notes.txt");
        std::fs::write(&text, "not a score").unwrap();
        let e = s
            .dispatch("score.open", &json!({ "path": text.to_string_lossy() }))
            .unwrap_err();
        assert_eq!(e.code(), "invalid_pdf");
        assert!(!s.has_score());
        let e = s.dispatch("score.open", &json!({})).unwrap_err();
        assert_eq!(e.code(), "bad_params");
    }
}
