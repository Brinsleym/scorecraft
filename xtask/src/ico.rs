//! `cargo xtask ico`: packs the hicolor PNG renders into `assets/app-icon/scorecraft.ico`.
//!
//! Every entry is stored as PNG, which Windows Vista and later read at every size. The PNGs come
//! from `assets/app-icon/render.sh`; `apps/scorecraft/build.rs` embeds the result in the `.exe`.

use std::path::Path;

/// The sizes Explorer, the taskbar, Alt+Tab and the title bar ask for.
pub const SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

pub fn run(root: &Path) -> Result<(), String> {
    let dir = root.join("assets/app-icon");
    let mut images = Vec::with_capacity(SIZES.len());
    for s in SIZES {
        let path = dir.join(format!("hicolor/{s}x{s}/apps/app.scorecraft.png"));
        let png = std::fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        images.push((s, png));
    }
    let ico = pack(&images)?;
    let out = dir.join("scorecraft.ico");
    std::fs::write(&out, ico).map_err(|e| format!("{}: {e}", out.display()))?;
    println!("wrote {} ({} sizes)", out.display(), SIZES.len());
    Ok(())
}

/// An ICO file holding each `(size, png)` as a PNG entry.
pub fn pack(images: &[(u32, Vec<u8>)]) -> Result<Vec<u8>, String> {
    let count = u16::try_from(images.len()).map_err(|_| "too many images".to_owned())?;
    let mut out = Vec::new();
    out.extend_from_slice(&[0, 0, 1, 0]); // reserved, type 1 (icon)
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * images.len();
    for (size, png) in images {
        if !(1..=256).contains(size) || !png.starts_with(b"\x89PNG\r\n\x1a\n") {
            return Err(format!("{size} px entry: not a PNG of 1 to 256 px"));
        }
        // 256 is written as 0.
        let side = u8::try_from(*size % 256).map_err(|_| "size".to_owned())?;
        let len = u32::try_from(png.len()).map_err(|_| "PNG too large".to_owned())?;
        let at = u32::try_from(offset).map_err(|_| "ICO too large".to_owned())?;
        out.extend_from_slice(&[side, side, 0, 0]); // width, height, no palette, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&at.to_le_bytes());
        offset += png.len();
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PNG: &[u8] = b"\x89PNG\r\n\x1a\nrest";

    #[test]
    fn packs_a_directory_then_the_images() {
        let ico = pack(&[(16, PNG.to_vec()), (256, PNG.to_vec())]).unwrap();
        assert_eq!(&ico[..6], &[0, 0, 1, 0, 2, 0]);
        assert_eq!(ico[6], 16);
        assert_eq!(ico[22], 0, "256 px is written as 0");
        assert_eq!(u32::from_le_bytes(ico[18..22].try_into().unwrap()), 38);
        assert_eq!(&ico[38..38 + PNG.len()], PNG);
        assert_eq!(ico.len(), 38 + 2 * PNG.len());
    }

    #[test]
    fn rejects_what_is_not_a_png() {
        assert!(pack(&[(16, b"BM".to_vec())]).is_err());
        assert!(pack(&[(512, PNG.to_vec())]).is_err());
    }

    /// The committed icon holds every size, and each entry is the matching hicolor render.
    #[test]
    fn committed_ico_is_current() {
        let root = crate::workspace_root();
        let ico = std::fs::read(root.join("assets/app-icon/scorecraft.ico")).unwrap();
        let mut images = Vec::new();
        for s in SIZES {
            let p = root.join(format!(
                "assets/app-icon/hicolor/{s}x{s}/apps/app.scorecraft.png"
            ));
            images.push((s, std::fs::read(p).unwrap()));
        }
        assert!(ico == pack(&images).unwrap(), "run `cargo xtask ico`");
    }
}
