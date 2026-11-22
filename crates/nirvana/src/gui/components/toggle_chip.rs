use iced::widget::button;
use iced::{Border, Element};

use crate::gui::design_system::typography::{CAPTION, CAPTION_STRONG};
use crate::gui::design_system::{radius, space, tokens};

/// One option of a multi-select group. Disabled until `on_press` is set.
pub(in crate::gui) fn toggle_chip<'a, Message: Clone + 'a>(
    label: &'a str,
    is_on: bool,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    let content = if is_on {
        CAPTION_STRONG.text(format!("✓ {label}"))
    } else {
        CAPTION.text(label)
    };
    button(content)
        .padding([space::S050, space::S100])
        .on_press_maybe(on_press)
        .style(move |theme, status| {
            let tokens = tokens(theme);
            let (background, text, border) = match (is_on, status) {
                (true, button::Status::Hovered | button::Status::Pressed) => (
                    tokens.fill_accent,
                    tokens.text_on_accent,
                    tokens.border_accent,
                ),
                (true, _) => (
                    tokens.fill_accent_subtle,
                    tokens.text_accent,
                    tokens.border_accent,
                ),
                (false, button::Status::Hovered) => (
                    tokens.fill_hover,
                    tokens.text_primary,
                    tokens.border_default,
                ),
                (false, button::Status::Pressed) => (
                    tokens.fill_pressed,
                    tokens.text_primary,
                    tokens.border_default,
                ),
                (false, _) => (
                    tokens.bg_raised,
                    tokens.text_secondary,
                    tokens.border_default,
                ),
            };
            let opacity = tokens.opacity(status != button::Status::Disabled);
            button::Style {
                background: Some(background.scale_alpha(opacity).into()),
                text_color: text.scale_alpha(opacity),
                border: Border {
                    color: border.scale_alpha(opacity),
                    width: 1.0,
                    radius: radius::SM.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}
