//! The app icon, everywhere Windows, macOS and Linux show one (`assets/app-icon/README.md`).
//!
//! - **Executable:** `build.rs` embeds `assets/app-icon/scorecraft.ico` (16–256 px) as icon
//!   resource 1, which Explorer, the Start menu, Alt+Tab and an unpinned taskbar button use.
//! - **Window:** [`window_icon`] sets the title-bar and taskbar icon at runtime (`WM_SETICON` on
//!   Windows, `_NET_WM_ICON` on X11, the Dock when running unbundled on macOS).

/// Window, taskbar and (unbundled) Dock icon: macOS gets the 1024 px render on Apple's icon grid,
/// everything else the 256 px render of the full tile. A decode failure leaves eframe's default.
pub fn window_icon() -> eframe::egui::IconData {
    match eframe::icon_data::from_png_bytes(PNG) {
        Ok(icon) => icon,
        Err(e) => {
            eprintln!("scorecraft: app icon: {e}");
            eframe::egui::IconData::default()
        }
    }
}

#[cfg(target_os = "macos")]
const PNG: &[u8] = include_bytes!("../../../assets/app-icon/scorecraft-1024.png");
#[cfg(not(target_os = "macos"))]
const PNG: &[u8] =
    include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/app.scorecraft.png");

#[cfg(test)]
mod tests {
    use super::*;

    const ICO: &[u8] = include_bytes!("../../../assets/app-icon/scorecraft.ico");

    fn u16_at(b: &[u8], i: usize) -> usize {
        usize::from(u16::from_le_bytes([b[i], b[i + 1]]))
    }

    #[test]
    fn window_icon_decodes() {
        let icon = window_icon();
        assert!(
            icon.width >= 256 && icon.width == icon.height,
            "{}x{}",
            icon.width,
            icon.height
        );
        assert_eq!(
            icon.rgba.len(),
            icon.width as usize * icon.height as usize * 4
        );
        assert!(
            icon.rgba.chunks(4).any(|p| p[3] > 0),
            "the icon isn't blank"
        );
    }

    #[test]
    fn the_tile_keeps_its_round_corners() {
        // rx=112 of 512 leaves (10, 10) of a 256 px render transparent.
        let png =
            include_bytes!("../../../assets/app-icon/hicolor/256x256/apps/app.scorecraft.png");
        let icon = eframe::icon_data::from_png_bytes(png).unwrap();
        assert_eq!(icon.width, 256);
        assert_eq!(
            icon.rgba[(10 * 256 + 10) * 4 + 3],
            0,
            "corner is transparent"
        );
        let centre_left = (128 * 256 + 2) * 4;
        assert_eq!(
            &icon.rgba[centre_left..centre_left + 4],
            &[0xc4, 0x58, 0x21, 255],
            "brand tile"
        );
    }

    /// Windows asks for these sizes; a missing one is scaled from another and blurs.
    #[test]
    fn exe_icon_has_every_windows_size() {
        assert_eq!(&ICO[..4], &[0, 0, 1, 0]);
        let count = u16_at(ICO, 4);
        let sizes: Vec<usize> = (0..count)
            .map(|i| match ICO[6 + 16 * i] {
                0 => 256,
                s => usize::from(s),
            })
            .collect();
        assert_eq!(sizes, [16, 24, 32, 48, 64, 128, 256]);
    }
}
