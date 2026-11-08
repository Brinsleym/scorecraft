//! Synthetic PDFs written from ISO 32000-1 (§7.5 file structure, §7.7 document structure), with
//! no third-party code: valid scores of any page count, size and rotation, each page drawn as
//! staves with its page number, plus hostile files for never-crash tests.
//!
//! Output is deterministic: the same builder always produces the same bytes.

use std::fmt::Write as _;

/// US Letter in PDF points.
pub const LETTER: (f64, f64) = (612.0, 792.0);
/// A4 in PDF points.
pub const A4: (f64, f64) = (595.276, 841.89);

/// One page of a synthetic score.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PageSpec {
    /// MediaBox width in points.
    pub width: f64,
    /// MediaBox height in points.
    pub height: f64,
    /// `/Rotate`, a multiple of 90 (written as given, so tests can also write bad values).
    pub rotate: i32,
    /// Five-line staves drawn top to bottom.
    pub staves: u32,
    /// The page is a scan: one grey-scale image filling the page (white, with a black band
    /// across its second quarter from the top), drawn under the staves.
    pub scan: bool,
}

impl Default for PageSpec {
    fn default() -> Self {
        PageSpec {
            width: LETTER.0,
            height: LETTER.1,
            rotate: 0,
            staves: 10,
            scan: false,
        }
    }
}

impl PageSpec {
    pub fn a4() -> Self {
        PageSpec {
            width: A4.0,
            height: A4.1,
            ..PageSpec::default()
        }
    }
    pub fn landscape(self) -> Self {
        PageSpec {
            width: self.height,
            height: self.width,
            ..self
        }
    }
    pub fn rotated(self, rotate: i32) -> Self {
        PageSpec { rotate, ..self }
    }
    pub fn with_staves(self, staves: u32) -> Self {
        PageSpec { staves, ..self }
    }
    /// A scanned page: an image of the page and nothing else drawn (no staves, no number).
    pub fn scanned(self) -> Self {
        PageSpec {
            scan: true,
            staves: 0,
            ..self
        }
    }
}

/// Builds a valid single-revision PDF 1.7 file.
#[derive(Debug, Clone, Default)]
pub struct PdfBuilder {
    pages: Vec<PageSpec>,
    title: Option<String>,
    composer: Option<String>,
}

impl PdfBuilder {
    pub fn new() -> Self {
        PdfBuilder::default()
    }

    /// Adds one page.
    pub fn page(mut self, spec: PageSpec) -> Self {
        self.pages.push(spec);
        self
    }

    /// Adds `n` identical pages.
    pub fn pages(mut self, n: usize, spec: PageSpec) -> Self {
        self.pages.extend(std::iter::repeat_n(spec, n));
        self
    }

    /// Sets the document information `/Title`.
    pub fn title(mut self, title: &str) -> Self {
        self.title = Some(title.to_owned());
        self
    }

    /// Sets the document information `/Author` (where score PDFs usually keep the composer).
    pub fn composer(mut self, composer: &str) -> Self {
        self.composer = Some(composer.to_owned());
        self
    }

    /// Writes the file. A builder with no pages writes a valid document with an empty page tree.
    pub fn build(&self) -> Vec<u8> {
        // Object numbers: 1 catalog, 2 page tree, 3 font, 4 info, then a (page, content) pair
        // per page, then the scans' image.
        let mut objects: Vec<Vec<u8>> = Vec::new();
        let page_obj = |i: usize| 5 + 2 * i;
        let kids: Vec<String> = (0..self.pages.len())
            .map(|i| format!("{} 0 R", page_obj(i)))
            .collect();
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        objects.push(
            format!(
                "<< /Type /Pages /Kids [{}] /Count {} >>",
                kids.join(" "),
                self.pages.len()
            )
            .into_bytes(),
        );
        objects.push(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec());
        let mut info = String::from("<< /Producer (ScoreCraft testkit)");
        if let Some(t) = &self.title {
            let _ = write!(info, " /Title {}", pdf_string(t));
        }
        if let Some(c) = &self.composer {
            let _ = write!(info, " /Author {}", pdf_string(c));
        }
        info.push_str(" >>");
        objects.push(info.into_bytes());
        let image_obj = page_obj(self.pages.len());
        for (i, spec) in self.pages.iter().enumerate() {
            let content = page_content(spec, i + 1);
            let image = if spec.scan {
                format!(" /XObject << /Scan {image_obj} 0 R >>")
            } else {
                String::new()
            };
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Rotate {} \
                     /Resources << /Font << /F1 3 0 R >>{image} >> /Contents {} 0 R >>",
                    num(spec.width),
                    num(spec.height),
                    spec.rotate,
                    page_obj(i) + 1
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            stream.extend_from_slice(content.as_bytes());
            stream.extend_from_slice(b"\nendstream");
            objects.push(stream);
        }
        if self.pages.iter().any(|p| p.scan) {
            // 4 × 4 DeviceGray (ISO 32000-1 §8.9.5): white, the second row black.
            let data = "FFFFFFFF 00000000 FFFFFFFF FFFFFFFF>";
            objects.push(
                format!(
                    "<< /Type /XObject /Subtype /Image /Width 4 /Height 4 /ColorSpace /DeviceGray \
                     /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length {} >>\nstream\n{data}\nendstream",
                    data.len()
                )
                .into_bytes(),
            );
        }
        assemble(&objects, Some(4))
    }
}

/// Lays out objects 1..=n, then a classic xref table and trailer (ISO 32000-1 §7.5.4–7.5.5).
fn assemble(objects: &[Vec<u8>], info: Option<usize>) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::with_capacity(objects.len());
    for (i, body) in objects.iter().enumerate() {
        offsets.push(out.len());
        out.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }
    let xref_at = out.len();
    let mut xref = format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1);
    for off in offsets {
        let _ = writeln!(xref, "{off:010} 00000 n ");
    }
    let info = info.map(|n| format!(" /Info {n} 0 R")).unwrap_or_default();
    let _ = write!(
        xref,
        "trailer\n<< /Size {} /Root 1 0 R{info} >>\nstartxref\n{xref_at}\n%%EOF\n",
        objects.len() + 1
    );
    out.extend_from_slice(xref.as_bytes());
    out
}

/// Staves of five lines each, spread down the page, and the page number at the foot.
fn page_content(spec: &PageSpec, number: usize) -> String {
    let margin = spec.width * 0.08;
    let top = spec.height * 0.92;
    let usable = spec.height * 0.82;
    let mut s = String::from("0 G 0.6 w\n");
    if spec.scan {
        let _ = writeln!(
            s,
            "q {} 0 0 {} 0 0 cm /Scan Do Q",
            num(spec.width),
            num(spec.height)
        );
        return s;
    }
    if spec.staves > 0 && spec.width.is_finite() && spec.height.is_finite() {
        let pitch = usable / f64::from(spec.staves);
        let gap = (pitch / 10.0).max(0.5);
        for staff in 0..spec.staves {
            let y0 = top - pitch * f64::from(staff);
            for line in 0..5 {
                let y = y0 - gap * f64::from(line);
                let _ = writeln!(
                    s,
                    "{} {} m {} {} l S",
                    num(margin),
                    num(y),
                    num(spec.width - margin),
                    num(y)
                );
            }
        }
    }
    let _ = write!(
        s,
        "BT /F1 10 Tf {} {} Td ({number}) Tj ET",
        num(spec.width / 2.0),
        num(spec.height * 0.04)
    );
    s
}

/// A PDF literal string with `\`, `(` and `)` escaped; non-ASCII as octal UTF-8 bytes behind a BOM.
fn pdf_string(s: &str) -> String {
    let ascii = s.is_ascii();
    let mut out = String::from("(");
    if !ascii {
        out.push_str("\\376\\377");
        for unit in s.encode_utf16() {
            let [hi, lo] = unit.to_be_bytes();
            let _ = write!(out, "\\{hi:03o}\\{lo:03o}");
        }
    } else {
        for c in s.chars() {
            match c {
                '\\' | '(' | ')' => {
                    out.push('\\');
                    out.push(c);
                }
                c if c.is_ascii_control() => {
                    let _ = write!(out, "\\{:03o}", c as u32);
                }
                c => out.push(c),
            }
        }
    }
    out.push(')');
    out
}

/// Formats a coordinate with at most 3 decimals and no trailing zeros (PDF reals have no exponent).
fn num(v: f64) -> String {
    if !v.is_finite() {
        return "0".to_owned();
    }
    let s = format!("{v:.3}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

/// Malformed files every PDF entry point must reject or repair without panicking.
pub mod hostile {
    use super::{PageSpec, PdfBuilder, assemble};

    /// Every hostile fixture, named, for loops in never-crash tests.
    pub fn all() -> Vec<(&'static str, Vec<u8>)> {
        vec![
            ("empty", Vec::new()),
            (
                "not_a_pdf",
                b"PK\x03\x04 this is a zip, not a score".to_vec(),
            ),
            ("header_only", b"%PDF-1.7\n".to_vec()),
            ("truncated", truncated()),
            ("no_xref", no_xref()),
            ("wrong_xref_offsets", wrong_xref_offsets()),
            ("cyclic_page_tree", cyclic_page_tree()),
            ("huge_page_count", huge_page_count()),
            ("bad_rotation", bad_rotation()),
            ("degenerate_media_box", degenerate_media_box()),
            ("huge_stream_length", huge_stream_length()),
        ]
    }

    fn sample() -> Vec<u8> {
        PdfBuilder::new().pages(3, PageSpec::default()).build()
    }

    /// The first half of a valid three-page file.
    pub fn truncated() -> Vec<u8> {
        let mut v = sample();
        v.truncate(v.len() / 2);
        v
    }

    /// A valid body with the xref table, trailer and `startxref` cut off (needs xref repair).
    pub fn no_xref() -> Vec<u8> {
        let v = sample();
        let cut = find(&v, b"xref\n").unwrap_or(v.len());
        v.get(..cut).map(<[u8]>::to_vec).unwrap_or_default()
    }

    /// Every xref offset shifted by 7 bytes.
    pub fn wrong_xref_offsets() -> Vec<u8> {
        let v = sample();
        // Only the xref table is rewritten; the binary header comment stays byte-exact.
        let at = find(&v, b"xref\n").unwrap_or(v.len());
        let (body, table) = v.split_at(at);
        let mut out = body.to_vec();
        for line in String::from_utf8_lossy(table).split_inclusive('\n') {
            match line
                .strip_suffix(" 00000 n \n")
                .and_then(|n| n.parse::<u64>().ok())
            {
                Some(off) => {
                    out.extend_from_slice(format!("{:010} 00000 n \n", off + 7).as_bytes())
                }
                None => out.extend_from_slice(line.as_bytes()),
            }
        }
        out
    }

    /// A page tree whose node lists itself as a kid (an infinite walk without a seen-set).
    pub fn cyclic_page_tree() -> Vec<u8> {
        assemble(
            &[
                b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
                b"<< /Type /Pages /Kids [2 0 R 3 0 R] /Count 2 >>".to_vec(),
                b"<< /Type /Pages /Parent 2 0 R /Kids [2 0 R] /Count 1 >>".to_vec(),
            ],
            None,
        )
    }

    /// `/Count` claims four billion pages for a tree with one.
    pub fn huge_page_count() -> Vec<u8> {
        assemble(
            &[
                b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
                b"<< /Type /Pages /Kids [3 0 R] /Count 4294967295 >>".to_vec(),
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>".to_vec(),
            ],
            None,
        )
    }

    /// `/Rotate 45`, which is not a multiple of 90.
    pub fn bad_rotation() -> Vec<u8> {
        PdfBuilder::new()
            .page(PageSpec::default().rotated(45))
            .build()
    }

    /// A zero-width MediaBox (a renderer must not divide by it).
    pub fn degenerate_media_box() -> Vec<u8> {
        PdfBuilder::new()
            .page(PageSpec {
                width: 0.0,
                height: 792.0,
                rotate: 0,
                staves: 3,
                scan: false,
            })
            .build()
    }

    /// A content stream whose `/Length` claims 2^62 bytes (an allocation cap must hold).
    pub fn huge_stream_length() -> Vec<u8> {
        assemble(
            &[
                b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
                b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>".to_vec(),
                b"<< /Length 4611686018427387904 >>\nstream\n0 0 m 1 1 l S\nendstream".to_vec(),
            ],
            None,
        )
    }

    fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
        hay.windows(needle.len()).position(|w| w == needle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bytes as text with each non-ASCII byte (the binary header comment) as `?`, so string
    /// offsets equal file offsets.
    fn text(b: &[u8]) -> String {
        b.iter()
            .map(|&c| if c.is_ascii() { c as char } else { '?' })
            .collect()
    }

    /// Reads `startxref`, the xref table and checks every in-use offset lands on `N 0 obj`.
    fn check_xref(pdf: &[u8]) -> usize {
        let t = text(pdf);
        let tail = t.rsplit("startxref\n").next().unwrap();
        let xref_at: usize = tail.lines().next().unwrap().parse().unwrap();
        assert!(t[xref_at..].starts_with("xref\n"));
        let mut entries = 0;
        for (n, line) in t[xref_at..]
            .lines()
            .skip(3)
            .take_while(|l| l.trim_end().ends_with(" n"))
            .enumerate()
        {
            let off: usize = line[..10].parse().unwrap();
            assert!(
                t[off..].starts_with(&format!("{} 0 obj\n", n + 1)),
                "object {}",
                n + 1
            );
            entries += 1;
        }
        entries
    }

    #[test]
    fn builds_a_valid_three_page_score() {
        let pdf = PdfBuilder::new()
            .title("Suite No. 1")
            .composer("J. S. Bach")
            .pages(3, PageSpec::default())
            .build();
        let t = text(&pdf);
        assert!(t.starts_with("%PDF-1.7\n"));
        assert!(t.ends_with("%%EOF\n"));
        assert!(t.contains("/Count 3"));
        assert!(t.contains("/Title (Suite No. 1)"));
        assert_eq!(check_xref(&pdf), 4 + 2 * 3);
    }

    #[test]
    fn stream_lengths_are_exact() {
        let pdf = PdfBuilder::new()
            .page(PageSpec::a4().with_staves(4))
            .build();
        let t = text(&pdf);
        let at = t.find("/Length ").unwrap() + 8;
        let len: usize = t[at..]
            .split_whitespace()
            .next()
            .unwrap()
            .trim_end_matches(">>")
            .parse()
            .unwrap();
        let start = t.find("stream\n").unwrap() + 7;
        assert_eq!(&t[start + len..start + len + 10], "\nendstream");
    }

    #[test]
    fn sizes_rotation_and_empty_documents() {
        let pdf = PdfBuilder::new()
            .page(PageSpec::a4().landscape().rotated(90))
            .build();
        let t = text(&pdf);
        assert!(t.contains("/MediaBox [0 0 841.89 595.276] /Rotate 90"));
        let empty = PdfBuilder::new().build();
        assert!(text(&empty).contains("/Kids [] /Count 0"));
        assert_eq!(check_xref(&empty), 4);
    }

    #[test]
    fn output_is_deterministic() {
        let b = PdfBuilder::new().pages(5, PageSpec::default());
        assert_eq!(b.build(), b.build());
    }

    #[test]
    fn strings_are_escaped() {
        assert_eq!(pdf_string("a(b)\\c"), "(a\\(b\\)\\\\c)");
        assert_eq!(pdf_string("é"), "(\\376\\377\\000\\351)");
        assert_eq!(pdf_string("\n"), "(\\012)");
    }

    #[test]
    fn numbers_are_pdf_reals() {
        assert_eq!(num(612.0), "612");
        assert_eq!(num(595.2756), "595.276");
        assert_eq!(num(-0.0001), "0");
        assert_eq!(num(f64::NAN), "0");
    }

    #[test]
    fn hostile_fixtures_are_distinct_and_named() {
        let all = hostile::all();
        let mut names: Vec<&str> = all.iter().map(|(n, _)| *n).collect();
        names.dedup();
        assert_eq!(names.len(), all.len());
        assert!(text(&hostile::no_xref()).find("xref").is_none());
        assert!(text(&hostile::cyclic_page_tree()).contains("/Kids [2 0 R]"));
        assert!(
            hostile::truncated().len()
                < PdfBuilder::new()
                    .pages(3, PageSpec::default())
                    .build()
                    .len()
        );
    }
}
