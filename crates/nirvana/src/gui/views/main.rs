use iced::widget::{Column, container, text};
use iced::{Element, Length};

use super::header::header;
use crate::gui::app::Message;
use crate::gui::theme::Tokens;

pub(in crate::gui) fn main<'a>(
    tokens: &'static Tokens,
    error: Option<&'a str>,
) -> Element<'a, Message> {
    let error = error.map(|e| {
        container(text(e).color(tokens.status_danger))
            .padding(16)
            .width(Length::Fill)
    });
    container(Column::new().push(header(tokens)).push(error))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::background(tokens.bg_default))
        .into()
}
