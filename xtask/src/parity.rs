//! `cargo xtask parity [--check]`: reads `docs/feature-map.md`, regenerates
//! `crates/engine/src/feature_catalog.rs` and `docs/parity-checklist.md` against the live engine
//! registry, and fails below `scorecraft_engine::parity::FLOOR`. With `--check`, fails instead of
//! writing when either generated file is stale (what CI runs).

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use scorecraft_engine::Session;

const FEATURE_MAP: &str = "docs/feature-map.md";
const CATALOG: &str = "crates/engine/src/feature_catalog.rs";
const CHECKLIST: &str = "docs/parity-checklist.md";
const SHELL_COMMANDS: &str = "crates/ui-egui/shell-commands.toml";
const HEADER: &str = "| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |";

/// One feature-map row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: String,
    pub area: String,
    pub title: String,
    pub commands: Vec<String>,
    pub priority: String,
    pub status: String,
}

/// Parses every row of every feature table (tables whose header is [`HEADER`]).
pub fn parse_feature_map(text: &str) -> Result<Vec<Row>, String> {
    let mut rows = Vec::new();
    let mut in_table = false;
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line == HEADER {
            in_table = true;
            continue;
        }
        if !line.starts_with('|') {
            in_table = false;
            continue;
        }
        if !in_table || line.starts_with("|---") {
            continue;
        }
        let cells: Vec<&str> = line.trim_matches('|').split('|').map(str::trim).collect();
        let [
            id,
            title,
            _tier,
            _source,
            command,
            _crate,
            priority,
            _est,
            status,
        ] = cells[..]
        else {
            return Err(format!(
                "{FEATURE_MAP}:{}: expected 9 cells, got {}",
                n + 1,
                cells.len()
            ));
        };
        let Some((area, num)) = id.split_once('-') else {
            return Err(format!("{FEATURE_MAP}:{}: bad id `{id}`", n + 1));
        };
        if area.is_empty()
            || !area.chars().all(|c| c.is_ascii_uppercase())
            || num.parse::<u32>().is_err()
        {
            return Err(format!("{FEATURE_MAP}:{}: bad id `{id}`", n + 1));
        }
        // Command ids are the backticked spans of the Command cell; "n/a (...)" rows have none.
        let commands = command
            .split('`')
            .skip(1)
            .step_by(2)
            .map(str::to_owned)
            .filter(|c| !c.is_empty())
            .collect();
        rows.push(Row {
            id: id.to_owned(),
            area: area.to_owned(),
            title: title.to_owned(),
            commands,
            priority: priority.to_owned(),
            status: status.to_owned(),
        });
    }
    let mut ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
    ids.sort_unstable();
    if let Some(w) = ids.windows(2).find(|w| w.first() == w.get(1)) {
        return Err(format!(
            "{FEATURE_MAP}: duplicate id {}",
            w.first().copied().unwrap_or("")
        ));
    }
    Ok(rows)
}

/// Every command the row names is registered (and it names at least one).
fn is_wired(row: &Row, registered: &[&str]) -> bool {
    !row.commands.is_empty()
        && row
            .commands
            .iter()
            .all(|c| registered.contains(&c.as_str()))
}

/// The feature map says the row behaves (`partial` or `done`, see its Status legend).
fn is_implemented(row: &Row) -> bool {
    matches!(row.status.as_str(), "partial" | "done")
}

/// A row is live when every command it names is registered **and** the feature map marks it
/// `partial` or `done`. Registering a stub that returns "unsupported" doesn't raise parity.
pub fn is_live(row: &Row, registered: &[&str]) -> bool {
    is_wired(row, registered) && is_implemented(row)
}

/// Rows the feature map marks `partial` or `done` whose commands aren't all registered: the map
/// claims behaviour the engine doesn't have.
pub fn inconsistencies(rows: &[Row], registered: &[&str]) -> Vec<String> {
    rows.iter()
        .filter(|r| is_implemented(r) && !r.commands.is_empty() && !is_wired(r, registered))
        .map(|r| {
            let missing: Vec<&str> = r
                .commands
                .iter()
                .map(String::as_str)
                .filter(|c| !registered.contains(c))
                .collect();
            format!(
                "{FEATURE_MAP}: {} is `{}` but these commands aren't registered: {}",
                r.id,
                r.status,
                missing.join(", ")
            )
        })
        .collect()
}

/// The shell's `ui.*` command ids from [`SHELL_COMMANDS`].
pub fn shell_commands(text: &str) -> Result<Vec<String>, String> {
    let table: toml::Table = text
        .parse()
        .map_err(|e| format!("parsing {SHELL_COMMANDS}: {e}"))?;
    let list = table
        .get("commands")
        .and_then(toml::Value::as_array)
        .ok_or_else(|| format!("{SHELL_COMMANDS}: needs `commands = [...]`"))?;
    let mut ids = Vec::new();
    for v in list {
        let id = v
            .as_str()
            .ok_or_else(|| format!("{SHELL_COMMANDS}: every command id is a string"))?;
        if !id.starts_with("ui.") {
            return Err(format!(
                "{SHELL_COMMANDS}: `{id}` is not a shell command (shell ids start with `ui.`)"
            ));
        }
        if ids.iter().any(|i| i == id) {
            return Err(format!("{SHELL_COMMANDS}: `{id}` is listed twice"));
        }
        ids.push(id.to_owned());
    }
    Ok(ids)
}

pub fn render_catalog(rows: &[Row]) -> String {
    let mut s = String::from(
        "//! The feature-map rows (`docs/feature-map.md`), the parity denominator.\n\
         //!\n\
         //! Generated by `cargo xtask parity`. Do not edit: change the feature map and regenerate.\n\
         \n\
         /// One feature-map row.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct Feature {\n    \
             pub id: &'static str,\n    \
             pub title: &'static str,\n    \
             /// `P0` (alpha) to `P3`.\n    \
             pub priority: &'static str,\n    \
             /// Engine or shell command ids that implement the row; empty for quality-bar rows.\n    \
             pub commands: &'static [&'static str],\n\
         }\n\
         \n\
         pub const FEATURES: &[Feature] = &[\n",
    );
    for r in rows {
        let cmds: Vec<String> = r.commands.iter().map(|c| format!("{c:?}")).collect();
        let _ = writeln!(
            s,
            "    Feature {{\n        id: {:?},\n        title: {:?},\n        priority: {:?},\n        commands: &[{}],\n    }},",
            r.id,
            r.title,
            r.priority,
            cmds.join(", ")
        );
    }
    s.push_str(
        "];\n\n#[cfg(test)]\nmod tests {\n    #[test]\n    fn ids_are_unique() {\n        \
         let mut ids: Vec<&str> = super::FEATURES.iter().map(|f| f.id).collect();\n        \
         ids.sort_unstable();\n        let n = ids.len();\n        ids.dedup();\n        \
         assert_eq!(n, ids.len());\n    }\n}\n",
    );
    s
}

fn pct(live: usize, total: usize) -> String {
    if total == 0 {
        return "0%".to_owned();
    }
    let p = (live as f64) * 100.0 / (total as f64);
    format!("{p:.0}%")
}

pub fn render_checklist(rows: &[Row], registered: &[&str]) -> String {
    let live_count = rows.iter().filter(|r| is_live(r, registered)).count();
    let total = rows.len();
    let overall = if total == 0 {
        0.0
    } else {
        (live_count as f64) * 100.0 / (total as f64)
    };
    let mut areas: Vec<&str> = Vec::new();
    for r in rows {
        if !areas.contains(&r.area.as_str()) {
            areas.push(&r.area);
        }
    }
    let mut s = String::new();
    s.push_str("# enScore feature parity checklist\n\n");
    s.push_str(
        "Generated by `cargo xtask parity` from [`feature-map.md`](feature-map.md) (via\n\
         `crates/engine/src/feature_catalog.rs`) and the engine's command registry. Do not edit by\n\
         hand: change the feature map or register commands, then regenerate. CI runs\n\
         `cargo xtask parity --check`.\n\n",
    );
    let _ = writeln!(
        s,
        "**{live_count} of {total} features live ({overall:.1}%).**\n"
    );
    s.push_str("| Area | Live | Total | % |\n|---|---:|---:|---:|\n");
    for area in &areas {
        let in_area: Vec<&Row> = rows.iter().filter(|r| r.area == *area).collect();
        let live = in_area.iter().filter(|r| is_live(r, registered)).count();
        let _ = writeln!(
            s,
            "| {area} | {live} | {} | {} |",
            in_area.len(),
            pct(live, in_area.len())
        );
    }
    s.push_str(
        "\n\"Live\" means every command id the row names is registered (in the engine, or for\n\
         `ui.*` ids in `crates/ui-egui/shell-commands.toml`) **and** the feature map marks the row\n\
         `partial` or `done`, which means it behaves as specified with a test. A registered stub\n\
         doesn't count. Rows with no command (quality bars, model rules) are never live here;\n\
         their parity docs track them.\n\n",
    );
    s.push_str("## Live\n\n");
    let live_rows: Vec<&Row> = rows.iter().filter(|r| is_live(r, registered)).collect();
    if live_rows.is_empty() {
        s.push_str("None yet.\n\n");
    } else {
        s.push_str("| Id | Feature | Commands |\n|---|---|---|\n");
        for r in live_rows {
            let _ = writeln!(s, "| {} | {} | {} |", r.id, r.title, code_list(&r.commands));
        }
        s.push('\n');
    }
    let wired: Vec<&Row> = rows
        .iter()
        .filter(|r| is_wired(r, registered) && !is_implemented(r))
        .collect();
    if !wired.is_empty() {
        s.push_str(
            "## Registered, not yet implemented\n\n\
             Their commands exist but the feature map still says `missing`, so they don't count.\n\n\
             | Id | Feature | Commands |\n|---|---|---|\n",
        );
        for r in wired {
            let _ = writeln!(s, "| {} | {} | {} |", r.id, r.title, code_list(&r.commands));
        }
        s.push('\n');
    }
    s.push_str("## Not yet implemented\n\n");
    for area in &areas {
        let missing: Vec<&Row> = rows
            .iter()
            .filter(|r| r.area == *area && !is_live(r, registered) && !is_wired(r, registered))
            .collect();
        if missing.is_empty() {
            continue;
        }
        let _ = writeln!(
            s,
            "### {area}\n\n| Id | P | Feature | Commands |\n|---|---|---|---|"
        );
        for r in missing {
            let cmds = if r.commands.is_empty() {
                "none".to_owned()
            } else {
                code_list(&r.commands)
            };
            let _ = writeln!(s, "| {} | {} | {} | {cmds} |", r.id, r.priority, r.title);
        }
        s.push('\n');
    }
    s
}

fn code_list(cmds: &[String]) -> String {
    let unregistered: Vec<String> = cmds.iter().map(|c| format!("`{c}`")).collect();
    unregistered.join(", ")
}

fn write_or_check(root: &Path, rel: &str, content: &str, check: bool) -> Result<bool, String> {
    let path = root.join(rel);
    // Ignore CRLF from a checkout that converted line endings; the content is what matters.
    let current = fs::read_to_string(&path)
        .unwrap_or_default()
        .replace("\r\n", "\n");
    if current == content {
        return Ok(false);
    }
    if check {
        return Err(format!(
            "{rel} is stale: run `cargo xtask parity` and commit the result"
        ));
    }
    fs::write(&path, content).map_err(|e| format!("writing {rel}: {e}"))?;
    Ok(true)
}

pub fn run(root: &Path, check: bool) -> Result<(), String> {
    let text = fs::read_to_string(root.join(FEATURE_MAP))
        .map_err(|e| format!("reading {FEATURE_MAP}: {e}"))?;
    let rows = parse_feature_map(&text)?;
    let session = Session::new();
    let infos = session.commands();
    let shell_text = fs::read_to_string(root.join(SHELL_COMMANDS))
        .map_err(|e| format!("reading {SHELL_COMMANDS}: {e}"))?;
    let shell = shell_commands(&shell_text)?;
    let mut registered: Vec<&str> = infos.iter().map(|c| c.id).collect();
    registered.extend(shell.iter().map(String::as_str));
    let bad = inconsistencies(&rows, &registered);
    if !bad.is_empty() {
        return Err(bad.join("\n"));
    }

    let catalog = render_catalog(&rows);
    let checklist = render_checklist(&rows, &registered);
    // The catalog is Rust source; keep it rustfmt-clean so `cargo fmt --check` stays green.
    let catalog = rustfmt(&catalog)?;
    let wrote_catalog = write_or_check(root, CATALOG, &catalog, check)?;
    let wrote_checklist = write_or_check(root, CHECKLIST, &checklist, check)?;

    let live = rows.iter().filter(|r| is_live(r, &registered)).count();
    let floor = scorecraft_engine::parity::FLOOR;
    if live < floor {
        return Err(format!(
            "parity {live} is below the floor {floor} (crates/engine/src/parity.rs)"
        ));
    }
    let catalog_rows = scorecraft_engine::feature_catalog::FEATURES.len();
    if !check && wrote_catalog && catalog_rows != rows.len() {
        println!(
            "parity: catalog regenerated ({catalog_rows} → {} rows); rebuild to pick it up",
            rows.len()
        );
    }
    println!(
        "parity: {live} of {} features live (floor {floor}){}{}",
        rows.len(),
        if wrote_catalog {
            "; wrote feature_catalog.rs"
        } else {
            ""
        },
        if wrote_checklist {
            "; wrote parity-checklist.md"
        } else {
            ""
        },
    );
    Ok(())
}

/// Formats generated Rust with the toolchain's rustfmt (stdin → stdout).
fn rustfmt(src: &str) -> Result<String, String> {
    use std::io::Write;
    use std::process::{Command, Stdio};
    let mut child = Command::new("rustfmt")
        .args(["--edition", "2024", "--emit", "stdout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| format!("running rustfmt: {e}"))?;
    child
        .stdin
        .take()
        .ok_or("rustfmt: no stdin")?
        .write_all(src.as_bytes())
        .map_err(|e| format!("rustfmt stdin: {e}"))?;
    let out = child
        .wait_with_output()
        .map_err(|e| format!("rustfmt: {e}"))?;
    if !out.status.success() {
        return Err("rustfmt failed on the generated catalog".to_owned());
    }
    String::from_utf8(out.stdout).map_err(|e| format!("rustfmt output: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAP: &str = "\
## READ

| Id | Feature | Tier | Source | Command | Crate | P | Est | Status |
|---|---|---|---|---|---|---|---:|---|
| READ-1 | Open a score | Free | S1 | `score.open`, `view.render` | pdf | P0 | 10–20 | missing |
| READ-2 | Single page | Free | S1 | `view.layout.single` | layout | P0 | 4–8 | partial |
| READ-14 | Quality | Free | S1 | n/a (quality bar) | pdf | P0 | 1 | missing |

| Id | What PDFCraft provides | Before | After |
|---|---|---|---|
| READ-1 | ignored | a | b |
";

    #[test]
    fn parses_only_feature_tables() {
        let rows = parse_feature_map(MAP).unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].commands, ["score.open", "view.render"]);
        assert!(rows[2].commands.is_empty());
        assert_eq!(rows[1].area, "READ");
    }

    #[test]
    fn rejects_malformed_rows_and_duplicates() {
        let bad = MAP.replace("| P0 | 4–8 | partial |", "| P0 |");
        assert!(parse_feature_map(&bad).is_err());
        let dup = MAP.replace("READ-2", "READ-1");
        assert!(parse_feature_map(&dup).is_err());
    }

    #[test]
    fn live_needs_every_command() {
        let rows = parse_feature_map(MAP).unwrap();
        assert!(!is_live(&rows[1], &[]));
        assert!(is_live(&rows[1], &["view.layout.single"]));
        assert!(!is_live(&rows[2], &["anything"]));
        let doc = render_checklist(&rows, &["view.layout.single"]);
        assert!(doc.contains("**1 of 3 features live (33.3%).**"));
        assert!(doc.contains("| READ | 1 | 3 | 33% |"));
    }

    #[test]
    fn a_registered_stub_is_not_live() {
        let rows = parse_feature_map(MAP).unwrap();
        // READ-1 is `missing`: registering its commands wires it but doesn't count it.
        assert!(!is_live(&rows[0], &["score.open", "view.render"]));
        let doc = render_checklist(&rows, &["score.open", "view.render"]);
        assert!(doc.contains("**0 of 3 features live (0.0%).**"));
        assert!(doc.contains("## Registered, not yet implemented"));
    }

    #[test]
    fn implemented_rows_need_their_commands() {
        let rows = parse_feature_map(MAP).unwrap();
        let bad = inconsistencies(&rows, &[]);
        assert_eq!(bad.len(), 1);
        assert!(bad[0].contains("READ-2 is `partial`"));
        assert!(inconsistencies(&rows, &["view.layout.single"]).is_empty());
    }

    #[test]
    fn shell_commands_must_be_ui_ids() {
        assert_eq!(
            shell_commands("commands = [\"ui.view.fullScreen\"]").unwrap(),
            ["ui.view.fullScreen"]
        );
        assert!(shell_commands("commands = [\"view.turn.next\"]").is_err());
        assert!(shell_commands("commands = [\"ui.a\", \"ui.a\"]").is_err());
        assert!(shell_commands("ids = []").is_err());
    }

    #[test]
    fn real_feature_map_parses_with_124_rows() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        let text = fs::read_to_string(root.join(FEATURE_MAP)).unwrap();
        assert_eq!(parse_feature_map(&text).unwrap().len(), 124);
    }

    #[test]
    fn committed_files_are_up_to_date() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("..");
        run(&root, true).unwrap();
    }
}
