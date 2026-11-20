use iced::widget::{Column, container, text};
use iced::{Element, Length};

use super::header::header;
use crate::gui::app::Message;
use crate::gui::theme::tokens;

pub(in crate::gui) fn main(error: Option<&str>) -> Element<'_, Message> {
    let error = error.map(|e| {
        container(text(e).style(|theme| text::Style {
            color: Some(tokens(theme).status_danger),
        }))
        .padding(16)
        .width(Length::Fill)
    });
    container(Column::new().push(header()).push(error))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default))
        .into()
}
