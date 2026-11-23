use iced::alignment::Vertical;
use iced::widget::{column, container, row, text};
use iced::{Element, Length, Padding, Theme};

use super::listing::side_pane;
use crate::gui::app::Message;
use crate::gui::components::{Button, RETURN_KEY, number_badge, tracked_text};
use crate::gui::design_system::typography::{
    BODY, HEADING_PAGE, LABEL_CAPS, LABEL_STRONG, PARAGRAPH,
};
use crate::gui::design_system::{space, tokens};
use crate::gui::state::settings;

pub(super) fn setup<'a>(
    available_width: f32,
    active_step: u8,
    web_available: bool,
) -> Element<'a, Message> {
    let steps = column![
        tracked_text(LABEL_CAPS, "SETUP · 3 STEPS", tertiary),
        step(
            1,
            "create a profile",
            "a name and a database for its logs. most people start with work.",
            active_step == 1,
        ),
        step(
            2,
            "connect Jira or Linear",
            "required. ticket names and descriptions come from it; worklogs publish to it.",
            active_step == 2,
        ),
        step(
            3,
            "start tracking",
            "come back here, press s and type a ticket key.",
            active_step == 3,
        ),
    ]
    .spacing(space::S200)
    .padding(Padding::from(space::S200).right(space::S250));
    row![welcome(web_available), side_pane(steps, available_width)].into()
}

fn step<'a>(
    number: u8,
    title: &'a str,
    description: &'a str,
    active: bool,
) -> Element<'a, Message> {
    let title = container(LABEL_STRONG.text(title).style(|theme| text::Style {
        color: Some(tokens(theme).text_primary),
    }))
    .height(24)
    .align_y(Vertical::Center);
    let description = BODY.text(description).style(tertiary);
    row![number_badge(number, active), column![title, description]]
        .spacing(space::S150)
        .into()
}

fn welcome<'a>(web_available: bool) -> Element<'a, Message> {
    let label = tracked_text(LABEL_CAPS, "WELCOME", tertiary);
    let heading = HEADING_PAGE
        .text("Set up nirvana in the browser")
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_primary),
        });
    let copy = container(
        PARAGRAPH
            .text(
                "nirvana needs a profile connected to Jira or Linear to \
                 look up ticket names and descriptions. both are set up \
                 in nirvana web and take about a minute.",
            )
            .style(|theme| text::Style {
                color: Some(tokens(theme).text_secondary),
            }),
    )
    .max_width(440);
    let open_web = Button::new("open nirvana web ↗").key(RETURN_KEY);
    let open_web = if web_available {
        open_web.on_press(Message::Settings(settings::Message::OpenWeb))
    } else {
        open_web
    };

    let content = column![
        label,
        heading,
        copy,
        iced::widget::space::vertical(),
        open_web
    ]
    .spacing(space::S150);
    container(content)
        .padding(space::S300)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
