//! A score: one imported file plus everything ScoreCraft keeps about it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use scorecraft_geom::{Rect, Rotation};

use crate::{Annotation, BlobRef, BookmarkId, FolderId, LayerId, PageKey, ScoreId};

/// What kind of file a score was imported from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SourceKind {
    #[default]
    Pdf,
    /// Images (photos, scans), one page per image, stored as one blob each.
    Images,
}

/// The imported file. Never rewritten (rule 3): edits are data beside it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Source {
    #[serde(default)]
    pub kind: SourceKind,
    /// The PDF, or the first image.
    pub blob: BlobRef,
    /// Further images, for [`SourceKind::Images`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub more: Vec<BlobRef>,
    /// The file's name when imported, for display and export.
    #[serde(default)]
    pub file_name: String,
    /// Pages in the source file.
    pub page_count: u32,
}

fn is_zero(n: &u8) -> bool {
    *n == 0
}

/// Title, composer and the rest of the searchable fields (LIB-4).
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Metadata {
    pub title: String,
    pub composer: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub arranger: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub key: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub genre: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub lyricist: String,
    /// Instruments and voices, one per entry ("Piano", "SATB").
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub instruments: Vec<String>,
    /// 1 to 5 stars; 0 is not set.
    #[serde(skip_serializing_if = "is_zero")]
    pub difficulty: u8,
    /// As typed: "1720", "c. 1720", "1801–02".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub year: String,
    /// "Op. 27 No. 2", "BWV 1007".
    #[serde(skip_serializing_if = "String::is_empty")]
    pub catalogue: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub publisher: String,
    /// User-defined fields.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub custom: BTreeMap<String, String>,
    /// The Description field of Edit Details.
    #[serde(skip_serializing_if = "String::is_empty")]
    pub notes: String,
    /// Fields filled in by detection rather than typed (LIB-7), so a re-run doesn't overwrite
    /// what the user wrote.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub detected: Vec<String>,
}

/// One page of the score as shown: which source page, turned and cropped how, hidden or not.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct PageRef {
    pub source: PageKey,
    /// Added to the PDF's own `/Rotate`.
    #[serde(default, skip_serializing_if = "is_r0")]
    pub rotation: Rotation,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub hidden: bool,
    /// In source-page PDF space; `None` shows the PDF's crop box.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub crop: Option<Rect>,
}

fn is_r0(r: &Rotation) -> bool {
    *r == Rotation::R0
}

impl PageRef {
    pub fn new(source: u32) -> PageRef {
        PageRef {
            source: PageKey(source),
            ..PageRef::default()
        }
    }
}

/// An inclusive range of score page positions, zero-based (PAGE-8, LIST-2).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRange {
    pub first: u32,
    pub last: u32,
}

impl PageRange {
    /// `None` when `first > last`.
    pub fn new(first: u32, last: u32) -> Option<PageRange> {
        (first <= last).then_some(PageRange { first, last })
    }

    pub fn contains(self, page: u32) -> bool {
        (self.first..=self.last).contains(&page)
    }
}

/// An annotation layer (ANNO-11): drawn in list order, bottom first.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: LayerId,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub locked: bool,
}

fn yes() -> bool {
    true
}

/// A named page (LIB-11).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bookmark {
    pub id: BookmarkId,
    pub name: String,
    pub page: PageKey,
}

/// A page turn recorded against the audio (PLAY-3).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnMark {
    pub ms: u64,
    pub page: PageKey,
}

/// An audio recording attached to the score (PLAY-1..6).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioTrack {
    pub blob: BlobRef,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub turns: Vec<TurnMark>,
    /// Loop start and end in milliseconds.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub loop_ms: Option<(u64, u64)>,
    /// Playback speed, 1 = normal.
    #[serde(default = "one")]
    pub speed: f32,
    /// Transposition in semitones.
    #[serde(default)]
    pub transpose: i8,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub play_on_open: bool,
}

fn one() -> f32 {
    1.0
}

/// MIDI messages sent when the score opens (MIDI-3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MidiOnOpen {
    pub channel: u8,
    pub program: Option<u8>,
    pub bank: Option<u16>,
    pub song: Option<u8>,
}

/// A score in the library.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Score {
    pub id: ScoreId,
    pub source: Source,
    #[serde(default)]
    pub metadata: Metadata,
    /// The folder it is filed in; `None` is the library root.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub folder: Option<FolderId>,
    /// The score's pages in reading order. Deleting a page removes its entry here; its source
    /// page and annotations stay in the file and the library.
    pub pages: Vec<PageRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub page_range: Option<PageRange>,
    pub layers: Vec<Layer>,
    /// Annotations by source page.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub annotations: BTreeMap<PageKey, Vec<Annotation>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub bookmarks: Vec<Bookmark>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub audio: Option<AudioTrack>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub midi_on_open: Option<MidiOnOpen>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub practice_notes: String,
    /// Unix milliseconds.
    pub added_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub opened_ms: Option<u64>,
    /// Unix milliseconds of the last change to this record.
    #[serde(default)]
    pub modified_ms: u64,
}

impl Score {
    /// A freshly imported score: every source page in order and one empty layer.
    pub fn new(id: ScoreId, source: Source, layer: LayerId, now_ms: u64) -> Score {
        let pages = (0..source.page_count).map(PageRef::new).collect();
        Score {
            id,
            source,
            metadata: Metadata::default(),
            folder: None,
            pages,
            page_range: None,
            layers: vec![Layer {
                id: layer,
                name: "Layer 1".to_owned(),
                visible: true,
                locked: false,
            }],
            annotations: BTreeMap::new(),
            bookmarks: Vec::new(),
            audio: None,
            midi_on_open: None,
            practice_notes: String::new(),
            added_ms: now_ms,
            opened_ms: None,
            modified_ms: now_ms,
        }
    }

    /// The pages a reader shows, in order: not hidden, within the page range. Positions are into
    /// [`Score::pages`].
    pub fn visible_pages(&self) -> Vec<usize> {
        self.pages
            .iter()
            .enumerate()
            .filter(|(i, p)| {
                !p.hidden
                    && self
                        .page_range
                        .is_none_or(|r| u32::try_from(*i).is_ok_and(|i| r.contains(i)))
            })
            .map(|(i, _)| i)
            .collect()
    }

    pub fn layer(&self, id: LayerId) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }

    /// Every annotation on source page `page`.
    pub fn annotations_on(&self, page: PageKey) -> &[Annotation] {
        self.annotations.get(&page).map_or(&[], Vec::as_slice)
    }

    /// The display title: the title, else the file name.
    pub fn display_title(&self) -> &str {
        if self.metadata.title.is_empty() {
            &self.source.file_name
        } else {
            &self.metadata.title
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    pub(crate) fn score(pages: u32) -> Score {
        Score::new(
            ScoreId(Uuid::from_u128(1)),
            Source {
                kind: SourceKind::Pdf,
                blob: BlobRef::of_hex(&"ab".repeat(32)).unwrap(),
                more: vec![],
                file_name: "Sonata.pdf".into(),
                page_count: pages,
            },
            LayerId(Uuid::from_u128(2)),
            1000,
        )
    }

    #[test]
    fn visible_pages_skip_hidden_and_out_of_range() {
        let mut s = score(5);
        s.pages[1].hidden = true;
        assert_eq!(s.visible_pages(), vec![0, 2, 3, 4]);
        s.page_range = PageRange::new(2, 3);
        assert_eq!(s.visible_pages(), vec![2, 3]);
        assert_eq!(s.display_title(), "Sonata.pdf");
    }

    #[test]
    fn minimal_json_loads_with_defaults() {
        let s = score(2);
        let json = serde_json::to_value(&s).unwrap();
        assert!(
            json.get("annotations").is_none(),
            "empty fields are left out"
        );
        let back: Score = serde_json::from_value(json).unwrap();
        assert_eq!(back, s);
    }
}
