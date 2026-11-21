mod bottom_bar;
mod header;
mod info;
mod listing;
mod main;
mod setup;
mod tracking;

use iced::widget::rule::{self, Rule};
use iced::widget::{container, stack};
use iced::{Element, Length, Padding};
pub(super) use info::{info, info_size};
pub(super) use main::{MAIN_WINDOW_SIZE, main};

use crate::gui::app::Message;
use crate::gui::components;
use crate::gui::design_system::tokens;
use crate::gui::toast::{self, Toast};

fn with_toast<'a>(
    content: Element<'a, Message>,
    toast: Option<&'a Toast>,
    bottom: f32,
) -> Element<'a, Message> {
    let layer = toast.map(|toast| {
        container(components::toast(
            toast,
            Message::Toast(toast::Message::Hide(toast.id)),
        ))
        .center_x(Length::Fill)
        .align_bottom(Length::Fill)
        .padding(Padding::ZERO.bottom(bottom))
    });
    stack![content].push(layer).into()
}

fn border(line: Rule<'_>) -> Rule<'_> {
    line.style(|theme| rule::Style {
        color: tokens(theme).border_default,
        ..rule::default(theme)
    })
}
