//! Bundled Iconoir icons (regular set, stroke width 1). Embedded with
//! `include_bytes!` so nothing is read from disk at runtime; each handle is
//! marked `.symbolic(true)` so libcosmic tints it to the theme's icon colour -
//! see `iced/widget/src/svg.rs` (`symbolic` field, used in `draw`) and
//! `widget/icon/mod.rs` (`Svg::symbolic(self.handle.symbolic)`).
use cosmic::widget;

const PLUS: &[u8] = include_bytes!("../../data/icons/iconoir/plus.svg");
const EDIT_PENCIL: &[u8] = include_bytes!("../../data/icons/iconoir/edit-pencil.svg");
const TRASH: &[u8] = include_bytes!("../../data/icons/iconoir/trash.svg");
const SEARCH: &[u8] = include_bytes!("../../data/icons/iconoir/search.svg");
const XMARK: &[u8] = include_bytes!("../../data/icons/iconoir/xmark.svg");
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

/// The search bar's clear button.
pub fn xmark() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(XMARK).symbolic(true)
}

/// The panel applet icon (`fleck --applet`).
pub fn page_edit() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(PAGE_EDIT).symbolic(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL: [&[u8]; 6] = [PLUS, EDIT_PENCIL, TRASH, SEARCH, XMARK, PAGE_EDIT];

    #[test]
    fn embedded_svgs_are_well_formed_with_a_stroke_of_one() {
        for bytes in ALL {
            let svg = std::str::from_utf8(bytes).expect("SVGs are UTF-8");
            assert!(svg.trim_start().starts_with("<svg"));
            assert!(
                svg.contains(r#"stroke-width="1""#),
                "every icon uses stroke 1"
            );
        }
    }

    #[test]
    fn handles_are_marked_symbolic() {
        for handle in [
            plus(),
            edit_pencil(),
            trash(),
            search(),
            xmark(),
            page_edit(),
        ] {
            assert!(handle.symbolic);
        }
    }
}
