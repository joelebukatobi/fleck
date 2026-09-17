//! Bundled Iconoir icons (regular set): stroke 2 for the `+` and the panel
//! icon, stroke 1.5 for the rest. Embedded with
//! `include_bytes!` so nothing is read from disk at runtime; each handle is
//! marked `.symbolic(true)` so libcosmic tints it to the theme's icon colour -
//! see `iced/widget/src/svg.rs` (`symbolic` field, used in `draw`) and
//! `widget/icon/mod.rs` (`Svg::symbolic(self.handle.symbolic)`).
use cosmic::widget;

const PLUS: &[u8] = include_bytes!("../../data/icons/iconoir/plus.svg");
const EDIT_PENCIL: &[u8] = include_bytes!("../../data/icons/iconoir/edit-pencil.svg");
const TRASH: &[u8] = include_bytes!("../../data/icons/iconoir/trash.svg");
const SEARCH: &[u8] = include_bytes!("../../data/icons/iconoir/search.svg");
const PAGE_EDIT: &[u8] = include_bytes!("../../data/icons/iconoir/page-edit.svg");

/// The `+` new-note button in the list window header.
pub fn plus() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(PLUS).symbolic(true)
}

/// The rename button on each card.
pub fn edit_pencil() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(EDIT_PENCIL).symbolic(true)
}

/// The delete button on each card.
pub fn trash() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(TRASH).symbolic(true)
}

/// The search bar's leading icon.
pub fn search() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(SEARCH).symbolic(true)
}

/// The panel applet icon (`fleck --applet`).
pub fn page_edit() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(PAGE_EDIT).symbolic(true)
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
    fn plus_and_panel_icon_use_stroke_two_the_rest_stroke_one_and_a_half() {
        for bytes in [PLUS, PAGE_EDIT] {
            assert_eq!(stroke_width(bytes), "2");
        }
        for bytes in [EDIT_PENCIL, TRASH, SEARCH] {
            assert_eq!(stroke_width(bytes), "1.5");
        }
    }

    #[test]
    fn handles_are_marked_symbolic() {
        for handle in [plus(), edit_pencil(), trash(), search(), page_edit()] {
            assert!(handle.symbolic);
        }
    }
}
