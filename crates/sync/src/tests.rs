//! Merge rules and convergence (`docs/spec/sync.md`, SYNC-T1..T9).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::collections::BTreeSet;

use proptest::prelude::*;

use scorecraft_doc::{
    Annotation, AnnotationKind, Bookmark, Color, DeviceId, EntryItem, Folder, IdGen, InkPoint,
    LayerId, Library, ListEntry, PageKey, ScoreId, Setlist, Stroke, Tool,
};
use scorecraft_testkit::library::{NOW_MS, sample_library};

use super::*;

#[test]
fn crate_name_matches_package() {
    assert_eq!(super::CRATE_NAME, "scorecraft-sync");
}

fn device(g: &mut IdGen) -> DeviceId {
    g.id(NOW_MS)
}

fn pair() -> (Replica, Replica, ScoreId, IdGen) {
    let lib = sample_library(7, 3);
    let mut g = IdGen::new(99);
    let a = Replica::new(device(&mut g), lib.clone());
    let b = Replica::new(device(&mut g), lib.clone());
    let score = *lib.scores.keys().next().unwrap();
    (a, b, score, g)
}

fn stroke(g: &mut IdGen, layer: LayerId, x: f32) -> Annotation {
    Annotation {
        id: g.id(NOW_MS),
        layer,
        stamp: Default::default(),
        kind: AnnotationKind::Stroke(Stroke {
            tool: Tool::Pen,
            color: Color::BLACK,
            width: 1.0,
            nib: Default::default(),
            points: vec![InkPoint::new(x, 100.0), InkPoint::new(x + 20.0, 120.0)],
        }),
    }
}

fn layer(lib: &Library, score: ScoreId) -> LayerId {
    lib.scores[&score].layers[0].id
}

fn put(score: ScoreId, page: u32, annotation: Annotation) -> Op {
    Op::PutAnnotation {
        score,
        page: PageKey(page),
        annotation,
        revive: false,
    }
}

fn rename(score: ScoreId, lib: &Library, title: &str) -> Op {
    let mut metadata = lib.scores[&score].metadata.clone();
    metadata.title = title.into();
    Op::SetScore {
        score,
        field: ScoreField::Metadata { metadata },
    }
}

fn recolour(mut a: Annotation, c: Color) -> Annotation {
    if let AnnotationKind::Stroke(s) = &mut a.kind {
        s.color = c;
    }
    a
}

#[test]
fn sync_t1_strokes_drawn_offline_on_both_are_both_kept_in_the_same_order() {
    let (mut a, mut b, score, mut g) = pair();
    let l = layer(a.library(), score);
    let before = a.library().scores[&score].annotations_on(PageKey(0)).len();
    let sa = stroke(&mut g, l, 10.0);
    let sb = stroke(&mut g, l, 50.0);
    // b draws second but its change is made "earlier" by the wall clock: order is by id.
    a.local(put(score, 0, sa.clone()), NOW_MS + 10);
    b.local(put(score, 0, sb.clone()), NOW_MS + 5);
    exchange(&mut a, &mut b, NOW_MS + 20);
    assert_eq!(a.library(), b.library());
    let ids: Vec<_> = a.library().scores[&score]
        .annotations_on(PageKey(0))
        .iter()
        .map(|x| x.id)
        .collect();
    assert_eq!(ids.len(), before + 2);
    assert!(ids.contains(&sa.id) && ids.contains(&sb.id));
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted, "markings on a page are in id order");
}

#[test]
fn sync_t2_a_delete_beats_a_concurrent_recolour_for_good() {
    let (mut a, mut b, score, mut g) = pair();
    let l = layer(a.library(), score);
    let s = stroke(&mut g, l, 10.0);
    a.local(put(score, 1, s.clone()), NOW_MS + 1);
    exchange(&mut a, &mut b, NOW_MS + 2);
    a.local(
        Op::RemoveAnnotation {
            score,
            annotation: s.id,
        },
        NOW_MS + 3,
    );
    // b's recolour is later by the clock, yet the delete wins.
    let recoloured = b.local(put(score, 1, recolour(s.clone(), Color::RED)), NOW_MS + 9);
    exchange(&mut a, &mut b, NOW_MS + 10);
    assert_eq!(a.library(), b.library());
    let gone = |r: &Replica| {
        r.library().scores[&score]
            .annotations
            .values()
            .flatten()
            .all(|x| x.id != s.id)
    };
    assert!(gone(&a) && gone(&b));
    assert!(a.is_removed(s.id));
    // The recolour arriving again (a resend, a restored backup) doesn't bring it back.
    let mut c = Replica::new(device(&mut g), sample_library(7, 3));
    c.receive(&a.changes_for(&Summary::new()), NOW_MS + 11);
    assert_eq!(
        c.receive(std::slice::from_ref(&recoloured), NOW_MS + 12)
            .new,
        0
    );
    a.receive(&[recoloured], NOW_MS + 12);
    assert!(gone(&a) && gone(&c));
    assert_eq!(a.library(), c.library());
}

#[test]
fn sync_t3_two_renames_offline_keep_the_later_one() {
    let (mut a, mut b, score, _) = pair();
    let op = rename(score, a.library(), "From A");
    a.local(op, NOW_MS + 100);
    let op = rename(score, b.library(), "From B");
    b.local(op, NOW_MS + 50);
    exchange(&mut a, &mut b, NOW_MS + 200);
    assert_eq!(a.library(), b.library());
    assert_eq!(a.library().scores[&score].metadata.title, "From A");
    // After hearing of A's rename, B's next rename is later even if B's wall clock is behind.
    let op = rename(score, b.library(), "B again");
    b.local(op, NOW_MS + 60);
    exchange(&mut a, &mut b, NOW_MS + 70);
    assert_eq!(a.library().scores[&score].metadata.title, "B again");
    assert_eq!(a.library(), b.library());
}

#[test]
fn sync_t5_duplicates_and_reordering_change_nothing() {
    let (mut a, b, score, mut g) = pair();
    let l = layer(a.library(), score);
    let s = stroke(&mut g, l, 10.0);
    let mut changes = vec![
        a.local(put(score, 0, s.clone()), NOW_MS + 1),
        a.local(put(score, 2, recolour(s.clone(), Color::BLUE)), NOW_MS + 2),
    ];
    let op = rename(score, a.library(), "Clara");
    changes.push(a.local(op, NOW_MS + 3));
    let s2 = stroke(&mut g, l, 70.0);
    changes.push(a.local(put(score, 1, s2.clone()), NOW_MS + 4));
    changes.push(a.local(
        Op::RemoveAnnotation {
            score,
            annotation: s2.id,
        },
        NOW_MS + 5,
    ));
    let mut in_order = b.clone();
    in_order.receive(&changes, NOW_MS + 10);
    assert_eq!(in_order.library(), a.library());
    let mut shuffled = b;
    let mut rev = changes.clone();
    rev.reverse();
    rev.extend(changes.iter().cloned());
    shuffled.receive(&rev, NOW_MS + 10);
    shuffled.receive(&changes, NOW_MS + 11);
    assert_eq!(shuffled.library(), a.library());
    // The stroke moved to the third page in blue; the second stroke is gone.
    let on2 = shuffled.library().scores[&score].annotations_on(PageKey(2));
    assert!(on2.iter().any(|x| x.id == s.id));
    assert_eq!(shuffled.summary()[&a.device()], 5);
    assert!(shuffled.changes_for(&a.summary()).is_empty());
}

#[test]
fn a_change_for_a_score_not_here_yet_waits_for_it() {
    let lib = sample_library(3, 2);
    let mut g = IdGen::new(5);
    let mut a = Replica::new(device(&mut g), lib.clone());
    let mut empty = lib.clone();
    let score_id = *lib.scores.keys().next().unwrap();
    let score = empty.scores.remove(&score_id).unwrap();
    let mut b = Replica::new(device(&mut g), empty);
    let l = score.layers[0].id;
    let s = stroke(&mut g, l, 5.0);
    let add = a.local(
        Op::AddScore {
            score: Box::new(score),
        },
        NOW_MS + 1,
    );
    let draw = a.local(put(score_id, 0, s), NOW_MS + 2);
    b.receive(&[draw], NOW_MS + 3);
    assert_eq!(b.pending(), 1);
    assert!(!b.library().scores.contains_key(&score_id));
    let r = b.receive(&[add], NOW_MS + 4);
    assert_eq!(b.pending(), 0);
    assert!(r.changed.contains(&Record::Score(score_id)));
    assert_eq!(a.library(), b.library());
}

#[test]
fn changes_travel_as_json() {
    let (mut a, _, score, mut g) = pair();
    let l = layer(a.library(), score);
    let c = a.local(put(score, 0, stroke(&mut g, l, 1.0)), NOW_MS);
    let text = serde_json::to_string(&c).unwrap();
    assert!(text.contains("\"putAnnotation\":{"), "{text}");
    let back: Change = serde_json::from_str(&text).unwrap();
    assert_eq!(back, c);
}

/// A device: its own library (edited directly, like the engine does) and its replica.
#[derive(Clone)]
struct Device {
    lib: Library,
    rep: Replica,
}

impl Device {
    fn join(lib: Library, id: DeviceId, now: u64) -> Device {
        let mut rep = Replica::new(id, Library::new(lib.id));
        rep.record_all(&lib, now);
        Device { lib, rep }
    }

    fn edit(&mut self, records: &[Record], now: u64) -> Vec<Change> {
        self.rep.record(&self.lib, records, now)
    }

    fn get(&mut self, changes: &[Change], now: u64) {
        let r = self.rep.receive(changes, now);
        self.rep.copy_into(&mut self.lib, &r.changed);
    }
}

/// Markings and bookmarks sorted by id, as replicas keep them.
fn normal(lib: &Library) -> Library {
    let mut l = lib.clone();
    for s in l.scores.values_mut() {
        for list in s.annotations.values_mut() {
            list.sort_by_key(|a| a.id);
        }
        s.bookmarks.sort_by_key(|b| b.id);
    }
    l
}

fn sync_all(devs: &mut [Device], now: u64) {
    for _ in 0..2 {
        for i in 0..devs.len() {
            for j in 0..devs.len() {
                if i != j {
                    let out = devs[i].rep.changes_for(&devs[j].rep.summary());
                    devs[j].get(&out, now);
                }
            }
        }
    }
}

#[test]
fn sync_t6_a_whole_library_joins_and_edits_follow() {
    let mut g = IdGen::new(8);
    let lib = sample_library(4, 3);
    let a = Device::join(lib.clone(), device(&mut g), NOW_MS);
    // b joins with nothing but the library's id.
    let b = Device::join(Library::new(lib.id), device(&mut g), NOW_MS);
    let mut devs = [a, b];
    sync_all(&mut devs, NOW_MS + 1);
    let [mut a, mut b] = devs;
    assert_eq!(normal(&b.lib), normal(&lib));
    assert_eq!(a.rep.library(), b.rep.library());
    // An edit made directly on a's library travels as changes.
    let sid = *lib.scores.keys().next().unwrap();
    a.lib.scores.get_mut(&sid).unwrap().metadata.composer = "Fanny Mendelssohn".into();
    let l = layer(&a.lib, sid);
    let s = stroke(&mut g, l, 3.0);
    a.lib
        .scores
        .get_mut(&sid)
        .unwrap()
        .annotations
        .entry(PageKey(1))
        .or_default()
        .push(s);
    let changes = a.edit(&[Record::Score(sid)], NOW_MS + 2);
    assert_eq!(changes.len(), 2, "{changes:?}");
    b.get(&changes, NOW_MS + 3);
    assert_eq!(normal(&a.lib), normal(&b.lib));
    // Nothing new to record when nothing changed.
    assert!(
        a.edit(&[Record::Score(sid), Record::Library], NOW_MS + 4)
            .is_empty()
    );
}

#[test]
fn sync_t7_undoing_a_delete_brings_it_back_everywhere() {
    let mut g = IdGen::new(9);
    let lib = sample_library(5, 2);
    let sid = *lib.scores.keys().next().unwrap();
    let mut a = Device::join(lib.clone(), device(&mut g), NOW_MS);
    let mut b = Device::join(Library::new(lib.id), device(&mut g), NOW_MS);
    b.get(&a.rep.changes_for(&b.rep.summary()), NOW_MS);
    let before = a.lib.clone();
    let victim = a.lib.scores[&sid]
        .annotations
        .values()
        .flatten()
        .next()
        .unwrap()
        .id;
    for list in a.lib.scores.get_mut(&sid).unwrap().annotations.values_mut() {
        list.retain(|x| x.id != victim);
    }
    let del = a.edit(&[Record::Score(sid)], NOW_MS + 1);
    b.get(&del, NOW_MS + 2);
    assert!(b.rep.is_removed(victim));
    // Undo puts the same marking back.
    a.lib = before.clone();
    let undo = a.edit(&[Record::Score(sid)], NOW_MS + 3);
    assert!(matches!(
        undo.as_slice(),
        [Change {
            op: Op::PutAnnotation { revive: true, .. },
            ..
        }]
    ));
    b.get(&undo, NOW_MS + 4);
    assert!(!b.rep.is_removed(victim));
    assert_eq!(normal(&b.lib), normal(&before));
    // The whole score deleted and its deletion undone, likewise.
    a.lib.scores.remove(&sid);
    let gone = a.edit(&[Record::Score(sid)], NOW_MS + 5);
    b.get(&gone, NOW_MS + 6);
    assert!(!b.lib.scores.contains_key(&sid));
    a.lib = before.clone();
    let back = a.edit(&[Record::Score(sid)], NOW_MS + 7);
    b.get(&back, NOW_MS + 8);
    assert_eq!(normal(&b.lib), normal(&before));
}

fn entry(g: &mut IdGen, label: &str) -> ListEntry {
    ListEntry {
        id: g.id(NOW_MS),
        item: EntryItem::Placeholder {
            label: label.into(),
        },
        note: String::new(),
    }
}

fn labels(lib: &Library, list: scorecraft_doc::ListId) -> Vec<String> {
    lib.lists[&list]
        .entries
        .iter()
        .map(|e| match &e.item {
            EntryItem::Placeholder { label } => label.clone(),
            EntryItem::Score { .. } => "score".into(),
        })
        .collect()
}

#[test]
fn sync_t8_list_order_merges_inserts_and_moves() {
    let mut g = IdGen::new(10);
    let mut base = Library::new(g.id(NOW_MS));
    let list: scorecraft_doc::ListId = g.id(NOW_MS);
    let entries = ["A", "B", "C", "D"]
        .iter()
        .map(|l| entry(&mut g, l))
        .collect();
    base.lists.insert(
        list,
        Setlist {
            id: list,
            name: "Gig".into(),
            entries,
            modified_ms: NOW_MS,
        },
    );
    let mut a = Device::join(base.clone(), device(&mut g), NOW_MS);
    let mut b = Device::join(Library::new(base.id), device(&mut g), NOW_MS);
    b.get(&a.rep.changes_for(&b.rep.summary()), NOW_MS);
    assert_eq!(labels(&b.lib, list), ["A", "B", "C", "D"]);
    // a moves D to the front; b inserts X after B and drops C.
    let es = &mut a.lib.lists.get_mut(&list).unwrap().entries;
    let d = es.pop().unwrap();
    es.insert(0, d);
    let ca = a.edit(&[Record::List(list)], NOW_MS + 1);
    assert_eq!(ca.len(), 1, "one move is one change: {ca:?}");
    assert_eq!(labels(a.rep.library(), list), ["D", "A", "B", "C"]);
    let x = entry(&mut g, "X");
    let es = &mut b.lib.lists.get_mut(&list).unwrap().entries;
    es.insert(2, x);
    es.retain(|e| !matches!(&e.item, EntryItem::Placeholder { label } if label == "C"));
    b.edit(&[Record::List(list)], NOW_MS + 2);
    let mut devs = [a, b];
    sync_all(&mut devs, NOW_MS + 3);
    assert_eq!(labels(&devs[0].lib, list), ["D", "A", "B", "X"]);
    assert_eq!(devs[0].lib, devs[1].lib);
}

#[test]
fn sync_t9_a_replica_saved_and_loaded_carries_on() {
    let mut g = IdGen::new(11);
    let lib = sample_library(6, 2);
    let a = Device::join(lib.clone(), device(&mut g), NOW_MS);
    let state = a.rep.state();
    let text = serde_json::to_string(&state).unwrap();
    let back: ReplicaState = serde_json::from_str(&text).unwrap();
    let loaded = Replica::load(a.rep.device(), a.rep.library().clone(), back);
    assert_eq!(loaded.summary(), a.rep.summary());
    assert_eq!(loaded.log_len(), a.rep.log_len());
    let mut a2 = Device {
        lib: a.lib.clone(),
        rep: loaded,
    };
    let sid = *lib.scores.keys().next().unwrap();
    a2.lib.scores.get_mut(&sid).unwrap().practice_notes = "slow".into();
    let c = a2.edit(&[Record::Score(sid)], NOW_MS + 1);
    assert_eq!(c.len(), 1);
    assert!(c[0].id.seq > a.rep.summary()[&a.rep.device()]);
}

/// One step of the property test, on device `d`.
#[derive(Clone, Debug)]
enum Step {
    Draw {
        d: usize,
        score: usize,
        page: u32,
    },
    Recolour {
        d: usize,
        pick: usize,
    },
    Move {
        d: usize,
        pick: usize,
        page: u32,
    },
    Delete {
        d: usize,
        pick: usize,
    },
    /// Put back the last marking this device deleted (an undo).
    Undo {
        d: usize,
    },
    Rename {
        d: usize,
        score: usize,
        name: u8,
    },
    Bookmark {
        d: usize,
        score: usize,
        page: u32,
    },
    Unbookmark {
        d: usize,
        score: usize,
    },
    Folder {
        d: usize,
        name: u8,
    },
    Unfolder {
        d: usize,
        pick: usize,
    },
    Trash {
        d: usize,
        score: usize,
    },
    Setting {
        d: usize,
        value: u8,
    },
    ListAdd {
        d: usize,
        at: usize,
    },
    ListMove {
        d: usize,
        from: usize,
        to: usize,
    },
    ListDrop {
        d: usize,
        at: usize,
    },
    /// Send some of what `from` has to `to` (a partial, out-of-order exchange).
    Send {
        from: usize,
        to: usize,
        keep: u8,
    },
    Wait {
        ms: u16,
    },
}

impl Step {
    fn device(&self) -> usize {
        match self {
            Step::Draw { d, .. }
            | Step::Recolour { d, .. }
            | Step::Move { d, .. }
            | Step::Delete { d, .. }
            | Step::Undo { d }
            | Step::Rename { d, .. }
            | Step::Bookmark { d, .. }
            | Step::Unbookmark { d, .. }
            | Step::Folder { d, .. }
            | Step::Unfolder { d, .. }
            | Step::Trash { d, .. }
            | Step::Setting { d, .. }
            | Step::ListAdd { d, .. }
            | Step::ListMove { d, .. }
            | Step::ListDrop { d, .. } => *d,
            Step::Send { .. } | Step::Wait { .. } => 0,
        }
    }
}

fn step() -> impl Strategy<Value = Step> {
    let d = 0..3usize;
    prop_oneof![
        (d.clone(), 0..3usize, 0..3u32).prop_map(|(d, score, page)| Step::Draw { d, score, page }),
        (d.clone(), any::<usize>()).prop_map(|(d, pick)| Step::Recolour { d, pick }),
        (d.clone(), any::<usize>(), 0..3u32).prop_map(|(d, pick, page)| Step::Move {
            d,
            pick,
            page
        }),
        (d.clone(), any::<usize>()).prop_map(|(d, pick)| Step::Delete { d, pick }),
        d.clone().prop_map(|d| Step::Undo { d }),
        (d.clone(), 0..3usize, any::<u8>()).prop_map(|(d, score, name)| Step::Rename {
            d,
            score,
            name
        }),
        (d.clone(), 0..3usize, 0..3u32).prop_map(|(d, score, page)| Step::Bookmark {
            d,
            score,
            page
        }),
        (d.clone(), 0..3usize).prop_map(|(d, score)| Step::Unbookmark { d, score }),
        (d.clone(), any::<u8>()).prop_map(|(d, name)| Step::Folder { d, name }),
        (d.clone(), any::<usize>()).prop_map(|(d, pick)| Step::Unfolder { d, pick }),
        (d.clone(), 0..3usize).prop_map(|(d, score)| Step::Trash { d, score }),
        (d.clone(), any::<u8>()).prop_map(|(d, value)| Step::Setting { d, value }),
        (d.clone(), any::<usize>()).prop_map(|(d, at)| Step::ListAdd { d, at }),
        (d.clone(), any::<usize>(), any::<usize>()).prop_map(|(d, from, to)| Step::ListMove {
            d,
            from,
            to
        }),
        (d.clone(), any::<usize>()).prop_map(|(d, at)| Step::ListDrop { d, at }),
        (d.clone(), 0..3usize, any::<u8>()).prop_map(|(from, to, keep)| Step::Send {
            from,
            to,
            keep
        }),
        (0..500u16).prop_map(|ms| Step::Wait { ms }),
    ]
}

type Mark = (ScoreId, PageKey, Annotation);

fn marks(lib: &Library) -> Vec<Mark> {
    lib.scores
        .iter()
        .flat_map(|(sid, s)| {
            s.annotations
                .iter()
                .flat_map(move |(p, l)| l.iter().map(move |a| (*sid, *p, a.clone())))
        })
        .collect()
}

/// Apply one editing step to `lib`; returns the records it touched.
fn edit(
    s: &Step,
    lib: &mut Library,
    g: &mut IdGen,
    scores: &[ScoreId],
    list: scorecraft_doc::ListId,
    deleted: &mut Vec<Mark>,
    now: u64,
) -> Vec<Record> {
    let mut touched = Vec::new();
    match *s {
        Step::Draw { score, page, .. } => {
            let sid = scores[score];
            if let Some(sc) = lib.scores.get_mut(&sid) {
                let a = stroke(g, sc.layers[0].id, page as f32 * 10.0);
                sc.annotations.entry(PageKey(page)).or_default().push(a);
                touched.push(Record::Score(sid));
            }
        }
        Step::Recolour { pick, .. } | Step::Move { pick, .. } | Step::Delete { pick, .. } => {
            let m = marks(lib);
            if let Some((sid, p, a)) = m.get(pick % m.len().max(1)).cloned() {
                let sc = lib.scores.get_mut(&sid).unwrap();
                for l in sc.annotations.values_mut() {
                    l.retain(|x| x.id != a.id);
                }
                sc.annotations.retain(|_, l| !l.is_empty());
                match *s {
                    Step::Recolour { .. } => sc
                        .annotations
                        .entry(p)
                        .or_default()
                        .push(recolour(a, Color::RED)),
                    Step::Move { page, .. } => {
                        sc.annotations.entry(PageKey(page)).or_default().push(a)
                    }
                    _ => deleted.push((sid, p, a)),
                }
                touched.push(Record::Score(sid));
            }
        }
        Step::Undo { .. } => {
            if let Some((sid, p, a)) = deleted.pop()
                && let Some(sc) = lib.scores.get_mut(&sid)
                && sc.annotations.values().flatten().all(|x| x.id != a.id)
            {
                sc.annotations.entry(p).or_default().push(a);
                touched.push(Record::Score(sid));
            }
        }
        Step::Rename { score, name, .. } => {
            if let Some(sc) = lib.scores.get_mut(&scores[score]) {
                sc.metadata.title = format!("Title {name}");
                sc.modified_ms = now;
                touched.push(Record::Score(scores[score]));
            }
        }
        Step::Bookmark { score, page, .. } => {
            if let Some(sc) = lib.scores.get_mut(&scores[score]) {
                sc.bookmarks.push(Bookmark {
                    id: g.id(NOW_MS),
                    name: "Coda".into(),
                    page: PageKey(page),
                });
                touched.push(Record::Score(scores[score]));
            }
        }
        Step::Unbookmark { score, .. } => {
            if let Some(sc) = lib.scores.get_mut(&scores[score])
                && !sc.bookmarks.is_empty()
            {
                sc.bookmarks.remove(0);
                touched.push(Record::Score(scores[score]));
            }
        }
        Step::Folder { name, .. } => {
            let id = g.id(NOW_MS);
            lib.folders.insert(
                id,
                Folder {
                    id,
                    name: format!("F{name}"),
                    parent: None,
                },
            );
            touched.push(Record::Library);
        }
        Step::Unfolder { pick, .. } => {
            let ids: Vec<_> = lib.folders.keys().copied().collect();
            if let Some(id) = ids.get(pick % ids.len().max(1)) {
                lib.folders.remove(id);
                touched.push(Record::Library);
            }
        }
        Step::Trash { score, .. } => {
            let sid = scores[score];
            if lib.trash.remove(&sid).is_none() {
                lib.trash.insert(sid, now);
            }
            touched.push(Record::Library);
        }
        Step::Setting { value, .. } => {
            lib.settings.insert("turn".into(), format!("{value}"));
            touched.push(Record::Library);
        }
        Step::ListAdd { at, .. } => {
            if let Some(l) = lib.lists.get_mut(&list) {
                let i = at % (l.entries.len() + 1);
                l.entries.insert(i, entry(g, "new"));
                touched.push(Record::List(list));
            }
        }
        Step::ListMove { from, to, .. } => {
            if let Some(l) = lib.lists.get_mut(&list)
                && !l.entries.is_empty()
            {
                let e = l.entries.remove(from % l.entries.len());
                let i = to % (l.entries.len() + 1);
                l.entries.insert(i, e);
                touched.push(Record::List(list));
            }
        }
        Step::ListDrop { at, .. } => {
            if let Some(l) = lib.lists.get_mut(&list)
                && !l.entries.is_empty()
            {
                let i = at % l.entries.len();
                l.entries.remove(i);
                touched.push(Record::List(list));
            }
        }
        Step::Send { .. } | Step::Wait { .. } => {}
    }
    touched
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, ..ProptestConfig::default() })]

    #[test]
    fn sync_t4_three_devices_converge(steps in proptest::collection::vec(step(), 1..60)) {
        let lib = sample_library(11, 3);
        let scores: Vec<ScoreId> = lib.scores.keys().copied().collect();
        let list = *lib.lists.keys().next().unwrap();
        let mut g = IdGen::new(1234);
        // Device 0 has the library; 1 and 2 join empty and receive it.
        let mut devs = vec![Device::join(lib.clone(), device(&mut g), NOW_MS)];
        for _ in 0..2 {
            devs.push(Device::join(Library::new(lib.id), device(&mut g), NOW_MS));
        }
        sync_all(&mut devs, NOW_MS);
        // Each device's wall clock runs a little differently.
        let mut now = [NOW_MS, NOW_MS + 7, NOW_MS - 30];
        let mut deleted: Vec<Vec<Mark>> = vec![Vec::new(); 3];
        for s in steps {
            let d = s.device();
            match s {
                Step::Send { from, to, keep } => {
                    if from != to {
                        let mut out = devs[from].rep.changes_for(&devs[to].rep.summary());
                        // Keep a pseudo-random subset, newest first.
                        out.reverse();
                        let out: Vec<Change> = out
                            .into_iter()
                            .enumerate()
                            .filter(|(i, _)| (keep >> (i % 8)) & 1 == 1)
                            .map(|(_, c)| c)
                            .collect();
                        devs[to].get(&out, now[to]);
                    }
                }
                Step::Wait { ms } => {
                    for n in &mut now {
                        *n += u64::from(ms);
                    }
                }
                _ => {
                    let touched = edit(&s, &mut devs[d].lib, &mut g, &scores, list, &mut deleted[d], now[d]);
                    if !touched.is_empty() {
                        devs[d].edit(&touched, now[d]);
                        // The replica now holds exactly what the device edited.
                        let all: BTreeSet<Record> = touched.iter().copied().collect();
                        let mut mirror = devs[d].lib.clone();
                        devs[d].rep.copy_into(&mut mirror, &all);
                        prop_assert_eq!(normal(&mirror), normal(&devs[d].lib));
                    }
                }
            }
            for n in &mut now {
                *n += 1;
            }
        }
        sync_all(&mut devs, now[0]);
        prop_assert_eq!(devs[0].rep.library(), devs[1].rep.library());
        prop_assert_eq!(devs[1].rep.library(), devs[2].rep.library());
        prop_assert_eq!(normal(&devs[0].lib), normal(devs[0].rep.library()));
        prop_assert_eq!(normal(&devs[1].lib), normal(&devs[2].lib));
        prop_assert_eq!(devs[0].rep.summary(), devs[2].rep.summary());
        for dv in &devs {
            prop_assert_eq!(dv.rep.pending(), 0);
            // Nothing left to record once everything is in.
            let mut all = vec![Record::Library, Record::List(list)];
            all.extend(scores.iter().map(|s| Record::Score(*s)));
            prop_assert!(dv.rep.diff(&dv.lib, &all).is_empty());
        }
    }
}
