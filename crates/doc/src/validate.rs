//! Integrity checks on a library (LIB-12 "Verify & Repair" uses these). A library read from disk
//! may be damaged or hand-edited, so every reference and number is checked, not assumed.

use std::collections::BTreeSet;

use crate::{AnnotationKind, EntryItem, Library, Score};

/// The most problems one check reports, so a badly damaged library can't produce a huge report.
pub const MAX_PROBLEMS: usize = 1000;

/// One thing wrong with a library.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Problem {
    /// Where: `scores/<id>/pages/3`, `lists/<id>/entries/0`, …
    pub path: String,
    pub message: String,
}

struct Report(Vec<Problem>);

impl Report {
    fn add(&mut self, path: impl Into<String>, message: impl Into<String>) {
        if self.0.len() < MAX_PROBLEMS {
            self.0.push(Problem {
                path: path.into(),
                message: message.into(),
            });
        }
    }
}

impl Library {
    /// Every integrity problem, up to [`MAX_PROBLEMS`]; empty for a sound library.
    pub fn problems(&self) -> Vec<Problem> {
        let mut r = Report(Vec::new());
        for (id, f) in &self.folders {
            let at = format!("folders/{id}");
            if f.id != *id {
                r.add(&at, "id does not match its key");
            }
            if let Some(p) = f.parent {
                if !self.folders.contains_key(&p) {
                    r.add(&at, format!("parent folder {p} does not exist"));
                } else if self.ancestors(*id).contains(id) || p == *id {
                    r.add(&at, "folder is its own ancestor");
                } else if self.ancestors(p).contains(id) {
                    r.add(&at, "folder tree has a cycle");
                }
            }
        }
        for (id, s) in &self.scores {
            let at = format!("scores/{id}");
            if s.id != *id {
                r.add(&at, "id does not match its key");
            }
            if let Some(f) = s.folder
                && !self.folders.contains_key(&f)
            {
                r.add(&at, format!("folder {f} does not exist"));
            }
            check_score(s, &at, &mut r);
        }
        for (id, l) in &self.lists {
            let at = format!("lists/{id}");
            if l.id != *id {
                r.add(&at, "id does not match its key");
            }
            let mut seen = BTreeSet::new();
            for (i, e) in l.entries.iter().enumerate() {
                let at = format!("{at}/entries/{i}");
                if !seen.insert(e.id) {
                    r.add(&at, "duplicate entry id");
                }
                if let EntryItem::Score {
                    score,
                    page_range,
                    tempo,
                } = &e.item
                {
                    match self.scores.get(score) {
                        None => r.add(&at, format!("score {score} does not exist")),
                        Some(s) => {
                            if let Some(pr) = page_range
                                && (pr.first > pr.last || pr.last as usize >= s.pages.len())
                            {
                                r.add(&at, "page range is outside the score");
                            }
                        }
                    }
                    if let Some(t) = tempo
                        && !(t.is_finite() && *t > 0.0 && *t <= 1000.0)
                    {
                        r.add(&at, format!("tempo {t} is not between 0 and 1000"));
                    }
                }
            }
        }
        for id in self.trash.keys() {
            if !self.scores.contains_key(id) {
                r.add(format!("trash/{id}"), "deleted score does not exist");
            }
        }
        r.0
    }

    /// No problems at all.
    pub fn is_sound(&self) -> bool {
        self.problems().is_empty()
    }
}

fn check_score(s: &Score, at: &str, r: &mut Report) {
    let n = s.source.page_count;
    for (i, p) in s.pages.iter().enumerate() {
        if p.source.0 >= n {
            r.add(
                format!("{at}/pages/{i}"),
                format!("source page {} is past the file's {n} pages", p.source.0),
            );
        }
        if p.crop.is_some_and(|c| !c.is_finite() || c.is_empty()) {
            r.add(format!("{at}/pages/{i}"), "crop is empty or not finite");
        }
    }
    if let Some(pr) = s.page_range
        && (pr.first > pr.last || pr.last as usize >= s.pages.len())
    {
        r.add(at, "page range is outside the score");
    }
    if s.layers.is_empty() {
        r.add(at, "score has no layer");
    }
    let mut layers = BTreeSet::new();
    for l in &s.layers {
        if !layers.insert(l.id) {
            r.add(format!("{at}/layers/{}", l.id), "duplicate layer id");
        }
    }
    let mut ids = BTreeSet::new();
    for (page, anns) in &s.annotations {
        let pat = format!("{at}/annotations/{}", page.0);
        if page.0 >= n {
            r.add(&pat, format!("page is past the file's {n} pages"));
        }
        for a in anns {
            let aat = format!("{pat}/{}", a.id);
            if !ids.insert(a.id) {
                r.add(&aat, "duplicate annotation id");
            }
            if !layers.contains(&a.layer) {
                r.add(&aat, format!("layer {} does not exist", a.layer));
            }
            if !annotation_is_finite(&a.kind) {
                r.add(&aat, "has a coordinate that is not a finite number");
            }
            let target = match &a.kind {
                AnnotationKind::Snip(sn) => [Some(sn.from.page), Some(sn.to.page)],
                AnnotationKind::Shortcut(sc) => [Some(sc.at.page), Some(sc.target)],
                _ => [None, None],
            };
            if target.into_iter().flatten().any(|p| p.0 >= n) {
                r.add(&aat, "points to a page past the end of the file");
            }
        }
    }
    for b in &s.bookmarks {
        if b.page.0 >= n {
            r.add(
                format!("{at}/bookmarks/{}", b.id),
                "page is past the end of the file",
            );
        }
    }
    if let Some(a) = &s.audio
        && !(a.speed.is_finite() && a.speed > 0.0)
    {
        r.add(format!("{at}/audio"), "speed must be a positive number");
    }
}

fn annotation_is_finite(k: &AnnotationKind) -> bool {
    match k {
        AnnotationKind::Stroke(s) => {
            s.width.is_finite() && s.points.iter().all(crate::InkPoint::is_finite)
        }
        AnnotationKind::Text(t) => t.rect.is_finite() && t.style.size.is_finite(),
        AnnotationKind::Stamp(s) => s.rect.is_finite() && s.size.is_finite(),
        AnnotationKind::Hairpin(h) => {
            h.from.is_finite() && h.to.is_finite() && h.opening.is_finite() && h.width.is_finite()
        }
        AnnotationKind::Snip(s) => s.from.rect.is_finite() && s.to.rect.is_finite(),
        AnnotationKind::Shortcut(s) => {
            s.at.rect.is_finite() && s.target_rect.is_none_or(|r| r.is_finite())
        }
    }
}
