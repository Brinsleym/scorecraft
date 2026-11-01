//! Property tests on ops and history (M1 DoD): random edit sequences on a full sample library
//! keep it sound; undoing every step restores it exactly; redoing every step reproduces the
//! final state; a failed transaction changes nothing.

use proptest::prelude::*;

use scorecraft_doc::{
    Annotation, AnnotationKind, Bookmark, Color, EntryItem, Folder, Hlc, IdGen, InkPoint, Layer,
    Library, ListEntry, Metadata, PageKey, PageRange, Setlist, Stroke, Tool,
};
use scorecraft_geom::Rotation;
use scorecraft_ops::{History, Op, OpError, Scope, Transaction, edit};
use scorecraft_testkit::library::{NOW_MS, sample_library};

/// One random edit: a kind and three numbers that pick its targets in the current library.
type Pick = (u8, u16, u16, u16);

fn nth<T: Clone>(it: impl Iterator<Item = T>, n: u16) -> Option<T> {
    let v: Vec<T> = it.collect();
    if v.is_empty() {
        None
    } else {
        v.get(n as usize % v.len()).cloned()
    }
}

/// Builds a transaction for `pick` against `lib`, or `None` when the pick has no target.
fn build(lib: &Library, g: &mut IdGen, (kind, a, b, c): Pick) -> Option<(String, Transaction)> {
    let score = nth(lib.scores.values(), a);
    let tx = match kind % 23 {
        0 => {
            let s = score?;
            edit::rotate_page(lib, s.id, b as usize % s.pages.len().max(1), Rotation::R90).ok()?
        }
        1 => {
            let s = score?;
            edit::set_page_hidden(lib, s.id, b as usize % s.pages.len().max(1), c % 2 == 0).ok()?
        }
        2 => {
            let s = score?;
            let n = s.pages.len().max(1);
            Transaction::single(Op::MovePage {
                score: s.id,
                from: b as usize % n,
                to: c as usize % n,
            })
        }
        3 => {
            let s = score?;
            edit::delete_pages(lib, s.id, &[b as usize % s.pages.len().max(1)]).ok()?
        }
        4 => {
            let s = score?;
            Transaction::single(Op::SetMetadata {
                score: s.id,
                metadata: Box::new(Metadata {
                    title: format!("Title {b}"),
                    ..s.metadata.clone()
                }),
            })
        }
        5 => {
            let s = score?;
            Transaction::single(Op::SetScoreFolder {
                score: s.id,
                folder: nth(lib.folders.keys().copied(), b).filter(|_| c % 3 != 0),
            })
        }
        6 => {
            let s = score?;
            let layer = nth(s.layers.iter().map(|l| l.id), b)?;
            let page = PageKey(u32::from(c) % s.source.page_count.max(1));
            let n = s.annotations_on(page).len();
            Transaction::single(Op::InsertAnnotation {
                score: s.id,
                page,
                index: b as usize % (n + 1),
                annotation: Box::new(Annotation {
                    id: g.id(NOW_MS),
                    layer,
                    stamp: Hlc::default(),
                    kind: AnnotationKind::Stroke(Stroke {
                        tool: Tool::Pen,
                        color: Color::BLACK,
                        width: 1.0,
                        nib: Default::default(),
                        points: vec![InkPoint::new(f32::from(b), f32::from(c))],
                    }),
                }),
            })
        }
        7 => {
            let s = score?;
            let (page, id) = nth(
                s.annotations
                    .iter()
                    .flat_map(|(p, v)| v.iter().map(move |x| (*p, x.id))),
                b,
            )?;
            Transaction::single(Op::RemoveAnnotation {
                score: s.id,
                page,
                annotation: id,
            })
        }
        8 => {
            let s = score?;
            let mut l = nth(s.layers.iter().cloned(), b)?;
            l.visible = !l.visible;
            Transaction::single(Op::SetLayer {
                score: s.id,
                layer: l,
            })
        }
        9 => {
            let s = score?;
            Transaction::single(Op::InsertLayer {
                score: s.id,
                index: b as usize % (s.layers.len() + 1),
                layer: Layer {
                    id: g.id(NOW_MS),
                    name: format!("L{c}"),
                    visible: true,
                    locked: false,
                },
            })
        }
        10 => {
            let s = score?;
            edit::remove_layer(lib, s.id, nth(s.layers.iter().map(|l| l.id), b)?).ok()?
        }
        11 => {
            let s = score?;
            Transaction::single(Op::InsertBookmark {
                score: s.id,
                index: 0,
                bookmark: Bookmark {
                    id: g.id(NOW_MS),
                    name: format!("B{b}"),
                    page: PageKey(u32::from(c) % s.source.page_count.max(1)),
                },
            })
        }
        12 => Transaction::single(Op::InsertFolder {
            folder: Folder {
                id: g.id(NOW_MS),
                name: format!("F{b}"),
                parent: nth(lib.folders.keys().copied(), c).filter(|_| b % 2 == 0),
            },
        }),
        13 => edit::delete_folder(lib, nth(lib.folders.keys().copied(), b)?).ok()?,
        14 => Transaction::single(Op::InsertList {
            list: Box::new(Setlist {
                id: g.id(NOW_MS),
                name: format!("List {b}"),
                entries: Vec::new(),
                modified_ms: NOW_MS,
            }),
        }),
        15 => {
            let l = nth(lib.lists.values(), b)?;
            let s = score?;
            Transaction::single(Op::InsertEntry {
                list: l.id,
                index: c as usize % (l.entries.len() + 1),
                entry: ListEntry {
                    id: g.id(NOW_MS),
                    item: EntryItem::Score {
                        score: s.id,
                        page_range: None,
                        tempo: Some(f32::from(c % 200) + 20.0),
                    },
                    note: String::new(),
                },
            })
        }
        16 => {
            let l = nth(lib.lists.values(), b)?;
            let n = l.entries.len().max(1);
            Transaction::single(Op::MoveEntry {
                list: l.id,
                from: a as usize % n,
                to: c as usize % n,
            })
        }
        17 => {
            let l = nth(lib.lists.values(), b)?;
            Transaction::single(Op::RemoveEntry {
                list: l.id,
                entry: nth(l.entries.iter().map(|e| e.id), c)?,
            })
        }
        18 => Transaction::single(Op::Trash {
            score: score?.id,
            at_ms: NOW_MS,
        }),
        19 => Transaction::single(Op::Untrash {
            score: nth(lib.trash.keys().copied(), b)?,
        }),
        20 => edit::empty_trash(lib).ok()?,
        21 => {
            let s = score?;
            let n = s.layers.len().max(1);
            Transaction::single(Op::MoveLayer {
                score: s.id,
                from: b as usize % n,
                to: c as usize % n,
            })
        }
        _ => {
            let s = score?;
            let n = s.pages.len() as u32;
            let first = u32::from(b) % n.max(1);
            Transaction::single(Op::SetPageRange {
                score: s.id,
                range: PageRange::new(first, first + u32::from(c) % (n - first).max(1)),
            })
        }
    };
    Some((format!("edit {kind}"), tx))
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn undo_all_restores_and_redo_all_replays(
        picks in prop::collection::vec(any::<Pick>(), 1..60),
        seed in any::<u64>(),
    ) {
        let original = sample_library(seed, 4);
        let mut lib = original.clone();
        let mut hist = History::new(1000);
        let mut g = IdGen::new(seed ^ 0xABCD);
        let mut steps = 0;
        for pick in picks {
            let Some((label, tx)) = build(&lib, &mut g, pick) else { continue };
            let before = lib.clone();
            match hist.commit(&mut lib, &label, tx) {
                Ok(applied) => {
                    if !applied.inverse.is_empty() {
                        steps += 1;
                    }
                    prop_assert!(lib.is_sound(), "{label}: {:#?}", lib.problems());
                }
                Err(_) => prop_assert_eq!(&lib, &before, "a failed edit changed the library"),
            }
        }
        let edited = lib.clone();
        for _ in 0..steps {
            hist.undo(&mut lib, None).unwrap();
            prop_assert!(lib.is_sound());
        }
        prop_assert!(matches!(hist.undo(&mut lib, None), Err(OpError::NothingToUndo)));
        prop_assert_eq!(&lib, &original);
        for _ in 0..steps {
            hist.redo(&mut lib, None).unwrap();
        }
        prop_assert_eq!(&lib, &edited);
    }

    #[test]
    fn ops_survive_json(picks in prop::collection::vec(any::<Pick>(), 1..30), seed in any::<u64>()) {
        let lib = sample_library(seed, 3);
        let mut g = IdGen::new(seed);
        for pick in picks {
            if let Some((_, tx)) = build(&lib, &mut g, pick) {
                let json = serde_json::to_string(&tx).unwrap();
                prop_assert_eq!(serde_json::from_str::<Transaction>(&json).unwrap(), tx);
            }
        }
    }
}

#[test]
fn a_failing_op_rolls_back_the_whole_transaction() {
    let mut lib = sample_library(9, 2);
    let before = lib.clone();
    let s = lib.scores.values().next().unwrap().clone();
    let tx = Transaction::new(vec![
        Op::SetPracticeNotes {
            score: s.id,
            notes: "changed".into(),
        },
        Op::MovePage {
            score: s.id,
            from: 0,
            to: 1,
        },
        Op::MovePage {
            score: s.id,
            from: 0,
            to: 999,
        },
    ]);
    let err = History::default().commit(&mut lib, "x", tx).unwrap_err();
    assert_eq!(err.code(), "out_of_range");
    assert_eq!(lib, before);
}

#[test]
fn scopes_undo_independently() {
    let mut lib = sample_library(10, 2);
    let original = lib.clone();
    let ids: Vec<_> = lib.scores.keys().copied().collect();
    let mut h = History::default();
    for (i, id) in ids.iter().enumerate() {
        h.commit(
            &mut lib,
            &format!("notes {i}"),
            Transaction::single(Op::SetPracticeNotes {
                score: *id,
                notes: format!("n{i}"),
            }),
        )
        .unwrap();
    }
    h.commit(
        &mut lib,
        "setting",
        Transaction::single(Op::SetSetting {
            key: "k".into(),
            value: Some("v".into()),
        }),
    )
    .unwrap();
    // Undo the first score's step while newer steps exist elsewhere.
    let u = h.undo(&mut lib, Some(Scope::Score(ids[0]))).unwrap();
    assert_eq!(u.label, "notes 0");
    assert_eq!(
        lib.scores[&ids[0]].practice_notes,
        original.scores[&ids[0]].practice_notes
    );
    assert_eq!(lib.scores[&ids[1]].practice_notes, "n1");
    assert_eq!(h.undo_label(None), Some("setting"));
    assert!(!h.can_undo(Some(Scope::Score(ids[0]))));
    assert!(h.can_redo(Some(Scope::Score(ids[0]))));
    // A new step in that scope clears its redo.
    h.commit(
        &mut lib,
        "again",
        Transaction::single(Op::SetPracticeNotes {
            score: ids[0],
            notes: "z".into(),
        }),
    )
    .unwrap();
    assert!(!h.can_redo(Some(Scope::Score(ids[0]))));
}

#[test]
fn history_is_bounded() {
    let mut lib = sample_library(11, 1);
    let id = *lib.scores.keys().next().unwrap();
    let mut h = History::new(3);
    for i in 0..10 {
        h.commit(
            &mut lib,
            "n",
            Transaction::single(Op::SetPracticeNotes {
                score: id,
                notes: i.to_string(),
            }),
        )
        .unwrap();
    }
    assert_eq!(h.depth(Scope::Score(id)), 3);
}

#[test]
fn integrity_guards() {
    let mut lib = sample_library(12, 3);
    let s = lib.scores.values().next().unwrap().clone();
    let only_layer_left = {
        let mut l = lib.clone();
        let sc = l.scores.get_mut(&s.id).unwrap();
        sc.annotations.clear();
        sc.layers.truncate(1);
        Op::RemoveLayer {
            score: s.id,
            layer: sc.layers[0].id,
        }
        .apply(&mut l)
        .unwrap_err()
    };
    assert_eq!(only_layer_left.code(), "invalid_edit");
    let f = *lib.folders.keys().next().unwrap();
    assert_eq!(
        Op::RemoveFolder { folder: f }
            .apply(&mut lib)
            .unwrap_err()
            .code(),
        "invalid_edit"
    );
    let all: Vec<usize> = (0..s.pages.len()).collect();
    assert!(edit::delete_pages(&lib, s.id, &all).is_err());
    // Moving a folder into its own child is refused.
    let child = lib
        .folders
        .values()
        .find(|x| x.parent.is_some())
        .unwrap()
        .clone();
    let mut parent = lib.folders[&child.parent.unwrap()].clone();
    parent.parent = Some(child.id);
    assert_eq!(
        Op::SetFolder { folder: parent }
            .apply(&mut lib)
            .unwrap_err()
            .code(),
        "invalid_edit"
    );
}
