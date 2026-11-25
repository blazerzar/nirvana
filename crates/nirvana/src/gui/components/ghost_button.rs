use iced::widget::button;
use iced::{Border, Element, Theme};

use crate::gui::design_system::typography::BODY;
use crate::gui::design_system::{radius, space, tokens};

/// A borderless text button without a background until it is hovered.
pub(in crate::gui) fn ghost_button<'a, Message: Clone + 'a>(
    label: &'a str,
    on_press: Message,
) -> Element<'a, Message> {
    button(BODY.text(label))
        .padding([space::S0, space::S050])
        .on_press(on_press)
        .style(ghost_style)
        .into()
}

pub(in crate::gui) fn ghost_style(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let (background, text_color) = match status {
        button::Status::Hovered => (Some(tokens.fill_hover), tokens.text_primary),
        button::Status::Pressed => (Some(tokens.fill_pressed), tokens.text_primary),
        button::Status::Active | button::Status::Disabled => (None, tokens.text_tertiary),
    };
    button::Style {
        background: background.map(Into::into),
        text_color,
        border: Border::default().rounded(radius::SM),
        ..button::Style::default()
    }
}
