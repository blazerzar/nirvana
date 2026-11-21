use iced::widget::{column, container, row, rule};
use iced::{Element, Length};

use super::{MAIN_WINDOW_SIZE, border};
use crate::gui::app::Message;
use crate::gui::design_system::tokens;

const MIN_WIDTH: f32 = 400.0;
const WINDOW_SHARE: f32 = MIN_WIDTH / MAIN_WINDOW_SIZE.width;

pub(super) fn listing<'a>(available_width: f32) -> Element<'a, Message> {
    side_pane(column![], available_width)
}

pub(super) fn side_pane<'a>(
    content: impl Into<Element<'a, Message>>,
    available_width: f32,
) -> Element<'a, Message> {
    let width = (available_width * WINDOW_SHARE).max(MIN_WIDTH);
    let pane = container(content)
        .width(width)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_subtle));

    row![border(rule::vertical(1)), pane].into()
}
