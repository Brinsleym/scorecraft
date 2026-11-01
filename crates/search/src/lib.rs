//! scorecraft-search: matching and ordering scores (spec `docs/spec/library.md`, LIB-5, LIB-9).
//!
//! Layer L2 in the workspace map (`docs/architecture.md` §2–3). Pure: no I/O and no clock.
//! [`fold`] compares text without case or accents; [`Query`] matches every query word against
//! the starts of a score's words and ranks the match; [`sort_key`] orders names the way a reader
//! expects (numbers as numbers, leading articles ignored). The full-text index (LIB-6) comes
//! later.
#![deny(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unimplemented,
    clippy::todo,
    clippy::unreachable
)]

use std::cmp::Ordering;

use unicode_normalization::UnicodeNormalization;
use unicode_normalization::char::is_combining_mark;

/// The crate's package name, used by `cargo xtask layers` smoke tests and diagnostics.
pub const CRATE_NAME: &str = env!("CARGO_PKG_NAME");

/// The longest query, in characters.
pub const MAX_QUERY_CHARS: usize = 256;
/// The most words in a query.
pub const MAX_QUERY_WORDS: usize = 16;
/// The most characters of one field looked at.
const MAX_FIELD_CHARS: usize = 4096;

/// `s` without case or accents: decomposed, combining marks dropped, lower case, and the
/// letters that don't decompose spelt out (`ß` → `ss`, `æ` → `ae`, `ø` → `o`, …).
pub fn fold(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars().take(MAX_FIELD_CHARS).nfkd() {
        if is_combining_mark(c) {
            continue;
        }
        match c {
            'ß' | 'ẞ' => out.push_str("ss"),
            'æ' | 'Æ' => out.push_str("ae"),
            'œ' | 'Œ' => out.push_str("oe"),
            'ø' | 'Ø' => out.push('o'),
            'ł' | 'Ł' => out.push('l'),
            'đ' | 'Đ' | 'ð' | 'Ð' => out.push('d'),
            'þ' | 'Þ' => out.push_str("th"),
            'ı' => out.push('i'),
            _ => out.extend(c.to_lowercase()),
        }
    }
    out
}

/// The folded words of `s`: runs of letters and digits.
pub fn words(s: &str) -> Vec<String> {
    fold(s)
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_owned)
        .collect()
}

/// Which field a word was found in; earlier fields rank higher.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    Title,
    Composer,
    /// Arranger, key, genre, tags, file name, folders.
    Other,
}

impl Field {
    fn weight(self) -> u32 {
        match self {
            Field::Title => 4,
            Field::Composer => 2,
            Field::Other => 1,
        }
    }
}

/// Why a query was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QueryError {
    TooLong { chars: usize },
    TooManyWords { words: usize },
}

impl std::fmt::Display for QueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            QueryError::TooLong { chars } => write!(
                f,
                "the search is {chars} characters long; at most {MAX_QUERY_CHARS}"
            ),
            QueryError::TooManyWords { words } => {
                write!(f, "the search has {words} words; at most {MAX_QUERY_WORDS}")
            }
        }
    }
}

impl std::error::Error for QueryError {}

/// A parsed search.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Query {
    words: Vec<String>,
}

impl Query {
    /// `text` as a query; refused when longer than [`MAX_QUERY_CHARS`] or
    /// [`MAX_QUERY_WORDS`].
    pub fn parse(text: &str) -> Result<Query, QueryError> {
        let chars = text.chars().count();
        if chars > MAX_QUERY_CHARS {
            return Err(QueryError::TooLong { chars });
        }
        let words = words(text);
        if words.len() > MAX_QUERY_WORDS {
            return Err(QueryError::TooManyWords { words: words.len() });
        }
        Ok(Query { words })
    }

    /// Whether the query has no words (it matches everything).
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }

    /// How well `fields` match: `None` when some query word starts no word in them; otherwise
    /// higher is better. Each query word counts its best match: the field's weight, doubled for
    /// a whole word.
    pub fn rank(&self, fields: &[(Field, &str)]) -> Option<u32> {
        if self.words.is_empty() {
            return Some(0);
        }
        let doc: Vec<(Field, Vec<String>)> = fields.iter().map(|(f, s)| (*f, words(s))).collect();
        let mut total = 0;
        for q in &self.words {
            let best = doc
                .iter()
                .flat_map(|(f, ws)| {
                    ws.iter().filter_map(move |w| {
                        if w == q {
                            Some(f.weight() * 2)
                        } else if w.starts_with(q.as_str()) {
                            Some(f.weight())
                        } else {
                            None
                        }
                    })
                })
                .max()?;
            total += best;
        }
        Some(total)
    }
}

/// One run of a sort key: digits compared as numbers, the rest as folded text.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Part {
    Num(String),
    Text(String),
}

impl Ord for Part {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            // Numbers as numbers: fewer digits (no leading zeros) is smaller.
            (Part::Num(a), Part::Num(b)) => a.len().cmp(&b.len()).then_with(|| a.cmp(b)),
            (Part::Num(_), Part::Text(_)) => Ordering::Less,
            (Part::Text(_), Part::Num(_)) => Ordering::Greater,
            (Part::Text(a), Part::Text(b)) => a.cmp(b),
        }
    }
}

impl PartialOrd for Part {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A name's place in a sorted list (LIB-9).
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct SortKey(Vec<Part>);

/// The sort key of `name`: folded, a leading "the", "a" or "an" dropped, runs of digits compared
/// as numbers, spaces and punctuation ignored.
pub fn sort_key(name: &str) -> SortKey {
    let folded = fold(name);
    let mut text = folded.trim_start();
    for article in ["the ", "an ", "a "] {
        if let Some(rest) = text.strip_prefix(article)
            && !rest.trim().is_empty()
        {
            text = rest;
            break;
        }
    }
    let mut parts = Vec::new();
    let mut cur = String::new();
    let mut digits = false;
    let flush = |cur: &mut String, digits: bool, parts: &mut Vec<Part>| {
        if cur.is_empty() {
            return;
        }
        let s = std::mem::take(cur);
        parts.push(if digits {
            let t = s.trim_start_matches('0');
            Part::Num(if t.is_empty() { "0".into() } else { t.into() })
        } else {
            Part::Text(s)
        });
    };
    for c in text.chars() {
        if !c.is_alphanumeric() {
            flush(&mut cur, digits, &mut parts);
            continue;
        }
        let d = c.is_ascii_digit();
        if d != digits {
            flush(&mut cur, digits, &mut parts);
            digits = d;
        }
        cur.push(c);
    }
    flush(&mut cur, digits, &mut parts);
    SortKey(parts)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_name_matches_package() {
        assert_eq!(super::CRATE_NAME, "scorecraft-search");
    }

    #[test]
    fn folding_drops_case_and_accents() {
        assert_eq!(fold("Dvořák"), "dvorak");
        assert_eq!(fold("Fauré ÉTUDE"), "faure etude");
        assert_eq!(fold("Straße"), "strasse");
        assert_eq!(fold("Sjøgren Œuvre"), "sjogren oeuvre");
        assert_eq!(
            words("W. A. Mozart — Sonata in A (K. 331)"),
            ["w", "a", "mozart", "sonata", "in", "a", "k", "331"]
        );
    }

    fn sonata() -> Vec<(Field, &'static str)> {
        vec![
            (Field::Title, "Sonata in A"),
            (Field::Composer, "W. A. Mozart"),
            (Field::Other, "sonata.pdf"),
        ]
    }

    #[test]
    fn libs_1_every_word_must_start_a_word() {
        let q = |s: &str| Query::parse(s).unwrap();
        assert!(q("moz son").rank(&sonata()).is_some());
        assert!(q("MOZART").rank(&sonata()).is_some());
        assert!(q("nata").rank(&sonata()).is_none(), "word starts only");
        assert!(q("mozart xyz").rank(&sonata()).is_none());
        assert_eq!(q("").rank(&sonata()), Some(0));
        let etude = [(Field::Title, "Étude"), (Field::Composer, "Chopin")];
        assert!(q("etude").rank(&etude).is_some());
    }

    #[test]
    fn libs_2_titles_rank_above_composers_and_whole_words_above_starts() {
        let q = Query::parse("rondo").unwrap();
        let title = q.rank(&[(Field::Title, "Rondo"), (Field::Composer, "Mozart")]);
        let composer = q.rank(&[(Field::Title, "Air"), (Field::Composer, "Rondo")]);
        let start = q.rank(&[(Field::Title, "Rondoletto")]);
        assert!(
            title > composer && title > start,
            "{title:?} {composer:?} {start:?}"
        );
    }

    #[test]
    fn libs_3_long_queries_are_refused() {
        assert!(matches!(
            Query::parse(&"a".repeat(300)),
            Err(QueryError::TooLong { chars: 300 })
        ));
        let many = vec!["w"; 17].join(" ");
        assert!(matches!(
            Query::parse(&many),
            Err(QueryError::TooManyWords { words: 17 })
        ));
        assert!(Query::parse(&vec!["w"; 16].join(" ")).is_ok());
    }

    #[test]
    fn libs_4_names_sort_naturally() {
        let mut names = vec!["The Swan", "Etude 10", "apple", "Etude 2", "Étude 1", "A"];
        names.sort_by_key(|n| sort_key(n));
        assert_eq!(
            names,
            ["A", "apple", "Étude 1", "Etude 2", "Etude 10", "The Swan"]
        );
        assert_eq!(sort_key("Op. 007"), sort_key("op 7"));
        assert!(sort_key("10") > sort_key("9"));
        // A huge number never overflows.
        let _ = sort_key(&"9".repeat(500));
    }
}
