//! Synthetic libraries: every model type filled in, deterministic from a seed, so the ops,
//! format and engine tests exercise the whole document model without real score files.

use scorecraft_doc::{
    Annotation, AnnotationKind, AudioTrack, BlobRef, Bookmark, Color, DeviceId, EntryItem, Folder,
    Hlc, IdGen, InkPoint, Layer, Library, ListEntry, Metadata, MidiOnOpen, PageArea, PageKey,
    PageRange, Score, Setlist, Shortcut, Snip, Source, SourceKind, Stamp, Stroke, TextNote,
    TextStyle, Tool, TurnMark,
};
use scorecraft_geom::{Rect, Rotation};

/// A fixed "now" for generated records: 2026-10-31T00:00:00Z.
pub const NOW_MS: u64 = 1_793_404_800_000;

/// A fake content address derived from `n` (not a real SHA-256 of anything).
pub fn blob(n: u8) -> BlobRef {
    BlobRef([n; 32])
}

/// A sound library of `scores` scores (at least one) with folders, a nested folder, layers, every
/// annotation kind, bookmarks, audio, MIDI, two setlists (one score in both), a placeholder,
/// one trashed score and settings. Same seed, same library.
pub fn sample_library(seed: u64, scores: usize) -> Library {
    let mut g = IdGen::new(seed);
    let now = NOW_MS;
    let device: DeviceId = g.id(now);
    let mut lib = Library::new(g.id(now));
    let horn: Folder = Folder {
        id: g.id(now),
        name: "Horn".into(),
        parent: None,
    };
    let etudes = Folder {
        id: g.id(now),
        name: "Études".into(),
        parent: Some(horn.id),
    };
    let folders = [horn.id, etudes.id];
    lib.folders.insert(horn.id, horn);
    lib.folders.insert(etudes.id, etudes);

    let mut ids = Vec::new();
    for i in 0..scores.max(1) {
        let pages = 3 + (i % 4) as u32;
        let mut s = Score::new(
            g.id(now),
            Source {
                kind: if i % 5 == 4 {
                    SourceKind::Images
                } else {
                    SourceKind::Pdf
                },
                blob: blob(i as u8),
                more: Vec::new(),
                file_name: format!("score-{i}.pdf"),
                page_count: pages,
            },
            g.id(now),
            now + i as u64,
        );
        s.metadata = Metadata {
            title: format!("Sonata No. {i}"),
            composer: ["Bach", "Mozart", "Brahms"]
                .get(i % 3)
                .copied()
                .unwrap_or_default()
                .into(),
            key: "E♭ major".into(),
            tags: vec!["recital".into(), format!("tag{}", i % 2)],
            ..Metadata::default()
        };
        s.metadata
            .custom
            .insert("Opus".into(), format!("{}", 10 + i));
        s.folder = folders.get(i % 2).copied().filter(|_| i % 3 != 0);
        if let Some(p) = s.pages.get_mut(1) {
            p.rotation = Rotation::R90;
            p.crop = Some(Rect::new(20.0, 20.0, 580.0, 760.0));
        }
        if let Some(p) = s.pages.get_mut(2) {
            p.hidden = i % 2 == 0;
        }
        if i % 2 == 1 {
            s.pages.swap(0, 1);
        }
        let top = Layer {
            id: g.id(now),
            name: "Teacher".into(),
            visible: true,
            locked: i % 2 == 0,
        };
        let base = s.layers.first().map(|l| l.id).unwrap_or(top.id);
        s.layers.push(top.clone());
        let stamp = Hlc::new(now, 0, device);
        let mut ann = |layer, kind| Annotation {
            id: g.id(now),
            layer,
            stamp,
            kind,
        };
        let anns = vec![
            ann(
                base,
                AnnotationKind::Stroke(Stroke {
                    tool: Tool::Pen,
                    color: Color::RED,
                    width: 1.5,
                    nib: Default::default(),
                    points: (0..20)
                        .map(|k| InkPoint {
                            x: 100.0 + k as f32 * 3.0,
                            y: 600.0 + (k as f32 * 0.7).sin() * 10.0,
                            pressure: 0.3 + k as f32 / 40.0,
                            tilt: 0.2,
                            twist: 0.0,
                            t: k * 8,
                        })
                        .collect(),
                }),
            ),
            ann(
                top.id,
                AnnotationKind::Stroke(Stroke {
                    tool: Tool::Highlighter,
                    color: Color::HIGHLIGHT,
                    width: 10.0,
                    nib: Default::default(),
                    points: vec![InkPoint::new(50.0, 300.0), InkPoint::new(250.0, 300.0)],
                }),
            ),
            ann(
                base,
                AnnotationKind::Text(TextNote {
                    rect: Rect::new(60.0, 700.0, 200.0, 720.0),
                    text: "Breathe here ✓".into(),
                    style: TextStyle {
                        bold: true,
                        background: Some(Color::WHITE),
                        ..TextStyle::default()
                    },
                }),
            ),
            ann(
                base,
                AnnotationKind::Stamp(Stamp {
                    glyph: 0xE4C0,
                    text: String::new(),
                    size: 0.0,
                    rect: Rect::new(300.0, 500.0, 320.0, 515.0),
                    color: Color::BLUE,
                }),
            ),
            ann(
                base,
                AnnotationKind::Stamp(Stamp {
                    glyph: 0,
                    text: "mf".into(),
                    size: 18.0,
                    rect: Rect::new(100.0, 400.0, 124.0, 418.0),
                    color: Color::BLACK,
                }),
            ),
            ann(
                base,
                AnnotationKind::Hairpin(scorecraft_doc::Hairpin {
                    from: scorecraft_geom::Point::new(130.0, 409.0),
                    to: scorecraft_geom::Point::new(202.0, 409.0),
                    crescendo: true,
                    opening: 6.0,
                    width: 1.0,
                    color: Color::BLACK,
                }),
            ),
        ];
        s.annotations.insert(PageKey(0), anns);
        s.annotations.insert(
            PageKey(1),
            vec![
                ann(
                    base,
                    AnnotationKind::Snip(Snip {
                        from: PageArea {
                            page: PageKey(2),
                            rect: Rect::new(0.0, 600.0, 612.0, 792.0),
                        },
                        to: PageArea {
                            page: PageKey(1),
                            rect: Rect::new(0.0, 0.0, 612.0, 192.0),
                        },
                    }),
                ),
                ann(
                    base,
                    AnnotationKind::Shortcut(Shortcut {
                        at: PageArea {
                            page: PageKey(1),
                            rect: Rect::new(500.0, 100.0, 540.0, 140.0),
                        },
                        target: PageKey(0),
                        target_rect: Some(Rect::new(0.0, 400.0, 612.0, 792.0)),
                    }),
                ),
            ],
        );
        s.bookmarks.push(Bookmark {
            id: g.id(now),
            name: "Coda".into(),
            page: PageKey(pages - 1),
        });
        if i == 0 {
            s.page_range = PageRange::new(0, pages - 2);
            s.audio = Some(AudioTrack {
                blob: blob(200),
                turns: vec![
                    TurnMark {
                        ms: 0,
                        page: PageKey(0),
                    },
                    TurnMark {
                        ms: 41_000,
                        page: PageKey(1),
                    },
                ],
                loop_ms: Some((10_000, 20_000)),
                speed: 0.75,
                transpose: -2,
                play_on_open: true,
            });
            s.midi_on_open = Some(MidiOnOpen {
                channel: 1,
                program: Some(60),
                bank: Some(0),
                song: None,
            });
            s.practice_notes = "Slow practice at ♩ = 60".into();
            s.opened_ms = Some(now + 1000);
        }
        ids.push(s.id);
        lib.scores.insert(s.id, s);
    }

    let list = |name: &str, scores: &[_], g: &mut IdGen| Setlist {
        id: g.id(now),
        name: name.into(),
        entries: scores
            .iter()
            .enumerate()
            .map(|(k, id)| ListEntry {
                id: g.id(now),
                item: EntryItem::Score {
                    score: *id,
                    page_range: (k == 0).then_some(PageRange { first: 0, last: 1 }),
                    tempo: (k == 1).then_some(96.0),
                },
                note: String::new(),
            })
            .collect(),
        modified_ms: now,
    };
    let mut recital = list("Recital", &ids, &mut g);
    recital.entries.insert(
        1,
        ListEntry {
            id: g.id(now),
            item: EntryItem::Placeholder {
                label: "Interval".into(),
            },
            note: "15 minutes".into(),
        },
    );
    let lesson = list("Lesson", ids.get(..1).unwrap_or_default(), &mut g);
    lib.lists.insert(recital.id, recital);
    lib.lists.insert(lesson.id, lesson);
    if let Some(last) = ids.last().filter(|_| ids.len() > 2) {
        lib.trash.insert(*last, now + 5000);
    }
    lib.settings.insert("turn.mode".into(), "half".into());
    lib
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_is_sound_and_deterministic() {
        let a = sample_library(1, 5);
        assert!(a.is_sound(), "{:#?}", a.problems());
        assert_eq!(a, sample_library(1, 5));
        assert_ne!(a, sample_library(2, 5));
        assert_eq!(a.scores.len(), 5);
        assert_eq!(a.trash.len(), 1);
    }
}

#[cfg(test)]
mod model_tests {
    use super::*;
    use scorecraft_doc::{FolderId, LayerId, ScoreId};

    #[test]
    fn whole_library_survives_json() {
        let lib = sample_library(3, 6);
        let json = serde_json::to_string_pretty(&lib).unwrap();
        let back: Library = serde_json::from_str(&json).unwrap();
        assert_eq!(back, lib);
    }

    #[test]
    fn problems_find_broken_references() {
        let mut lib = sample_library(4, 3);
        let score_id = *lib.scores.keys().next().unwrap();
        let s = lib.scores.get_mut(&score_id).unwrap();
        s.folder = Some(FolderId::NIL);
        s.pages.push(scorecraft_doc::PageRef::new(99));
        s.annotations
            .values_mut()
            .next()
            .unwrap()
            .first_mut()
            .unwrap()
            .layer = LayerId::NIL;
        if let Some(AnnotationKind::Stroke(st)) = s
            .annotations
            .values_mut()
            .next()
            .and_then(|v| v.first_mut())
            .map(|a| &mut a.kind)
        {
            st.points[0].x = f32::NAN;
        }
        lib.trash.insert(ScoreId::NIL, 0);
        let list = lib.lists.values_mut().next().unwrap();
        list.entries.push(ListEntry {
            id: list.entries[0].id,
            item: EntryItem::Score {
                score: ScoreId::NIL,
                page_range: None,
                tempo: Some(f32::INFINITY),
            },
            note: String::new(),
        });
        let problems = lib.problems();
        let text = format!("{problems:#?}");
        for needle in [
            "folder 00000000-0000-0000-0000-000000000000 does not exist",
            "source page 99",
            "layer 00000000-0000-0000-0000-000000000000 does not exist",
            "not a finite number",
            "deleted score does not exist",
            "duplicate entry id",
            "tempo inf",
        ] {
            assert!(text.contains(needle), "missing `{needle}` in {text}");
        }
    }

    #[test]
    fn folder_cycles_are_problems_not_hangs() {
        let mut lib = sample_library(5, 1);
        let ids: Vec<_> = lib.folders.keys().copied().collect();
        for (a, b) in [(ids[0], ids[1]), (ids[1], ids[0])] {
            lib.folders.get_mut(&a).unwrap().parent = Some(b);
        }
        assert!(
            format!("{:?}", lib.problems()).contains("cycle")
                || format!("{:?}", lib.problems()).contains("ancestor")
        );
    }
}
