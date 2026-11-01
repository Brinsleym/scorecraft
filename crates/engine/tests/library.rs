//! The M1 library workflow end to end, driven only through command ids (as an agent would):
//! create a library, import, edit metadata, folders, pages, bookmarks and setlists, undo and
//! redo, close, reopen with everything intact, verify and repair.
#![allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing)]

use std::path::{Path, PathBuf};

use scorecraft_engine::Session;
use scorecraft_testkit::pdf::{PageSpec, PdfBuilder};
use serde_json::{Value, json};

fn temp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("scorecraft-engine-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn pdf(dir: &Path, name: &str, pages: usize, title: &str, composer: &str) -> String {
    let mut b = PdfBuilder::new().pages(pages, PageSpec::default());
    if !title.is_empty() {
        b = b.title(title).composer(composer);
    }
    let path = dir.join(name);
    std::fs::write(&path, b.build()).unwrap();
    path.to_string_lossy().into_owned()
}

fn run(s: &mut Session, id: &str, params: Value) -> Value {
    match s.dispatch(id, &params) {
        Ok(v) => v,
        Err(e) => panic!("{id} {params}: {} ({})", e, e.code()),
    }
}

fn fails(s: &mut Session, id: &str, params: Value) -> String {
    s.dispatch(id, &params).unwrap_err().code().to_owned()
}

fn fixed_clock(s: &mut Session) {
    s.set_clock(Box::new(|| 1_793_404_800_000));
}

#[test]
fn library_workflow_survives_reopen() {
    let dir = temp("workflow");
    let lib_path = dir.join("Music.scorecraft").to_string_lossy().into_owned();
    let a = pdf(&dir, "sonata.pdf", 4, "Sonata in A", "W. A. Mozart");
    let a_bytes = std::fs::read(&a).unwrap();
    let b = pdf(&dir, "etude.pdf", 3, "", "");
    let junk = dir.join("notes.txt");
    std::fs::write(&junk, "not a score").unwrap();

    let mut s = Session::new();
    fixed_clock(&mut s);
    assert_eq!(fails(&mut s, "library.scores", json!({})), "disabled");
    run(&mut s, "library.create", json!({ "path": lib_path }));
    assert_eq!(
        fails(&mut s, "library.create", json!({ "path": lib_path })),
        "library_exists"
    );

    // Import: two scores and one failure reported, not fatal.
    let r = run(
        &mut s,
        "library.import.files",
        json!({ "paths": [a, b, junk.to_string_lossy()] }),
    );
    assert_eq!(r["imported"].as_array().unwrap().len(), 2);
    assert_eq!(r["failed"][0]["code"], "invalid_pdf");
    assert_eq!(r["undo"], "Import 2 Scores");
    let sonata = r["imported"][0]["score"].as_str().unwrap().to_owned();
    let etude = r["imported"][1]["score"].as_str().unwrap().to_owned();
    assert_eq!(r["imported"][0]["title"], "Sonata in A");
    assert_eq!(
        r["imported"][1]["title"], "etude",
        "untitled PDFs take the file name"
    );

    // Metadata.
    run(
        &mut s,
        "score.metadata.set",
        json!({ "score": etude, "title": "Étude Op. 10 No. 1", "composer": "Chopin",
                "tags": ["recital", "recital", " piano "], "custom": { "Opus": "10" } }),
    );
    let got = run(&mut s, "library.score.get", json!({ "score": etude }));
    assert_eq!(got["metadata"]["tags"], json!(["recital", "piano"]));
    assert_eq!(got["metadata"]["custom"]["Opus"], "10");

    // Folders.
    let horn = run(&mut s, "library.folder.new", json!({ "name": "Horn" }))["folder"]
        .as_str()
        .unwrap()
        .to_owned();
    let sub = run(
        &mut s,
        "library.folder.new",
        json!({ "name": "Études", "parent": horn }),
    )["folder"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
        &mut s,
        "library.score.move",
        json!({ "score": etude, "folder": sub }),
    );
    let in_sub = run(&mut s, "library.scores", json!({ "folder": sub }));
    assert_eq!(in_sub["scores"].as_array().unwrap().len(), 1);

    // Pages: rotate, hide, move, delete, range.
    let r = run(
        &mut s,
        "page.rotate",
        json!({ "score": sonata, "page": 2, "degrees": -90 }),
    );
    assert_eq!(r["rotation"], 270);
    run(
        &mut s,
        "page.setHidden",
        json!({ "score": sonata, "page": 3 }),
    );
    run(
        &mut s,
        "page.move",
        json!({ "score": sonata, "from": 1, "to": 4 }),
    );
    run(
        &mut s,
        "score.pageRange.set",
        json!({ "score": sonata, "first": 1, "last": 3 }),
    );
    let r = run(
        &mut s,
        "page.delete",
        json!({ "score": sonata, "pages": [1] }),
    );
    assert_eq!(r["pages"], 3);
    let got = run(&mut s, "library.score.get", json!({ "score": sonata }));
    // Source pages after: [2 (rotated), 3 (hidden), 0] minus position 0 → [2, 3, 0]… check by
    // source index rather than by hand-computed order.
    let sources: Vec<u64> = got["pages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["source"].as_u64().unwrap())
        .collect();
    assert_eq!(sources, vec![2, 3, 0]);
    assert_eq!(got["pageRange"], json!({ "first": 0, "last": 1 }));
    assert_eq!(
        fails(
            &mut s,
            "page.delete",
            json!({ "score": sonata, "pages": [1, 2, 3] })
        ),
        "invalid_edit"
    );
    assert_eq!(
        fails(&mut s, "page.rotate", json!({ "score": sonata, "page": 9 })),
        "bad_params"
    );

    // Bookmarks and going to one in the reader.
    let bm = run(
        &mut s,
        "score.bookmark.add",
        json!({ "score": sonata, "name": "Coda", "page": 3 }),
    )["bookmark"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        fails(&mut s, "view.bookmark.goto", json!({ "bookmark": bm })),
        "disabled"
    );
    let opened = run(&mut s, "score.open", json!({ "score": sonata }));
    assert_eq!(opened["pageCount"], 4);
    // The reader shows the score's pages within its range (positions 1–2) without the hidden
    // one, with each page's rotation.
    let shown: Vec<(usize, &Value)> = got["pages"].as_array().unwrap()[..2]
        .iter()
        .enumerate()
        .filter(|(_, p)| !p["hidden"].as_bool().unwrap_or(false))
        .collect();
    let (intro_position, first) = shown[0];
    let st = run(&mut s, "view.state", json!({}));
    assert_eq!(st["pageCount"], shown.len());
    assert_eq!(
        st["slots"][0]["filePage"].as_u64(),
        first["source"].as_u64().map(|p| p + 1)
    );
    assert_eq!(
        st["slots"][0]["rotation"].as_i64().unwrap(),
        first["rotation"].as_i64().unwrap_or(0)
    );
    let intro_file_page = first["source"].as_u64().unwrap() + 1;
    // "Coda" is on score page 3, outside the page range.
    assert_eq!(
        fails(&mut s, "view.bookmark.goto", json!({ "bookmark": bm })),
        "bad_params"
    );
    let intro = run(
        &mut s,
        "score.bookmark.add",
        json!({ "score": sonata, "name": "Intro", "page": intro_position + 1 }),
    )["bookmark"]
        .as_str()
        .unwrap()
        .to_owned();
    let r = run(&mut s, "view.bookmark.goto", json!({ "bookmark": intro }));
    assert_eq!(
        (r["page"].as_u64(), r["filePage"].as_u64()),
        (Some(1), Some(intro_file_page))
    );
    let r = run(&mut s, "view.render", json!({ "dpi": 36 }));
    assert_eq!(r["page"], intro_file_page);

    // Setlists.
    let list = run(&mut s, "list.new", json!({ "name": "Recital" }))["list"]
        .as_str()
        .unwrap()
        .to_owned();
    let e1 = run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": etude }),
    )["entry"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
        &mut s,
        "list.entry.addPlaceholder",
        json!({ "list": list, "label": "Interval" }),
    );
    let e3 = run(
        &mut s,
        "list.entry.add",
        json!({ "list": list, "score": sonata, "at": 1 }),
    )["entry"]
        .as_str()
        .unwrap()
        .to_owned();
    run(
        &mut s,
        "list.entry.move",
        json!({ "list": list, "from": 1, "to": 3 }),
    );
    run(
        &mut s,
        "list.entry.tempo.set",
        json!({ "list": list, "entry": e1, "bpm": 96 }),
    );
    run(
        &mut s,
        "list.entry.pageRange.set",
        json!({ "list": list, "entry": e3, "first": 2, "last": 3 }),
    );
    let got = run(&mut s, "list.get", json!({ "list": list }));
    let order: Vec<&str> = got["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["type"].as_str().unwrap())
        .collect();
    assert_eq!(order, vec!["score", "placeholder", "score"]);
    assert_eq!(got["entries"][0]["tempo"], 96.0);
    assert_eq!(
        got["entries"][2]["pageRange"],
        json!({ "first": 1, "last": 2 })
    );
    assert_eq!(got["entries"][0]["title"], "Étude Op. 10 No. 1");

    // Delete and restore; undo and redo.
    run(&mut s, "library.score.delete", json!({ "score": etude }));
    assert_eq!(
        run(&mut s, "library.scores", json!({ "trashed": true }))["scores"][0]["id"],
        etude
    );
    run(&mut s, "library.trash.restore", json!({ "score": etude }));
    let u = run(&mut s, "edit.undo", json!({}));
    assert_eq!(u["undid"], "Restore Score");
    let u = run(&mut s, "edit.redo", json!({}));
    assert_eq!(u["redid"], "Restore Score");
    // The sonata's own undo stack: the newest sonata step is its page range from the list?
    // No: list edits are library steps. The sonata's newest step is the bookmark.
    let u = run(&mut s, "edit.undo", json!({ "score": sonata }));
    assert_eq!(u["undid"], "Add Bookmark");
    run(&mut s, "edit.redo", json!({ "score": sonata }));

    let before = s.library().unwrap().clone();
    assert!(before.is_sound(), "{:?}", before.problems());
    run(&mut s, "library.close", json!({}));
    assert!(!s.has_library());

    // Reopen in a new session: everything intact, the PDFs untouched.
    let mut s2 = Session::new();
    fixed_clock(&mut s2);
    let r = run(&mut s2, "library.open", json!({ "path": lib_path }));
    assert_eq!(r["unreadable"], json!([]));
    assert_eq!(s2.library().unwrap(), &before);
    let v = run(&mut s2, "library.verify", json!({}));
    assert_eq!(v["clean"], true, "{v}");
    assert_eq!(
        std::fs::read(&a).unwrap(),
        a_bytes,
        "the imported file is never touched"
    );

    // Damage a record on disk: the library still opens, verify sees it, repair fixes it.
    run(&mut s2, "library.close", json!({}));
    let scores_dir = Path::new(&lib_path).join("scores");
    let victim = scores_dir.join(format!("{etude}.json"));
    std::fs::write(&victim, "{ broken").unwrap();
    let r = run(&mut s2, "library.open", json!({ "path": lib_path }));
    assert_eq!(r["unreadable"].as_array().unwrap().len(), 1);
    let v = run(&mut s2, "library.verify", json!({}));
    assert_eq!(v["clean"], false);
    let v = run(&mut s2, "library.verify", json!({ "repair": true }));
    assert!(
        !v["repaired"]["fixes"].as_array().unwrap().is_empty()
            || !v["repaired"]["quarantined"].as_array().unwrap().is_empty()
    );
    let v = run(&mut s2, "library.verify", json!({}));
    assert_eq!(v["clean"], true, "{v}");
    assert!(s2.library().unwrap().is_sound());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn create_refuses_folders_with_other_files() {
    let dir = temp("refuse");
    std::fs::write(dir.join("photo.jpg"), b"x").unwrap();
    let mut s = Session::new();
    assert_eq!(
        fails(
            &mut s,
            "library.create",
            json!({ "path": dir.to_string_lossy() })
        ),
        "io_error"
    );
    assert_eq!(
        fails(
            &mut s,
            "library.create",
            json!({ "path": dir.join("photo.jpg").to_string_lossy() })
        ),
        "io_error"
    );
    assert_eq!(
        fails(&mut s, "library.create", json!({ "path": "  " })),
        "io_error"
    );
    assert_eq!(
        fails(
            &mut s,
            "library.open",
            json!({ "path": dir.to_string_lossy() })
        ),
        "not_a_library"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_failed_save_leaves_memory_and_disk_agreeing() {
    let mut s = Session::new();
    let mem = s.create_memory_library().unwrap();
    run(&mut s, "list.new", json!({ "name": "A" }));
    let before = s.library().unwrap().clone();
    mem.crash_after(Some(0));
    assert_eq!(
        fails(&mut s, "list.new", json!({ "name": "B" })),
        "io_error"
    );
    assert_eq!(s.library().unwrap(), &before);
    assert_eq!(fails(&mut s, "edit.redo", json!({})), "disabled");
    mem.crash_after(None);
    run(&mut s, "list.new", json!({ "name": "C" }));
    assert_eq!(s.library().unwrap().lists.len(), 2);
}

#[test]
fn undo_is_disabled_until_something_changes() {
    let mut s = Session::new();
    assert_eq!(s.is_enabled("edit.undo"), Some(false));
    s.create_memory_library().unwrap();
    assert_eq!(s.is_enabled("edit.undo"), Some(false));
    run(&mut s, "library.folder.new", json!({ "name": "F" }));
    assert_eq!(s.is_enabled("edit.undo"), Some(true));
    run(&mut s, "edit.undo", json!({}));
    assert!(s.library().unwrap().folders.is_empty());
    assert_eq!(s.is_enabled("edit.redo"), Some(true));
    assert_eq!(fails(&mut s, "edit.undo", json!({})), "disabled");
    assert_eq!(
        fails(&mut s, "edit.redo", json!({ "scope": "everything" })),
        "bad_params"
    );
}

#[test]
fn opening_a_file_twice_reuses_its_library_score() {
    let dir = temp("openfile");
    let lib_path = dir.join("Music.scorecraft").to_string_lossy().into_owned();
    let a = pdf(&dir, "sonata.pdf", 2, "Sonata", "Mozart");
    let copy = dir.join("copy.pdf");
    std::fs::copy(&a, &copy).unwrap();
    let mut s = Session::new();
    fixed_clock(&mut s);
    assert_eq!(
        fails(&mut s, "library.openFile", json!({ "path": a })),
        "disabled"
    );
    run(&mut s, "library.create", json!({ "path": lib_path }));
    let first = run(&mut s, "library.openFile", json!({ "path": a }));
    assert_eq!(first["imported"], true);
    // The same bytes under another name are the same score.
    let again = run(
        &mut s,
        "library.openFile",
        json!({ "path": copy.to_string_lossy() }),
    );
    assert_eq!(again["imported"], false);
    assert_eq!(again["score"], first["score"]);
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    // Markup works on it: it is a library score.
    let st = run(
        &mut s,
        "annotate.stroke",
        json!({ "points": [[100, 100], [200, 100]], "space": "page", "page": 1 }),
    );
    assert_eq!(st["drawn"], true);
    assert_eq!(
        fails(
            &mut s,
            "library.openFile",
            json!({ "path": dir.join("none.pdf").to_string_lossy() })
        ),
        "io_error"
    );
}

#[test]
fn page_6_manuscript_paper_is_a_new_library_score() {
    let mut s = Session::new();
    fixed_clock(&mut s);
    s.create_memory_library().unwrap();
    let r = run(
        &mut s,
        "score.newManuscript",
        json!({ "paper": "grand", "size": "a4", "pages": 3, "title": "Lesson 4" }),
    );
    assert_eq!(r["imported"], true);
    let st = s.reader_state().unwrap();
    assert_eq!(st["pageCount"], 3);
    let scores = run(&mut s, "library.scores", json!({}));
    assert_eq!(scores["scores"][0]["title"], "Lesson 4");
    assert_eq!(
        fails(&mut s, "score.newManuscript", json!({ "paper": "harp" })),
        "bad_params"
    );
    assert_eq!(
        fails(&mut s, "score.newManuscript", json!({ "pages": 0 })),
        "bad_params"
    );
}

#[test]
fn imp_3_photos_become_a_score() {
    let dir = temp("images");
    let mut paths = Vec::new();
    for (i, (w, h)) in [(60u32, 80u32), (60, 80), (90, 40)].iter().enumerate() {
        let rgba = vec![180u8; (w * h * 4) as usize];
        let png = scorecraft_codecs::png::encode_premultiplied(*w, *h, &rgba).unwrap();
        let p = dir.join(format!("page{i}.png"));
        std::fs::write(&p, png).unwrap();
        paths.push(p.to_string_lossy().into_owned());
    }
    let mut s = Session::new();
    fixed_clock(&mut s);
    s.create_memory_library().unwrap();
    let r = run(
        &mut s,
        "library.import.images",
        json!({ "paths": paths, "title": "Phone scans" }),
    );
    assert_eq!(r["imported"], true);
    assert_eq!(s.reader_state().unwrap()["pageCount"], 3);
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"][0]["title"],
        "Phone scans"
    );
    let junk = dir.join("notes.txt");
    std::fs::write(&junk, "hello").unwrap();
    assert_eq!(
        fails(
            &mut s,
            "library.import.images",
            json!({ "paths": [junk.to_string_lossy()] })
        ),
        "invalid_pdf"
    );
}

/// Recently Deleted keeps folders too (owner, 2026-11-01): a deleted folder takes its folders
/// and scores with it, nothing in it can be changed, a restore brings it all back, and 30 days
/// later it is gone for good. A score left filed in a deleted folder is reported by verify.
#[test]
fn lib_10_folders_go_to_recently_deleted_for_30_days() {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    const DAY: u64 = 24 * 60 * 60 * 1000;
    let dir = temp("deleted-folders");
    let lib_path = dir.join("Music.scorecraft").to_string_lossy().into_owned();
    let a = pdf(&dir, "a.pdf", 2, "Concerto", "Strauss");
    let b = pdf(&dir, "b.pdf", 2, "Etude", "Kopprasch");
    let c = pdf(&dir, "c.pdf", 2, "Loose", "Anon");
    let t0 = 1_793_404_800_000;
    let clock = Arc::new(AtomicU64::new(t0));
    let session = |clock: &Arc<AtomicU64>| {
        let mut s = Session::new();
        let c = clock.clone();
        s.set_clock(Box::new(move || c.load(Ordering::SeqCst)));
        s
    };
    let mut s = session(&clock);
    run(&mut s, "library.create", json!({ "path": lib_path }));
    let r = run(
        &mut s,
        "library.import.files",
        json!({ "paths": [a, b, c] }),
    );
    let id = |i: usize| r["imported"][i]["score"].as_str().unwrap().to_owned();
    let (sa, sb, sc) = (id(0), id(1), id(2));
    let folder = |s: &mut Session, name: &str, parent: Option<&str>| {
        run(
            s,
            "library.folder.new",
            json!({ "name": name, "parent": parent }),
        )["folder"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let horn = folder(&mut s, "Horn", None);
    let etudes = folder(&mut s, "Etudes", Some(&horn));
    run(
        &mut s,
        "library.score.move",
        json!({ "score": sa, "folder": horn }),
    );
    run(
        &mut s,
        "library.score.move",
        json!({ "score": sb, "folder": etudes }),
    );

    let r = run(&mut s, "library.folder.delete", json!({ "folder": horn }));
    assert_eq!(r["trashedScores"], 2);
    let info = run(&mut s, "library.info", json!({}));
    assert_eq!(info["folders"], json!([]));
    assert_eq!(info["deletedFolders"].as_array().unwrap().len(), 2);
    assert_eq!(
        run(&mut s, "library.scores", json!({}))["scores"][0]["id"],
        sc
    );
    assert_eq!(
        run(&mut s, "library.scores", json!({ "trashed": true }))["scores"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    // Read-only while deleted.
    let edit = json!({ "score": sa, "title": "Changed" });
    assert_eq!(fails(&mut s, "score.metadata.set", edit), "invalid_edit");
    assert_eq!(
        fails(
            &mut s,
            "library.folder.rename",
            json!({ "folder": etudes, "name": "X" })
        ),
        "invalid_edit"
    );
    assert_eq!(
        fails(
            &mut s,
            "library.score.move",
            json!({ "score": sc, "folder": horn })
        ),
        "invalid_edit"
    );
    // Restoring the folder brings back its folders and scores, in their places.
    run(&mut s, "library.trash.restore", json!({ "folder": horn }));
    let info = run(&mut s, "library.info", json!({}));
    assert_eq!(info["folders"].as_array().unwrap().len(), 2);
    assert_eq!(info["trashed"], 0);
    let got = run(&mut s, "library.score.get", json!({ "score": sb }));
    assert_eq!(got["folder"], etudes.as_str(), "{got}");

    // Deleted again: 29 days on it is still there; at 30 days it is gone for good.
    run(&mut s, "library.folder.delete", json!({ "folder": horn }));
    run(&mut s, "library.close", json!({}));
    clock.store(t0 + 29 * DAY, Ordering::SeqCst);
    let mut s = session(&clock);
    assert_eq!(
        run(&mut s, "library.open", json!({ "path": lib_path }))["purged"],
        0
    );
    run(&mut s, "library.close", json!({}));

    // Meanwhile another device filed a score in the deleted folder: verify reports it and the
    // folder can be restored. (Made by editing the library record as a merge would leave it.)
    let record = Path::new(&lib_path).join("library.json");
    let mut v: Value = serde_json::from_slice(&std::fs::read(&record).unwrap()).unwrap();
    v["trash"].as_object_mut().unwrap().remove(&sb);
    std::fs::write(&record, serde_json::to_vec(&v).unwrap()).unwrap();
    let mut s = session(&clock);
    run(&mut s, "library.open", json!({ "path": lib_path }));
    let v = run(&mut s, "library.verify", json!({}));
    assert_eq!(v["clean"], false, "{v}");
    assert_eq!(v["inDeletedFolders"][0]["score"], sb.as_str(), "{v}");
    assert_eq!(v["inDeletedFolders"][0]["path"], "Horn/Etudes");
    assert_eq!(v["inDeletedFolders"][0]["restore"], horn.as_str());
    let info = run(&mut s, "library.info", json!({}));
    assert_eq!(info["inDeletedFolders"], v["inDeletedFolders"]);
    run(&mut s, "library.close", json!({}));

    clock.store(t0 + 30 * DAY, Ordering::SeqCst);
    let mut s = session(&clock);
    let r = run(&mut s, "library.open", json!({ "path": lib_path }));
    // The concerto and both folders; the etude was not deleted, so it moves up to the root.
    assert_eq!(r["purged"], 3, "{r}");
    let info = run(&mut s, "library.info", json!({}));
    assert_eq!(info["deletedFolders"], json!([]));
    assert_eq!(info["folders"], json!([]));
    assert_eq!(info["scores"], 2);
    let got = run(&mut s, "library.score.get", json!({ "score": sb }));
    assert!(got["folder"].is_null(), "{got}");
    assert_eq!(run(&mut s, "library.verify", json!({}))["clean"], true);
    let _ = std::fs::remove_dir_all(&dir);
}
