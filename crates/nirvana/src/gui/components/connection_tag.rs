use iced::widget::{container, text};
use iced::{Border, Element};

use crate::gui::design_system::typography::MICRO;
use crate::gui::design_system::{radius, space, tokens};

/// The connection of a ticket whose key exists in more than one connection.
pub(in crate::gui) fn connection_tag<'a, Message: 'a>(connection: &'a str) -> Element<'a, Message> {
    let label = MICRO.text(connection).style(|theme| text::Style {
        color: Some(tokens(theme).text_secondary),
    });
    container(label)
        .padding([space::S0, space::S050])
        .style(|theme| {
            container::Style::default()
                .background(tokens(theme).bg_default)
                .border(Border::default().rounded(radius::XS))
        })
        .into()
}
