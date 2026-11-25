mod bottom_bar;
mod header;
mod info;
mod listing;
mod main;
mod overview;
mod publish;
mod settings;
mod setup;
mod start;
mod tracking;

pub(super) use header::header;
use iced::alignment::Vertical;
use iced::widget::rule::{self, Rule};
use iced::widget::text::IntoFragment;
use iced::widget::{Row, column, container, row, stack, text};
use iced::{Element, Length, Padding};
pub(super) use info::{info, info_size};
pub(super) use main::{MAIN_WINDOW_SIZE, main, main_screen};
pub(super) use overview::{overview_bar, overview_view};
pub(super) use publish::publish_modal;
pub(super) use settings::{settings_bar, settings_view};

use crate::gui::app::{Message, Screen};
use crate::gui::components::{self, Button, Size, link};
use crate::gui::design_system::typography::{BODY, HEADING_SCREEN};
use crate::gui::design_system::{space, tokens};
use crate::gui::state;
use crate::gui::state::toast::{self, Toast};

const DONE_BAR_HEIGHT: f32 = 52.0;

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

/// The title of a screen with a note next to it.
fn screen_heading<'a>(title: &'a str, note: impl IntoFragment<'a>) -> Row<'a, Message> {
    row![
        HEADING_SCREEN.text(title).style(|theme| text::Style {
            color: Some(tokens(theme).text_primary),
        }),
        BODY.text(note).style(|theme| text::Style {
            color: Some(tokens(theme).text_tertiary),
        }),
    ]
    .spacing(space::S150)
    .align_y(Vertical::Bottom)
}

/// The bottom bar of a screen: a note pointing to nirvana web, a link while
/// it is available, and a done button that goes back to the main screen.
fn done_bar<'a>(note: &'a str, screen: Screen, web_available: bool) -> Element<'a, Message> {
    let note = row![
        BODY.text(note).style(|theme| text::Style {
            color: Some(tokens(theme).text_tertiary),
        }),
        link(
            BODY.text(if web_available {
                "nirvana web ↗"
            } else {
                "nirvana web"
            }),
            web_available.then_some(Message::Settings(state::settings::Message::OpenWeb)),
        ),
    ]
    .spacing(space::S075);
    let done = Button::new("done")
        .size(Size::Sm)
        .key("esc")
        .primary()
        .on_press(Message::ToggleScreen(screen));
    let bar = container(
        row![note, iced::widget::space::horizontal(), done]
            .spacing(space::S100)
            .align_y(Vertical::Center),
    )
    .height(DONE_BAR_HEIGHT)
    .padding(Padding::from([space::S0, space::S200]).left(space::S300))
    .align_y(Vertical::Center);

    column![border(rule::horizontal(1)), bar].into()
}

fn border(line: Rule<'_>) -> Rule<'_> {
    line.style(|theme| rule::Style {
        color: tokens(theme).border_default,
        ..rule::default(theme)
    })
}
