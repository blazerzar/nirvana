use iced::alignment::Vertical;
use iced::widget::{Row, column, container, rule};
use iced::{Element, Length};

use super::border;
use crate::gui::app::Message;
use crate::gui::design_system::space;

pub(super) fn bottom_bar(content: Row<'_, Message>) -> Element<'_, Message> {
    let bar = container(content.spacing(space::S100).align_y(Vertical::Center))
        .width(Length::Fill)
        .height(60)
        .padding([space::S0, space::S200])
        .align_y(Vertical::Center);

    column![border(rule::horizontal(1)), bar].into()
}
