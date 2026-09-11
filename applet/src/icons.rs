//! The applet's one bundled Phosphor icon, embedded with `include_bytes!` so
//! nothing is read from disk at runtime. Marked `.symbolic(true)` so
//! `Applet::icon_button_from_handle` tints it like the panel's other icons -
//! see `libcosmic`'s `src/applet/mod.rs` (`icon_button_from_handle`, which
//! reads `icon.symbolic` to pick colour and size) and `src/widget/icon/mod.rs`.
use cosmic::widget;

const NOTE_PENCIL: &[u8] = include_bytes!("../../data/icons/phosphor/note-pencil-bold.svg");

/// The panel applet icon. Replaces `accessories-text-editor-symbolic`.
pub fn note_pencil() -> widget::icon::Handle {
    widget::icon::from_svg_bytes(NOTE_PENCIL).symbolic(true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_svg_is_non_empty_and_well_formed() {
        assert!(!NOTE_PENCIL.is_empty());
        let s = std::str::from_utf8(NOTE_PENCIL).expect("valid utf8");
        assert!(s.trim_start().starts_with("<svg"));
    }

    #[test]
    fn handle_is_marked_symbolic() {
        assert!(note_pencil().symbolic);
    }
}
