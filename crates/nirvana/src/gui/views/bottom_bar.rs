use iced::alignment::Vertical;
use iced::widget::{column, container, row, rule};
use iced::{Element, Length};

use super::border;
use crate::gui::app::Message;

pub(super) fn bottom_bar<'a>() -> Element<'a, Message> {
    let bar = container(row![].spacing(8).align_y(Vertical::Center))
        .width(Length::Fill)
        .height(60)
        .padding([0, 16])
        .align_y(Vertical::Center);

    column![border(rule::horizontal(1)), bar].into()
}
