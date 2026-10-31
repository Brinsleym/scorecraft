//! Merge rules and convergence (`docs/spec/sync.md`, SYNC-T1..T5).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use proptest::prelude::*;

use scorecraft_doc::{
    Annotation, AnnotationKind, Color, DeviceId, IdGen, InkPoint, Library, PageKey, ScoreId,
    Stroke, Tool,
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

fn stroke(g: &mut IdGen, layer: scorecraft_doc::LayerId, x: f32) -> Annotation {
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

fn layer(lib: &Library, score: ScoreId) -> scorecraft_doc::LayerId {
    lib.scores[&score].layers[0].id
}

fn put(score: ScoreId, page: u32, annotation: Annotation) -> Op {
    Op::PutAnnotation {
        score,
        page: PageKey(page),
        annotation,
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
    let mut c = Replica::new(device(&mut g), a.library().clone());
    c.receive(&a.changes_for(&Summary::new()), NOW_MS + 11);
    assert_eq!(c.receive(std::slice::from_ref(&recoloured), NOW_MS + 12), 0);
    a.receive(&[recoloured], NOW_MS + 12);
    assert!(gone(&a) && gone(&c));
}

#[test]
fn sync_t3_two_renames_offline_keep_the_later_one() {
    let (mut a, mut b, score, _) = pair();
    let mut ma = a.library().scores[&score].metadata.clone();
    ma.title = "From A".into();
    let mut mb = ma.clone();
    mb.title = "From B".into();
    a.local(
        Op::SetMetadata {
            score,
            metadata: ma,
        },
        NOW_MS + 100,
    );
    b.local(
        Op::SetMetadata {
            score,
            metadata: mb,
        },
        NOW_MS + 50,
    );
    exchange(&mut a, &mut b, NOW_MS + 200);
    assert_eq!(a.library(), b.library());
    assert_eq!(a.library().scores[&score].metadata.title, "From A");
    // After hearing of A's rename, B's next rename is later even if B's wall clock is behind.
    let mut mb2 = a.library().scores[&score].metadata.clone();
    mb2.title = "B again".into();
    b.local(
        Op::SetMetadata {
            score,
            metadata: mb2,
        },
        NOW_MS + 60,
    );
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
    let mut m = a.library().scores[&score].metadata.clone();
    m.composer = "Clara Schumann".into();
    changes.push(a.local(Op::SetMetadata { score, metadata: m }, NOW_MS + 3));
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
    // The stroke moved to page 3 in blue; the second stroke is gone.
    let on2 = shuffled.library().scores[&score].annotations_on(PageKey(2));
    assert!(on2.iter().any(|x| x.id == s.id));
    assert!(
        shuffled.library().scores[&score]
            .annotations_on(PageKey(0))
            .iter()
            .all(|x| x.id != s.id)
    );
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
    b.receive(&[add], NOW_MS + 4);
    assert_eq!(b.pending(), 0);
    assert_eq!(a.library(), b.library());
}

#[test]
fn changes_travel_as_json() {
    let (mut a, _, score, mut g) = pair();
    let l = layer(a.library(), score);
    let c = a.local(put(score, 0, stroke(&mut g, l, 1.0)), NOW_MS);
    let text = serde_json::to_string(&c).unwrap();
    assert!(text.contains("\"op\":\"putAnnotation\""));
    let back: Change = serde_json::from_str(&text).unwrap();
    assert_eq!(back, c);
}

/// One step of the property test.
#[derive(Clone, Debug)]
enum Step {
    Draw {
        r: usize,
        score: usize,
        page: u32,
    },
    Recolour {
        r: usize,
        pick: usize,
        red: bool,
    },
    Move {
        r: usize,
        pick: usize,
        page: u32,
    },
    Delete {
        r: usize,
        pick: usize,
    },
    Rename {
        r: usize,
        score: usize,
        name: u8,
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

fn step() -> impl Strategy<Value = Step> {
    prop_oneof![
        (0..3usize, 0..3usize, 0..3u32).prop_map(|(r, score, page)| Step::Draw { r, score, page }),
        (0..3usize, any::<usize>(), any::<bool>()).prop_map(|(r, pick, red)| Step::Recolour {
            r,
            pick,
            red
        }),
        (0..3usize, any::<usize>(), 0..3u32).prop_map(|(r, pick, page)| Step::Move {
            r,
            pick,
            page
        }),
        (0..3usize, any::<usize>()).prop_map(|(r, pick)| Step::Delete { r, pick }),
        (0..3usize, 0..3usize, any::<u8>()).prop_map(|(r, score, name)| Step::Rename {
            r,
            score,
            name
        }),
        (0..3usize, 0..3usize, any::<u8>()).prop_map(|(from, to, keep)| Step::Send {
            from,
            to,
            keep
        }),
        (0..500u16).prop_map(|ms| Step::Wait { ms }),
    ]
}

/// The markings on a replica's library, with their score and page, for picking one.
fn markings(r: &Replica) -> Vec<(ScoreId, PageKey, Annotation)> {
    r.library()
        .scores
        .iter()
        .flat_map(|(sid, s)| {
            s.annotations
                .iter()
                .flat_map(move |(p, list)| list.iter().map(move |a| (*sid, *p, a.clone())))
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 96, ..ProptestConfig::default() })]

    #[test]
    fn sync_t4_three_replicas_converge(steps in proptest::collection::vec(step(), 1..60)) {
        let lib = sample_library(11, 3);
        let scores: Vec<ScoreId> = lib.scores.keys().copied().collect();
        let mut g = IdGen::new(1234);
        let mut reps: Vec<Replica> =
            (0..3).map(|_| Replica::new(device(&mut g), lib.clone())).collect();
        // Each replica's wall clock runs a little differently.
        let mut now = [NOW_MS, NOW_MS + 7, NOW_MS - 30];
        for s in steps {
            match s {
                Step::Draw { r, score, page } => {
                    let sid = scores[score];
                    let l = layer(reps[r].library(), sid);
                    let a = stroke(&mut g, l, page as f32 * 10.0);
                    reps[r].local(put(sid, page, a), now[r]);
                }
                Step::Recolour { r, pick, red } => {
                    let m = markings(&reps[r]);
                    if let Some((sid, p, a)) = m.get(pick % m.len().max(1)).cloned() {
                        let c = if red { Color::RED } else { Color::BLUE };
                        reps[r].local(put(sid, p.0, recolour(a, c)), now[r]);
                    }
                }
                Step::Move { r, pick, page } => {
                    let m = markings(&reps[r]);
                    if let Some((sid, _, a)) = m.get(pick % m.len().max(1)).cloned() {
                        reps[r].local(put(sid, page, a), now[r]);
                    }
                }
                Step::Delete { r, pick } => {
                    let m = markings(&reps[r]);
                    if let Some((sid, _, a)) = m.get(pick % m.len().max(1)).cloned() {
                        reps[r].local(Op::RemoveAnnotation { score: sid, annotation: a.id }, now[r]);
                    }
                }
                Step::Rename { r, score, name } => {
                    let sid = scores[score];
                    let mut md = reps[r].library().scores[&sid].metadata.clone();
                    md.title = format!("Title {name}");
                    reps[r].local(Op::SetMetadata { score: sid, metadata: md }, now[r]);
                }
                Step::Send { from, to, keep } => {
                    if from != to {
                        let mut out = reps[from].changes_for(&reps[to].summary());
                        // Keep a pseudo-random subset, newest first.
                        out.reverse();
                        let out: Vec<Change> = out
                            .into_iter()
                            .enumerate()
                            .filter(|(i, _)| (keep >> (i % 8)) & 1 == 1)
                            .map(|(_, c)| c)
                            .collect();
                        reps[to].receive(&out, now[to]);
                    }
                }
                Step::Wait { ms } => {
                    for n in &mut now {
                        *n += u64::from(ms);
                    }
                }
            }
            for n in &mut now {
                *n += 1;
            }
        }
        // Everyone exchanges with everyone.
        for _ in 0..2 {
            for i in 0..3 {
                for j in 0..3 {
                    if i != j {
                        let out = reps[i].changes_for(&reps[j].summary());
                        reps[j].receive(&out, now[j]);
                    }
                }
            }
        }
        prop_assert_eq!(reps[0].library(), reps[1].library());
        prop_assert_eq!(reps[1].library(), reps[2].library());
        prop_assert_eq!(reps[0].summary(), reps[2].summary());
        for r in &reps {
            prop_assert_eq!(r.pending(), 0);
        }
    }
}
