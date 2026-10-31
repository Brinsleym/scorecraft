//! Sync through a shared folder (`docs/spec/sync.md`, SYNC-T10..T13).
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use serde_json::{Value, json};

use scorecraft_format::MemStorage;
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};

use crate::Session;

fn run(s: &mut Session, id: &str, p: Value) -> Value {
    s.dispatch(id, &p)
        .unwrap_or_else(|e| panic!("{id} {p}: {e} ({})", e.code()))
}

fn device(name: &str, folder: &MemStorage, t: u64) -> Session {
    let mut s = Session::new();
    s.set_clock(Box::new(move || 1_793_404_800_000 + t));
    s.create_memory_library().unwrap();
    s.set_sync_storage(Box::new(folder.clone()), name).unwrap();
    s
}

fn import(s: &mut Session, name: &str) -> String {
    let dir = std::env::temp_dir().join(format!("scorecraft-sync-{name}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{name}.pdf"));
    std::fs::write(
        &path,
        PdfBuilder::new().pages(2, PageSpec::default()).build(),
    )
    .unwrap();
    let r = run(
        s,
        "library.import.files",
        json!({ "paths": [path.to_string_lossy()] }),
    );
    r["imported"][0]["score"].as_str().unwrap().to_owned()
}

fn stroke(s: &mut Session, score: &str, x: f64) -> Value {
    run(s, "score.open", json!({ "score": score }));
    run(
        s,
        "annotate.stroke",
        json!({ "points": [[x, 100.0], [x + 40.0, 140.0], [x + 80.0, 120.0]], "space": "page", "page": 1 }),
    )
}

fn titles(s: &Session) -> Vec<String> {
    let mut t: Vec<String> = s
        .library()
        .unwrap()
        .scores
        .values()
        .map(|x| x.display_title().to_owned())
        .collect();
    t.sort();
    t
}

fn marks(s: &Session, score: &str) -> usize {
    let id: scorecraft_doc::ScoreId = score.parse().unwrap();
    s.library().unwrap().scores[&id]
        .annotations
        .values()
        .map(Vec::len)
        .sum()
}

#[test]
fn sync_t10_a_score_and_its_ink_reach_another_device_through_a_folder() {
    let folder = MemStorage::new();
    let mut a = device("a", &folder, 0);
    let mut b = device("b", &folder, 5);
    let id = import(&mut a, "etude");
    stroke(&mut a, &id, 100.0);
    let r = run(&mut a, "sync.now", json!({}));
    assert!(r["sent"].as_u64().unwrap() >= 2, "{r}");
    assert_eq!(r["uploaded"], 1);
    let r = run(&mut b, "sync.now", json!({}));
    assert!(r["received"].as_u64().unwrap() >= 2, "{r}");
    assert_eq!(
        (r["downloaded"].as_u64(), r["waitingFiles"].as_u64()),
        (Some(1), Some(0))
    );
    assert_eq!(titles(&b), titles(&a));
    assert_eq!(marks(&b, &id), 1);
    // b can open and draw it: the file came too.
    run(&mut b, "score.open", json!({ "score": id }));
    run(
        &mut b,
        "view.render",
        json!({ "page": 1, "dpi": 18, "out": std::env::temp_dir().join(format!("scorecraft-sync-b-{}.png", std::process::id())).to_string_lossy() }),
    );
    stroke(&mut b, &id, 300.0);
    run(
        &mut b,
        "score.metadata.set",
        json!({ "score": id, "title": "Étude in E" }),
    );
    run(&mut b, "sync.now", json!({}));
    run(&mut a, "sync.now", json!({}));
    assert_eq!(marks(&a, &id), 2);
    assert_eq!(titles(&a), ["Étude in E"]);
    assert_eq!(
        a.library(),
        b.library()
            .map(|l| {
                let mut l = l.clone();
                l.id = a.library().unwrap().id;
                l
            })
            .as_ref()
    );
    // Nothing new: nothing sent, nothing received.
    let r = run(&mut a, "sync.now", json!({}));
    assert_eq!(
        (r["sent"].as_u64(), r["received"].as_u64()),
        (Some(0), Some(0))
    );
}

#[test]
fn sync_t11_offline_edits_on_both_merge() {
    let folder = MemStorage::new();
    let mut a = device("a", &folder, 0);
    let mut b = device("b", &folder, 3);
    let id = import(&mut a, "sonata");
    run(&mut a, "sync.now", json!({}));
    run(&mut b, "sync.now", json!({}));
    // Both draw offline; a deletes the score's first page from view, b renames it.
    stroke(&mut a, &id, 50.0);
    stroke(&mut b, &id, 400.0);
    run(
        &mut b,
        "score.metadata.set",
        json!({ "score": id, "composer": "Hensel" }),
    );
    run(&mut a, "sync.now", json!({}));
    run(&mut b, "sync.now", json!({}));
    run(&mut a, "sync.now", json!({}));
    assert_eq!(marks(&a, &id), 2);
    assert_eq!(marks(&b, &id), 2);
    let sid: scorecraft_doc::ScoreId = id.parse().unwrap();
    assert_eq!(
        a.library().unwrap().scores[&sid].metadata.composer,
        "Hensel"
    );
    let st = run(&mut a, "sync.status", json!({}));
    assert_eq!(
        (st["on"].as_bool(), st["pending"].as_u64()),
        (Some(true), Some(0))
    );
}

#[test]
fn sync_t12_sync_survives_closing_the_library() {
    let dir = std::env::temp_dir().join(format!("scorecraft-sync-reopen-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (lib_dir, shared) = (dir.join("lib"), dir.join("shared"));
    std::fs::create_dir_all(&shared).unwrap();
    let mut a = Session::new();
    run(
        &mut a,
        "library.create",
        json!({ "path": lib_dir.to_string_lossy() }),
    );
    run(
        &mut a,
        "sync.backend.set",
        json!({ "kind": "folder", "path": shared.to_string_lossy() }),
    );
    let id = import(&mut a, "reopen");
    let before = run(&mut a, "sync.status", json!({}));
    let device = before["device"].clone();
    drop(a);
    let mut a = Session::new();
    run(
        &mut a,
        "library.open",
        json!({ "path": lib_dir.to_string_lossy() }),
    );
    let st = run(&mut a, "sync.status", json!({}));
    assert_eq!(st["device"], device, "the device keeps its id");
    assert_eq!(st["changes"], before["changes"], "the log was rebuilt");
    assert_eq!(st["backend"], json!(shared.to_string_lossy()));
    stroke(&mut a, &id, 10.0);
    let r = run(&mut a, "sync.now", json!({}));
    assert!(r["sent"].as_u64().unwrap() >= 2);
    // A second device on the same folder gets everything.
    let mut b = Session::new();
    b.create_memory_library().unwrap();
    run(
        &mut b,
        "sync.backend.set",
        json!({ "kind": "folder", "path": shared.to_string_lossy() }),
    );
    run(&mut b, "sync.now", json!({}));
    assert_eq!(marks(&b, &id), 1);
    let blobs = std::fs::read_dir(shared.join("blobs")).unwrap().count();
    assert_eq!(blobs, 1);
}

#[test]
fn sync_t13_bad_params_and_a_damaged_block_are_refused_politely() {
    let folder = MemStorage::new();
    let mut a = device("a", &folder, 0);
    for p in [
        json!({ "kind": "cloud" }),
        json!({ "kind": "folder" }),
        json!({ "kind": "none", "path": "/x" }),
        json!({ "kind": "folder", "path": "/definitely/not/here" }),
    ] {
        assert!(a.dispatch("sync.backend.set", &p).is_err(), "{p}");
    }
    let mut f = folder.clone();
    use scorecraft_format::Storage;
    let other: scorecraft_doc::DeviceId = scorecraft_doc::IdGen::new(3).id(1);
    f.write_atomic(
        &format!("changes/{other}/00000000000000000001-00000000000000000002.json"),
        b"{not json",
    )
    .unwrap();
    let r = run(&mut a, "sync.now", json!({}));
    assert_eq!(r["received"], 0);
    let st = run(&mut a, "sync.status", json!({}));
    assert_eq!(st["problems"].as_array().unwrap().len(), 1, "{st}");
    // Without a library, sync is greyed out.
    let mut none = Session::new();
    assert!(none.dispatch("sync.now", &json!({})).is_err());
}
