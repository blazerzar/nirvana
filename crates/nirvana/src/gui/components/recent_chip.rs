use iced::alignment::Vertical;
use iced::widget::{button, row};
use iced::{Border, Element};

use super::connection_tag;
use crate::gui::design_system::typography::BODY;
use crate::gui::design_system::{radius, space, tokens};
use crate::gui::state::start::Recent;

/// A recent ticket that publishes `on_press` when clicked.
pub(in crate::gui) fn recent_chip<'a, Message: Clone + 'a>(
    recent: &'a Recent,
    on_press: Message,
) -> Element<'a, Message> {
    let content = row![BODY.text(&recent.key)]
        .push(recent.tag.as_deref().map(connection_tag))
        .spacing(space::S075)
        .align_y(Vertical::Center);
    button(content)
        .padding([space::S050, space::S100])
        .on_press(on_press)
        .style(|theme, status| {
            let tokens = tokens(theme);
            let (background, text) = match status {
                button::Status::Hovered => (tokens.fill_hover, tokens.text_primary),
                button::Status::Pressed => (tokens.fill_pressed, tokens.text_primary),
                _ => (tokens.bg_raised, tokens.text_secondary),
            };
            button::Style {
                background: Some(background.into()),
                text_color: text,
                border: Border {
                    color: tokens.border_default,
                    width: 1.0,
                    radius: radius::SM.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}
