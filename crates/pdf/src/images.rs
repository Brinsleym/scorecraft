//! Photos and scans as pages (feature-map IMP-3): each image one page of a new PDF, on
//! PDFCraft's object layer. JPEGs are embedded as they are (`DCTDecode`); PNGs are decoded and
//! stored as 8-bit grey or RGB (transparency over white paper).

use pdfcraft_cos::{Dict, Document as Cos, Object, PdfString, SaveOptions, Stream};

use crate::PdfError;

/// The most images one import turns into pages.
pub const MAX_IMAGES: usize = 500;
/// The most pixels one image may have (decoded).
const MAX_PIXELS: u64 = 80_000_000;

/// A decoded or passed-through image, ready to be an image XObject.
struct Image {
    width: u32,
    height: u32,
    color: &'static str,
    /// `Some("DCTDecode")` for JPEG bytes as they are; `None` for raw samples to compress.
    filter: Option<&'static str>,
    data: Vec<u8>,
}

/// Width, height and components of a baseline or progressive JPEG, from its frame header.
fn jpeg_size(b: &[u8]) -> Option<(u32, u32, u8)> {
    let mut i = 2;
    while i + 9 < b.len() {
        if *b.get(i)? != 0xFF {
            return None;
        }
        let marker = *b.get(i + 1)?;
        let len = usize::from(u16::from_be_bytes([*b.get(i + 2)?, *b.get(i + 3)?]));
        if matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF) {
            let h = u16::from_be_bytes([*b.get(i + 5)?, *b.get(i + 6)?]);
            let w = u16::from_be_bytes([*b.get(i + 7)?, *b.get(i + 8)?]);
            return Some((u32::from(w), u32::from(h), *b.get(i + 9)?));
        }
        i += 2 + len;
    }
    None
}

fn decode(name: &str, bytes: &[u8]) -> Result<Image, PdfError> {
    let bad = |why: &str| PdfError::Invalid(format!("{name}: {why}"));
    if bytes.starts_with(&[0xFF, 0xD8]) {
        let (width, height, comps) =
            jpeg_size(bytes).ok_or_else(|| bad("not a JPEG we can read"))?;
        let color = match comps {
            1 => "DeviceGray",
            3 => "DeviceRGB",
            4 => "DeviceCMYK",
            _ => return Err(bad("an unusual JPEG")),
        };
        return Ok(Image {
            width,
            height,
            color,
            filter: Some("DCTDecode"),
            data: bytes.to_vec(),
        });
    }
    if bytes.starts_with(b"\x89PNG") {
        let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
        dec.set_transformations(png::Transformations::EXPAND | png::Transformations::STRIP_16);
        let mut reader = dec.read_info().map_err(|e| bad(&e.to_string()))?;
        let (w, h) = (reader.info().width, reader.info().height);
        if u64::from(w) * u64::from(h) > MAX_PIXELS {
            return Err(bad("the image is too large"));
        }
        let mut buf = vec![
            0;
            reader
                .output_buffer_size()
                .ok_or_else(|| bad("the image is too large"))?
        ];
        let frame = reader
            .next_frame(&mut buf)
            .map_err(|e| bad(&e.to_string()))?;
        let px = buf.get(..frame.buffer_size()).unwrap_or(&[]);
        // Transparency goes over white paper.
        let over =
            |c: u8, a: u8| ((u16::from(c) * u16::from(a) + 255 * (255 - u16::from(a))) / 255) as u8;
        let (color, data): (&'static str, Vec<u8>) = match frame.color_type {
            png::ColorType::Grayscale => ("DeviceGray", px.to_vec()),
            png::ColorType::GrayscaleAlpha => (
                "DeviceGray",
                px.as_chunks::<2>()
                    .0
                    .iter()
                    .map(|[c, a]| over(*c, *a))
                    .collect(),
            ),
            png::ColorType::Rgb => ("DeviceRGB", px.to_vec()),
            png::ColorType::Rgba => (
                "DeviceRGB",
                px.as_chunks::<4>()
                    .0
                    .iter()
                    .flat_map(|[r, g, b, a]| [over(*r, *a), over(*g, *a), over(*b, *a)])
                    .collect(),
            ),
            png::ColorType::Indexed => return Err(bad("an unusual PNG")),
        };
        return Ok(Image {
            width: w,
            height: h,
            color,
            filter: None,
            data,
        });
    }
    Err(bad("use PNG or JPEG images"))
}

/// A new PDF titled `title`, with one page per image (PNG or JPEG), in order. Each page is the
/// image's shape, 612 points wide (792 for landscape images).
pub fn pdf_from_images(images: &[(String, Vec<u8>)], title: &str) -> Result<Vec<u8>, PdfError> {
    if images.is_empty() || images.len() > MAX_IMAGES {
        return Err(PdfError::Invalid(format!("give 1 to {MAX_IMAGES} images")));
    }
    let fail = |e: &dyn std::fmt::Display| PdfError::Render(format!("making the PDF: {e}"));
    let mut doc = Cos::new_empty();
    for (i, (name, bytes)) in images.iter().enumerate() {
        let img = decode(name, bytes)?;
        if img.width == 0 || img.height == 0 {
            return Err(PdfError::Invalid(format!("{name}: the image is empty")));
        }
        let w = if img.width >= img.height {
            792.0
        } else {
            612.0
        };
        let h = (w * f64::from(img.height) / f64::from(img.width)).clamp(72.0, 14_000.0);
        let page = pdfcraft_organize::insert_blank_page(&mut doc, i, w, h).map_err(|e| fail(&e))?;
        let mut d = Dict::new();
        d.set(b"Type".to_vec(), Object::name("XObject"));
        d.set(b"Subtype".to_vec(), Object::name("Image"));
        d.set(b"Width".to_vec(), Object::Int(i64::from(img.width)));
        d.set(b"Height".to_vec(), Object::Int(i64::from(img.height)));
        d.set(b"ColorSpace".to_vec(), Object::name(img.color));
        d.set(b"BitsPerComponent".to_vec(), Object::Int(8));
        let stream = match img.filter {
            Some(f) => {
                d.set(b"Filter".to_vec(), Object::name(f));
                Stream::from_raw(d, img.data)
            }
            None => Stream::flate(d, &img.data),
        };
        let xobject = doc.add(Object::Stream(stream));
        let content = format!("q {w:.2} 0 0 {h:.2} 0 0 cm /Im0 Do Q\n");
        let contents = doc.add(Object::Stream(Stream::flate(
            Dict::new(),
            content.as_bytes(),
        )));
        doc.update_dict(page, |p| {
            let mut xo = Dict::new();
            xo.set(b"Im0".to_vec(), Object::Ref(xobject));
            let mut res = Dict::new();
            res.set(b"XObject".to_vec(), Object::Dict(xo));
            p.set(b"Resources".to_vec(), Object::Dict(res));
            p.set(b"Contents".to_vec(), Object::Ref(contents));
        })
        .map_err(|e| fail(&e))?;
    }
    let mut info = Dict::new();
    info.set(b"Title".to_vec(), PdfString::text(title));
    info.set(b"Producer".to_vec(), PdfString::text("ScoreCraft"));
    let info = doc.add(Object::Dict(info));
    doc.trailer_mut().set(b"Info".to_vec(), Object::Ref(info));
    pdfcraft_cos::write_full(&doc, &SaveOptions::default()).map_err(|e| fail(&e))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use std::sync::Arc;

    use super::*;
    use crate::Document;

    #[test]
    fn imp_3_images_become_pages() {
        let png = |w: u32, h: u32| {
            let rgba = vec![200u8; (w * h * 4) as usize];
            tiny_png(w, h, &rgba)
        };
        let pdf = pdf_from_images(
            &[("a.png".into(), png(40, 60)), ("b.png".into(), png(80, 30))],
            "Scans",
        )
        .unwrap();
        let doc = Document::open(Arc::new(pdf)).unwrap();
        assert_eq!(doc.page_count(), 2);
        let (a, b) = (doc.page(0).unwrap(), doc.page(1).unwrap());
        assert!(
            a.crop[3] - a.crop[1] > a.crop[2] - a.crop[0],
            "portrait image, portrait page"
        );
        assert!(b.crop[2] - b.crop[0] > b.crop[3] - b.crop[1]);
        assert!(pdf_from_images(&[], "x").is_err());
        // A JPEG's size comes from its frame header.
        let jpeg = [
            0xFF, 0xD8, 0xFF, 0xC0, 0x00, 0x11, 0x08, 0x00, 0x20, 0x00, 0x40, 0x03, 0, 0, 0,
        ];
        assert_eq!(jpeg_size(&jpeg), Some((64, 32, 3)));
        assert_eq!(jpeg_size(&[0xFF, 0xD8, 0x00]), None);
        assert!(pdf_from_images(&[("x.png".into(), b"not an image".to_vec())], "x").is_err());
    }

    /// A tiny PNG encoder for the test (stored, unfiltered).
    fn tiny_png(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
        let mut data = Vec::new();
        for row in rgba.chunks((w * 4) as usize) {
            data.push(0u8);
            data.extend_from_slice(row);
        }
        let mut out = Vec::new();
        let mut enc = png_encoder(&mut out, w, h);
        enc.write_all(&data);
        out
    }

    fn png_encoder(out: &mut Vec<u8>, w: u32, h: u32) -> PngWriter<'_> {
        PngWriter { out, w, h }
    }

    struct PngWriter<'a> {
        out: &'a mut Vec<u8>,
        w: u32,
        h: u32,
    }

    impl PngWriter<'_> {
        fn chunk(&mut self, kind: &[u8], data: &[u8]) {
            self.out
                .extend_from_slice(&(data.len() as u32).to_be_bytes());
            let mut c = kind.to_vec();
            c.extend_from_slice(data);
            self.out.extend_from_slice(&c);
            self.out.extend_from_slice(&crc(&c).to_be_bytes());
        }

        fn write_all(&mut self, raw: &[u8]) {
            self.out.extend_from_slice(b"\x89PNG\r\n\x1a\n");
            let mut ihdr = Vec::new();
            ihdr.extend_from_slice(&self.w.to_be_bytes());
            ihdr.extend_from_slice(&self.h.to_be_bytes());
            ihdr.extend_from_slice(&[8, 6, 0, 0, 0]);
            self.chunk(b"IHDR", &ihdr);
            // zlib with stored blocks.
            let mut z = vec![0x78, 0x01];
            for (i, block) in raw.chunks(65_535).enumerate() {
                let last = (i + 1) * 65_535 >= raw.len();
                z.push(u8::from(last));
                let len = block.len() as u16;
                z.extend_from_slice(&len.to_le_bytes());
                z.extend_from_slice(&(!len).to_le_bytes());
                z.extend_from_slice(block);
            }
            z.extend_from_slice(&adler(raw).to_be_bytes());
            self.chunk(b"IDAT", &z);
            self.chunk(b"IEND", &[]);
        }
    }

    fn crc(data: &[u8]) -> u32 {
        let mut c = 0xffff_ffffu32;
        for b in data {
            c ^= u32::from(*b);
            for _ in 0..8 {
                c = if c & 1 == 1 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                };
            }
        }
        !c
    }

    fn adler(data: &[u8]) -> u32 {
        let (mut a, mut b) = (1u32, 0u32);
        for x in data {
            a = (a + u32::from(*x)) % 65_521;
            b = (b + a) % 65_521;
        }
        (b << 16) | a
    }
}
