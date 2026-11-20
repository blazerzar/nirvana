use iced::widget::{Column, container};
use iced::{Element, Length};

use super::header::header;
use crate::gui::app::Message;
use crate::gui::theme::Tokens;

pub(in crate::gui) fn main<'a>(tokens: &'static Tokens) -> Element<'a, Message> {
    container(Column::new().push(header(tokens)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|_| container::background(tokens.bg_default))
        .into()
}
