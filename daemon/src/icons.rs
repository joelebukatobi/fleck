//! Bundled Phosphor icons, replacing the named COSMIC symbolic icons Tack
//! used before it had an icon of its own. Embedded with `include_bytes!` so
//! nothing is read from disk at runtime; each handle is marked
//! `.symbolic(true)` so libcosmic tints it to the theme's icon colour the
//! same way it tints `widget::icon::from_name("...-symbolic")` - see
//! `iced/widget/src/svg.rs` (`symbolic` field, used in `draw`) and
//! `widget/icon/mod.rs` (`Svg::symbolic(self.handle.symbolic)`).
use cosmic::widget;

const PLUS: &[u8] = include_bytes!("../../data/icons/phosphor/plus-bold.svg");
const PENCIL_SIMPLE: &[u8] = include_bytes!("../../data/icons/phosphor/pencil-simple-bold.svg");
const TRASH: &[u8] = include_bytes!("../../data/icons/phosphor/trash-bold.svg");
const MAGNIFYING_GLASS: &[u8] = include_bytes!("../../data/icons/phosphor/magnifying-glass-bold.svg");
const X: &[u8] = include_bytes!("../../data/icons/phosphor/x-bold.svg");

/// The `+` new-note button in the list window header. Replaces `list-add-symbolic`.
pub fn plus() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(PLUS).symbolic(true)
}

/// The rename button on each card. Replaces `edit-symbolic`.
pub fn pencil_simple() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(PENCIL_SIMPLE).symbolic(true)
}

/// The delete button on each card. Replaces `user-trash-symbolic`.
pub fn trash() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(TRASH).symbolic(true)
}

/// The search bar's leading icon. Replaces libcosmic's built-in
/// `system-search-symbolic`.
pub fn magnifying_glass() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(MAGNIFYING_GLASS).symbolic(true)
}

/// The search bar's clear button. Replaces libcosmic's `edit-clear-symbolic`.
pub fn x() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(X).symbolic(true)
}

#[cfg(test)]
mod tests {
    #[test]
    fn x_icon_is_an_svg_and_symbolic() {
        assert!(X.starts_with(b"<svg"), "x-bold.svg must be an SVG");
        assert!(x().symbolic, "the clear icon must be symbolic so it takes the theme colour");
    }

    use super::*;

    fn is_valid_svg(bytes: &[u8]) -> bool {
        std::str::from_utf8(bytes).is_ok_and(|s| s.trim_start().starts_with("<svg"))
    }

    #[test]
    fn embedded_svgs_are_non_empty_and_well_formed() {
        for bytes in [PLUS, PENCIL_SIMPLE, TRASH, MAGNIFYING_GLASS] {
            assert!(!bytes.is_empty());
            assert!(is_valid_svg(bytes));
        }
    }

    #[test]
    fn handles_are_marked_symbolic() {
        assert!(plus().symbolic);
        assert!(pencil_simple().symbolic);
        assert!(trash().symbolic);
        assert!(magnifying_glass().symbolic);
    }
}
