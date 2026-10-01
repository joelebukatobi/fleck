//! Bundled icons: Fleck's own mark for the panel, Iconoir (regular set, stroke
//! 1.5) for everything inside the app. Embedded with
//! `include_bytes!` so nothing is read from disk at runtime; each handle is
//! marked `.symbolic(true)` so libcosmic tints it to the theme's icon colour -
//! see `iced/widget/src/svg.rs` (`symbolic` field, used in `draw`) and
//! `widget/icon/mod.rs` (`Svg::symbolic(self.handle.symbolic)`).
use cosmic::widget;

const TRASH: &[u8] = include_bytes!("../../data/icons/iconoir/trash.svg");
const TRASH_FILLED: &[u8] = include_bytes!("../../data/icons/iconoir/trash-filled.svg");
const SEARCH: &[u8] = include_bytes!("../../data/icons/iconoir/search.svg");
/// Fleck's own icon, in its monochrome form: a note with a fleck on it.
const MARK: &[u8] =
    include_bytes!("../../data/icons/fleck/io.github.joelebukatobi.Fleck-symbolic.svg");
const MICROPHONE: &[u8] = include_bytes!("../../data/icons/iconoir/microphone.svg");

/// The delete button on each card.
pub fn trash() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(TRASH).symbolic(true)
}

/// The delete button on each card while hovered: Iconoir's trash, filled.
pub fn trash_filled() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(TRASH_FILLED).symbolic(true)
}

/// The search bar's leading icon.
pub fn search() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(SEARCH).symbolic(true)
}

/// The dictation button in the corner of a note.
pub fn microphone() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(MICROPHONE).symbolic(true)
}

/// The panel applet icon (`fleck --applet`): Fleck's own mark.
pub fn mark() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(MARK).symbolic(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stroke_width(bytes: &[u8]) -> &str {
        let svg = std::str::from_utf8(bytes).expect("SVGs are UTF-8");
        assert!(svg.trim_start().starts_with("<svg"));
        let start = svg
            .find(r#"stroke-width=""#)
            .expect("icon sets a stroke width")
            + 14;
        &svg[start..start + svg[start..].find('"').unwrap()]
    }

    #[test]
    fn the_iconoir_icons_use_stroke_one_and_a_half() {
        for bytes in [TRASH, TRASH_FILLED, SEARCH, MICROPHONE] {
            assert_eq!(stroke_width(bytes), "1.5");
        }
    }

    /// The panel draws the mark at 16 px and tints it, so it is filled rather
    /// than stroked, and square.
    #[test]
    fn the_mark_is_a_square_filled_icon() {
        let svg = std::str::from_utf8(MARK).expect("SVGs are UTF-8");
        assert!(svg.contains(r#"viewBox="0 0 16 16""#), "{svg}");
        assert!(!svg.contains("stroke-width"), "{svg}");
    }

    #[test]
    fn handles_are_marked_symbolic() {
        for handle in [trash(), trash_filled(), search(), mark(), microphone()] {
            assert!(handle.symbolic);
        }
    }
}
