//! Theme-derived styles for the notes list: search bar, cards, and the
//! header and card icon buttons.

use cosmic::iced::{Border, Color};

use crate::palette::Colour;

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

/// A palette RGB triple as an iced `Color`.
pub(super) fn rgb((r, g, b): (u8, u8, u8)) -> Color {
    Color::from_rgb8(r, g, b)
}

/// A card's style: the theme's `Card` appearance with the note's colour as
/// its background, the note's text colour for its text and icons, and
/// `CARD_RADIUS` corners.
pub(super) fn card_container_style(
    colour: Colour,
    theme: &cosmic::Theme,
) -> cosmic::iced::widget::container::Style {
    let mut style = <cosmic::Theme as cosmic::iced::widget::container::Catalog>::style(
        theme,
        &cosmic::theme::Container::Card,
    );
    if let (Some(paper), Some(text)) = (colour.paper(), colour.text()) {
        style.background = Some(rgb(paper).into());
        style.text_color = Some(rgb(text));
        style.icon_color = Some(rgb(text));
    }
    style.border.radius = CARD_RADIUS.into();
    style
}

/// The note window's paper, below the title bar: the theme's window background
/// with the note's colour as its fill and the note's text colour. Square at the
/// top, where it meets the title bar; the window's own rounding at the bottom.
pub(super) fn note_paper_style(
    colour: Colour,
) -> impl Fn(&cosmic::Theme) -> cosmic::iced::widget::container::Style {
    move |theme| {
        let mut style = <cosmic::Theme as cosmic::iced::widget::container::Catalog>::style(
            theme,
            &cosmic::theme::Container::WindowBackground,
        );
        // Always opaque, even with COSMIC's frosted glass on: only the title
        // bar is see-through. Default uses the theme's solid background.
        let solid = theme.cosmic().background(false);
        let paper = colour.paper().map_or_else(|| Color::from(solid.base), rgb);
        let text = colour.text().map_or_else(|| Color::from(solid.on), rgb);
        style.background = Some(paper.into());
        style.text_color = Some(text);
        style.icon_color = Some(text);
        style.border.radius.top_left = 0.0;
        style.border.radius.top_right = 0.0;
        style
    }
}

/// The note window's title bar: the theme's window background - translucent
/// when COSMIC's frosted glass is on - rounded only at the top to follow the
/// window's corners, square where the paper starts.
pub(super) fn note_title_bar_style(
    theme: &cosmic::Theme,
) -> cosmic::iced::widget::container::Style {
    let mut style = <cosmic::Theme as cosmic::iced::widget::container::Catalog>::style(
        theme,
        &cosmic::theme::Container::WindowBackground,
    );
    style.border.radius.bottom_left = 0.0;
    style.border.radius.bottom_right = 0.0;
    style
}

/// One swatch in the colour dialog: the note colour (the theme's background
/// for Default), 4 px corners, and an accent border when it's the note's
/// current colour.
pub(super) fn swatch_style(
    colour: Colour,
    selected: bool,
) -> impl Fn(&cosmic::Theme) -> cosmic::iced::widget::container::Style {
    move |theme| {
        let cosmic = theme.cosmic();
        cosmic::iced::widget::container::Style {
            background: Some(
                colour
                    .paper()
                    .map_or_else(
                        || Color::from(cosmic.background(theme.transparent).base),
                        rgb,
                    )
                    .into(),
            ),
            border: Border {
                radius: CARD_RADIUS.into(),
                width: if selected { 2.0 } else { 1.0 },
                color: if selected {
                    cosmic.accent.base.into()
                } else {
                    cosmic.background(theme.transparent).divider.into()
                },
            },
            ..cosmic::iced::widget::container::Style::default()
        }
    }
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
pub(super) fn card_heading_style(
    colour: Colour,
) -> impl Fn(&cosmic::Theme) -> cosmic::iced::widget::container::Style {
    move |theme| {
        let card = card_container_style(colour, theme);
        cosmic::iced::widget::container::Style {
            // A coloured card's own deeper shade; Default darkens the theme's card.
            background: colour.heading().map_or_else(
                || {
                    card.background.map(|background| match background {
                        cosmic::iced::Background::Color(c) => {
                            cosmic::iced::Background::Color(darken(c, HEADING_DARKEN_FACTOR))
                        }
                        gradient @ cosmic::iced::Background::Gradient(_) => gradient,
                    })
                },
                |heading| Some(rgb(heading).into()),
            ),
            border: Border {
                radius: cosmic::iced::border::top(CARD_RADIUS),
                ..Border::default()
            },
            ..card
        }
    }
}

/// The card's background colour alone (see `card_container_style`),
/// for building the card button's hover/press variants. Falls back to
/// transparent for the (never-hit-in-practice) case that the theme's Card
/// container has no solid colour background.
pub(super) fn card_color(colour: Colour, theme: &cosmic::Theme) -> Color {
    match card_container_style(colour, theme).background {
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
    colour: Colour,
    focused: bool,
    background: Color,
) -> cosmic::widget::button::Style {
    let card = card_container_style(colour, theme);
    let mut style = cosmic::widget::button::Style {
        background: Some(cosmic::iced::Background::Color(background)),
        text_color: card.text_color,
        icon_color: card.icon_color,
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
pub(super) fn card_button_class(colour: Colour) -> cosmic::theme::Button {
    cosmic::theme::Button::Custom {
        active: Box::new(move |focused, theme| {
            card_button_style(theme, colour, focused, card_color(colour, theme))
        }),
        disabled: Box::new(move |theme| {
            card_button_style(theme, colour, false, card_color(colour, theme))
        }),
        hovered: Box::new(move |focused, theme| {
            card_button_style(
                theme,
                colour,
                focused,
                lighten(card_color(colour, theme), CARD_HOVER_LIGHTEN_FACTOR),
            )
        }),
        pressed: Box::new(move |focused, theme| {
            card_button_style(
                theme,
                colour,
                focused,
                darken(card_color(colour, theme), CARD_PRESS_DARKEN_FACTOR),
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
    // Destructive: the icon sits on a red badge (`icon_hover_background`), so
    // it takes the colour drawn on that red.
    let cosmic = theme.cosmic();
    match role {
        IconHoverRole::Accent => cosmic.accent.base.into(),
        IconHoverRole::Destructive => cosmic.destructive.on.into(),
    }
}

/// COSMIC's grey for secondary text and icons: the colour libcosmic gives an
/// unfocused window's header bar - the background's component foreground,
/// mixed halfway into the background itself.
pub(super) fn muted_color(theme: &cosmic::Theme) -> Color {
    let background = theme.cosmic().background(theme.transparent);
    let on = Color::from(background.component.on);
    let base = Color::from(background.base);
    Color {
        r: f32::midpoint(on.r, base.r),
        g: f32::midpoint(on.g, base.g),
        b: f32::midpoint(on.b, base.b),
        a: on.a,
    }
}

/// `icon_button_class`, resting in `rest` instead of the inherited icon colour
/// when it's `Some`. For a coloured card's rename and delete icons.
pub(super) fn tinted_icon_button_class(
    role: IconHoverRole,
    rest: Option<Color>,
) -> cosmic::theme::Button {
    let Some(rest) = rest else {
        return icon_button_class(role);
    };
    let cosmic::theme::Button::Custom {
        active,
        disabled,
        hovered,
        pressed,
    } = icon_button_class(role)
    else {
        unreachable!("icon_button_class always builds a custom class");
    };
    cosmic::theme::Button::Custom {
        active: Box::new(move |focused, theme| {
            let mut style = active(focused, theme);
            style.icon_color = Some(rest);
            style
        }),
        disabled,
        hovered,
        pressed,
    }
}

/// The background a hovered icon button shows, if any: none for accent
/// buttons, a red badge (the theme's destructive red) for delete - readable on
/// every note colour. `pressed` darkens it slightly.
pub(super) fn icon_hover_background(
    role: IconHoverRole,
    pressed: bool,
    theme: &cosmic::Theme,
) -> Option<cosmic::iced::Background> {
    match role {
        IconHoverRole::Accent => None,
        IconHoverRole::Destructive => {
            let red: Color = theme.cosmic().destructive.base.into();
            let red = if pressed {
                darken(red, CARD_PRESS_DARKEN_FACTOR)
            } else {
                red
            };
            Some(red.into())
        }
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
            style.background = icon_hover_background(role, false, theme);
            style.border_radius = CARD_RADIUS.into();
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
            style.background = icon_hover_background(role, true, theme);
            style.border_radius = CARD_RADIUS.into();
            style
        }),
    }
}

/// COSMIC's default text-button style (its own hover and pressed backgrounds),
/// with the label in COSMIC's grey (`muted_color`) at rest, turning the accent
/// colour while hovered or pressed - the same hover colour the icon buttons use
/// (`icon_hover_color`).
pub(super) fn text_button_class() -> cosmic::theme::Button {
    fn with_accent_text(
        mut style: cosmic::widget::button::Style,
        theme: &cosmic::Theme,
    ) -> cosmic::widget::button::Style {
        style.text_color = Some(icon_hover_color(IconHoverRole::Accent, theme));
        style
    }
    cosmic::theme::Button::Custom {
        active: Box::new(|focused, theme| {
            let mut style = <cosmic::Theme as cosmic::widget::button::Catalog>::active(
                theme,
                focused,
                false,
                &cosmic::theme::Button::Text,
            );
            style.text_color = Some(muted_color(theme));
            style
        }),
        disabled: Box::new(|theme| {
            <cosmic::Theme as cosmic::widget::button::Catalog>::disabled(
                theme,
                &cosmic::theme::Button::Text,
            )
        }),
        hovered: Box::new(|focused, theme| {
            with_accent_text(
                <cosmic::Theme as cosmic::widget::button::Catalog>::hovered(
                    theme,
                    focused,
                    false,
                    &cosmic::theme::Button::Text,
                ),
                theme,
            )
        }),
        pressed: Box::new(|focused, theme| {
            with_accent_text(
                <cosmic::Theme as cosmic::widget::button::Catalog>::pressed(
                    theme,
                    focused,
                    false,
                    &cosmic::theme::Button::Text,
                ),
                theme,
            )
        }),
    }
}

/// Corner radius of a note menu item's hover and pressed background.
pub(super) const MENU_ITEM_RADIUS: f32 = 4.0;

/// COSMIC's menu-item style, with `MENU_ITEM_RADIUS` corners on its hover and
/// pressed background.
pub(super) fn menu_item_button_class() -> cosmic::theme::Button {
    fn rounded(mut style: cosmic::widget::button::Style) -> cosmic::widget::button::Style {
        style.border_radius = MENU_ITEM_RADIUS.into();
        style
    }
    cosmic::theme::Button::Custom {
        active: Box::new(|focused, theme| {
            rounded(<cosmic::Theme as cosmic::widget::button::Catalog>::active(
                theme,
                focused,
                false,
                &cosmic::theme::Button::MenuItem,
            ))
        }),
        disabled: Box::new(|theme| {
            rounded(
                <cosmic::Theme as cosmic::widget::button::Catalog>::disabled(
                    theme,
                    &cosmic::theme::Button::MenuItem,
                ),
            )
        }),
        hovered: Box::new(|focused, theme| {
            rounded(<cosmic::Theme as cosmic::widget::button::Catalog>::hovered(
                theme,
                focused,
                false,
                &cosmic::theme::Button::MenuItem,
            ))
        }),
        pressed: Box::new(|focused, theme| {
            rounded(<cosmic::Theme as cosmic::widget::button::Catalog>::pressed(
                theme,
                focused,
                false,
                &cosmic::theme::Button::MenuItem,
            ))
        }),
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp)] // exact pixel values are the point of these tests
mod tests {
    use super::*;

    #[test]
    fn muted_color_is_not_the_hover_accent() {
        for theme in [cosmic::Theme::dark(), cosmic::Theme::light()] {
            assert_ne!(
                muted_color(&theme),
                icon_hover_color(IconHoverRole::Accent, &theme),
                "at rest the Settings label and + must not already look hovered"
            );
        }
    }

    #[test]
    fn hovered_delete_is_a_red_badge_with_the_colour_drawn_on_red() {
        let theme = cosmic::Theme::dark();
        let red: Color = theme.cosmic().destructive.base.into();
        let on_red: Color = theme.cosmic().destructive.on.into();
        assert_eq!(
            icon_hover_background(IconHoverRole::Destructive, false, &theme),
            Some(red.into())
        );
        assert_eq!(icon_hover_color(IconHoverRole::Destructive, &theme), on_red);
        assert_eq!(
            icon_hover_background(IconHoverRole::Accent, false, &theme),
            None
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
