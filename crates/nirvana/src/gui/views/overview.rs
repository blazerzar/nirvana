use iced::widget::column;
use iced::{Element, Length, Padding};

use super::{done_bar, screen_heading};
use crate::gui::app::{Message, Screen};
use crate::gui::design_system::space;

pub(in crate::gui) fn overview_view(profile: &str) -> Element<'_, Message> {
    let heading = screen_heading(
        "overview",
        format!("profile {profile} · includes the running slot"),
    )
    .padding(Padding::from([space::S200, space::S300]).bottom(space::S050));

    column![heading].height(Length::Fill).into()
}

pub(in crate::gui) fn overview_bar<'a>() -> Element<'a, Message> {
    done_bar(
        "per-ticket breakdown and month view are in",
        Screen::Overview,
    )
}
