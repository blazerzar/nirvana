use iced::widget::{column, container};
use iced::{Element, Length};

use crate::gui::app::Message;

pub(super) fn tracking<'a>() -> Element<'a, Message> {
    container(column![])
        .padding(24)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}
