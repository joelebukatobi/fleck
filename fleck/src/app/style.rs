//! Theme-derived styles for the notes list: search bar, cards, and the
//! header and card icon buttons.

use cosmic::iced::{Border, Color};

pub(super) const SEARCH_RADIUS: f32 = 4.0;
pub(super) const CARD_RADIUS: f32 = 4.0;
/// How much darker the card heading strip is than the card itself: each
/// channel is scaled down by this factor, so `1.0` would be no change and
/// `0.0` would be black.
pub(super) const HEADING_DARKEN_FACTOR: f32 = 0.70;
/// How much lighter a hovered card gets: each channel is mixed towards
/// white by this fraction, so `0.0` would be no change and `1.0` would be
/// white.
pub(super) const CARD_HOVER_LIGHTEN_FACTOR: f32 = 0.08;
/// How much darker a pressed card gets - a subtler scale-down than
/// `HEADING_DARKEN_FACTOR`, which exists to contrast a whole strip rather
/// than to read as a light "pressed" tap.
pub(super) const CARD_PRESS_DARKEN_FACTOR: f32 = 0.90;

/// Builds one state closure of the search bar's style: the theme's `Search`
/// appearance for `state` (active/hovered/focused/error/disabled), with the
/// radius replaced by `SEARCH_RADIUS`. Every other field - background,
/// border colour, text colours - stays exactly what the theme gives it.
pub(super) fn search_input_style(
    state: fn(&cosmic::Theme, &cosmic::theme::TextInput) -> cosmic::widget::text_input::Appearance,
) -> Box<dyn Fn(&cosmic::Theme) -> cosmic::widget::text_input::Appearance> {
    Box::new(move |theme| {
        let mut appearance = state(theme, &cosmic::theme::TextInput::Search);
        appearance.border_radius = SEARCH_RADIUS.into();
        appearance
    })
}

/// A card's style: the theme's `Card` appearance with the radius replaced
/// by `CARD_RADIUS` instead of the theme's `radius_s`. Every other field -
/// background, text/icon colours - stays exactly what the theme gives it.
pub(super) fn card_container_style(
    theme: &cosmic::Theme,
) -> cosmic::iced::widget::container::Style {
    let mut style = <cosmic::Theme as cosmic::iced::widget::container::Catalog>::style(
        theme,
        &cosmic::theme::Container::Card,
    );
    style.border.radius = CARD_RADIUS.into();
    style
}

/// Scales a colour's channels down by `factor`, leaving alpha untouched. A
/// pure scale-down of already non-negative channels can never produce a
/// negative channel, so black stays black instead of clipping.
pub(super) fn darken(color: Color, factor: f32) -> Color {
    Color {
        r: color.r * factor,
        g: color.g * factor,
        b: color.b * factor,
        a: color.a,
    }
}

/// Mixes a colour's channels towards white by `factor`, leaving alpha
/// untouched. A pure lerp towards 1.0 of already at-most-1.0 channels can
/// never push a channel past 1.0, so white stays white instead of clipping.
pub(super) fn lighten(color: Color, factor: f32) -> Color {
    Color {
        r: color.r + (1.0 - color.r) * factor,
        g: color.g + (1.0 - color.g) * factor,
        b: color.b + (1.0 - color.b) * factor,
        a: color.a,
    }
}

/// The card heading strip's style: the card's own background (see
/// `card_container_style`) darkened by `darken`, rounded only at the top
/// (`CARD_RADIUS`) so it follows the card's rounded top edge while its
/// bottom edge - where it meets the content section - stays square.
///
/// Stays the same regardless of the card button's hover/press state: a
/// container's style function only ever receives the theme, not its
/// parent button's interaction state, so there is no state to shift it
/// with even if the design wanted that - see `view_card`.
pub(super) fn card_heading_style(theme: &cosmic::Theme) -> cosmic::iced::widget::container::Style {
    let card = card_container_style(theme);
    cosmic::iced::widget::container::Style {
        background: card.background.map(|background| match background {
            cosmic::iced::Background::Color(c) => {
                cosmic::iced::Background::Color(darken(c, HEADING_DARKEN_FACTOR))
            }
            gradient @ cosmic::iced::Background::Gradient(_) => gradient,
        }),
        border: Border {
            radius: cosmic::iced::border::top(CARD_RADIUS),
            ..Border::default()
        },
        ..card
    }
}

/// The card's theme background colour alone (see `card_container_style`),
/// for building the card button's hover/press variants. Falls back to
/// transparent for the (never-hit-in-practice) case that the theme's Card
/// container has no solid colour background.
pub(super) fn card_color(theme: &cosmic::Theme) -> Color {
    match card_container_style(theme).background {
        Some(cosmic::iced::Background::Color(c)) => c,
        _ => Color::TRANSPARENT,
    }
}

/// One state's appearance for the card button (see `view_card`): `card_container_style`'s
/// border and `CARD_RADIUS`, with `background` as the fill - `card_color`
/// itself for the resting state, `lighten`/`darken`d for hover/press. `focused`
/// draws the same accent outline `cosmic`'s own button styles draw for a
/// Tab-focused button, so keyboard focus stays visible.
pub(super) fn card_button_style(
    theme: &cosmic::Theme,
    focused: bool,
    background: Color,
) -> cosmic::widget::button::Style {
    let card = card_container_style(theme);
    let mut style = cosmic::widget::button::Style {
        background: Some(cosmic::iced::Background::Color(background)),
        border_radius: card.border.radius,
        border_width: card.border.width,
        border_color: card.border.color,
        ..cosmic::widget::button::Style::new()
    };
    if focused {
        let cosmic = theme.cosmic();
        style.outline_width = 1.0;
        style.outline_color = cosmic.accent.base.into();
        style.border_width = 2.0;
        style.border_color = Color::TRANSPARENT;
    }
    style
}

/// The card button's full `Button::Custom` style class: active/hovered/
/// pressed each derive their background from `card_color`, lightened on
/// hover and darkened on press (see `lighten`/`darken`) - never a hardcoded
/// colour. `disabled` is never actually reached (a card button is never
/// disabled) but is required to build the class.
pub(super) fn card_button_class() -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(|focused, theme| card_button_style(theme, focused, card_color(theme))),
        disabled: Box::new(|theme| card_button_style(theme, false, card_color(theme))),
        hovered: Box::new(|focused, theme| {
            card_button_style(
                theme,
                focused,
                lighten(card_color(theme), CARD_HOVER_LIGHTEN_FACTOR),
            )
        }),
        pressed: Box::new(|focused, theme| {
            card_button_style(
                theme,
                focused,
                darken(card_color(theme), CARD_PRESS_DARKEN_FACTOR),
            )
        }),
    }
}

/// Which theme colour a header icon button's (`+`, pencil, trash) hovered/
/// pressed icon takes - see `icon_button_class`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum IconHoverRole {
    Accent,
    Destructive,
}

/// The hovered/pressed icon colour for a header icon button, given its
/// role - pure and GUI-free so it's directly testable. `cosmic.accent.base`
/// and `cosmic.destructive.base` are exactly the colours `accent_button` and
/// `destructive_button` use for their own backgrounds too (both are built
/// from that same base colour - see `cosmic-theme`'s
/// `Component::colored_button`), so `Destructive` here really is "the same
/// red the delete confirmation dialog's `widget::button::destructive`
/// draws", not a new colour.
pub(super) fn icon_hover_color(role: IconHoverRole, theme: &cosmic::Theme) -> Color {
    let cosmic = theme.cosmic();
    match role {
        IconHoverRole::Accent => cosmic.accent.base.into(),
        IconHoverRole::Destructive => cosmic.destructive.base.into(),
    }
}

/// A header icon button's (the `+` new-note button in `header_start`, and
/// the pencil/trash buttons in `view_card`) full `Button::Custom` style
/// class: no background in any state - removing libcosmic's default icon-
/// button hover/press fill - the theme's normal icon colour at rest and
/// disabled exactly as `widget::button::icon` already draws it, and `role`'s
/// colour (`icon_hover_color`) while hovered or pressed.
///
/// Built from `Catalog::active/hovered/pressed/disabled` for
/// `theme::Button::Icon` (libcosmic's own icon-button appearance) rather
/// than a hand-built `Style`, so focus outline, sizing, and disabled
/// appearance stay byte-for-byte what they are today - the only fields this
/// ever touches are `background` (always cleared) and hover/press
/// `icon_color`.
pub(super) fn icon_button_class(role: IconHoverRole) -> cosmic::theme::Button {
    fn no_background(mut style: cosmic::widget::button::Style) -> cosmic::widget::button::Style {
        style.background = None;
        style
    }
    cosmic::theme::Button::Custom {
        active: Box::new(|focused, theme| {
            no_background(<cosmic::Theme as cosmic::widget::button::Catalog>::active(
                theme,
                focused,
                false,
                &cosmic::theme::Button::Icon,
            ))
        }),
        disabled: Box::new(|theme| {
            no_background(
                <cosmic::Theme as cosmic::widget::button::Catalog>::disabled(
                    theme,
                    &cosmic::theme::Button::Icon,
                ),
            )
        }),
        hovered: Box::new(move |focused, theme| {
            let mut style =
                no_background(<cosmic::Theme as cosmic::widget::button::Catalog>::hovered(
                    theme,
                    focused,
                    false,
                    &cosmic::theme::Button::Icon,
                ));
            style.icon_color = Some(icon_hover_color(role, theme));
            style
        }),
        pressed: Box::new(move |focused, theme| {
            let mut style =
                no_background(<cosmic::Theme as cosmic::widget::button::Catalog>::pressed(
                    theme,
                    focused,
                    false,
                    &cosmic::theme::Button::Icon,
                ));
            style.icon_color = Some(icon_hover_color(role, theme));
            style
        }),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn icon_hover_color_maps_trash_to_destructive() {
        let theme = cosmic::Theme::dark();
        let expected: Color = theme.cosmic().destructive.base.into();
        assert_eq!(
            icon_hover_color(IconHoverRole::Destructive, &theme),
            expected
        );
    }

    #[test]
    fn icon_hover_color_maps_plus_and_pencil_to_accent() {
        let theme = cosmic::Theme::dark();
        let expected: Color = theme.cosmic().accent.base.into();
        assert_eq!(icon_hover_color(IconHoverRole::Accent, &theme), expected);
    }

    #[test]
    fn icon_hover_color_accent_and_destructive_differ() {
        let theme = cosmic::Theme::dark();
        assert_ne!(
            icon_hover_color(IconHoverRole::Accent, &theme),
            icon_hover_color(IconHoverRole::Destructive, &theme),
            "trash must read visibly differently from the + and pencil buttons"
        );
    }

    /// Relative luminance (sRGB weights), used only to compare two colours'
    /// perceived brightness in these tests.
    fn luminance(c: Color) -> f32 {
        0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b
    }

    #[test]
    fn darken_reduces_luminance_for_light_mid_and_dark_colors() {
        for c in [
            Color::from_rgb(0.9, 0.9, 0.9),
            Color::from_rgb(0.5, 0.5, 0.5),
            Color::from_rgb(0.15, 0.15, 0.15),
        ] {
            let d = darken(c, HEADING_DARKEN_FACTOR);
            assert!(
                luminance(d) < luminance(c),
                "{d:?} should be darker than {c:?}"
            );
        }
    }

    #[test]
    fn darken_preserves_alpha() {
        let c = Color::from_rgba(0.5, 0.4, 0.3, 0.42);
        assert_eq!(darken(c, HEADING_DARKEN_FACTOR).a, 0.42);
    }

    #[test]
    fn darken_keeps_pure_black_black_not_negative() {
        let black = Color::from_rgb(0.0, 0.0, 0.0);
        let d = darken(black, HEADING_DARKEN_FACTOR);
        assert_eq!(d, black);
        assert!(d.r >= 0.0 && d.g >= 0.0 && d.b >= 0.0);
    }

    /// `lighten` doesn't exist yet - this locks in what it must do before
    /// it's written: raise luminance for light, mid and dark inputs alike,
    /// preserve alpha, and leave white unchanged (mixing towards white can
    /// never push a channel past 1.0).
    #[test]
    fn lighten_increases_luminance_for_light_mid_and_dark_colors() {
        for c in [
            Color::from_rgb(0.9, 0.9, 0.9),
            Color::from_rgb(0.5, 0.5, 0.5),
            Color::from_rgb(0.15, 0.15, 0.15),
        ] {
            let l = lighten(c, CARD_HOVER_LIGHTEN_FACTOR);
            assert!(
                luminance(l) > luminance(c),
                "{l:?} should be lighter than {c:?}"
            );
        }
    }

    #[test]
    fn lighten_preserves_alpha() {
        let c = Color::from_rgba(0.5, 0.4, 0.3, 0.42);
        assert_eq!(lighten(c, CARD_HOVER_LIGHTEN_FACTOR).a, 0.42);
    }

    #[test]
    fn lighten_keeps_white_white() {
        let white = Color::from_rgb(1.0, 1.0, 1.0);
        let l = lighten(white, CARD_HOVER_LIGHTEN_FACTOR);
        assert_eq!(l, white);
    }
}
