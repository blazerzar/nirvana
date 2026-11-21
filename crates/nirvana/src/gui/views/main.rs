use iced::widget::{Column, container, responsive, row, text};
use iced::{Element, Length, Size};

use super::bottom_bar::bottom_bar;
use super::header::header;
use super::listing::listing;
use super::tracking::tracking;
use crate::gui::app::Message;
use crate::gui::design_system::tokens;
use crate::gui::profile::ProfileState;

pub(in crate::gui) const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);

pub(in crate::gui) fn main<'a>(
    error: Option<&'a str>,
    profile: Option<&'a ProfileState>,
) -> Element<'a, Message> {
    let error = error.map(|e| {
        container(text(e).style(|theme| text::Style {
            color: Some(tokens(theme).status_danger),
        }))
        .padding(16)
        .width(Length::Fill)
    });
    let body = responsive(|size| row![tracking(), listing(size.width)].into());
    let layout = Column::new()
        .push(header(profile))
        .push(error)
        .push(body)
        .push(bottom_bar());
    container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default))
        .into()
}
