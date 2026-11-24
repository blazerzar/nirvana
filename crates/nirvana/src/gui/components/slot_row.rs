use iced::alignment::Vertical;
use iced::widget::text::Wrapping;
use iced::widget::{container, row, text};
use iced::{Border, Color, Element, Length, Theme};
use nirvana_core::PublishState;

use super::ellipsized_text;
use crate::gui::design_system::typography::{BODY, BODY_LG, LABEL_STRONG};
use crate::gui::design_system::{Tokens, radius, space, tokens};

const MARK_WIDTH: f32 = 14.0;
const TIME_RANGE_WIDTH: f32 = 92.0;
const DURATION_MIN: &str = "1h 10m";

/// The mark in front of a slot: running, or the publish state of a
/// stopped one.
#[derive(Clone, Copy)]
pub(in crate::gui) enum Mark {
    Running,
    Stopped(PublishState),
}

impl Mark {
    fn glyph(self) -> &'static str {
        match self {
            Self::Running => "►",
            Self::Stopped(PublishState::Unpublished) => "●",
            Self::Stopped(PublishState::Pending) => "○",
            Self::Stopped(PublishState::Failed) => "×",
            Self::Stopped(PublishState::Published) => "✓",
        }
    }

    fn color(self, tokens: &Tokens) -> Color {
        match self {
            Self::Running | Self::Stopped(PublishState::Unpublished) => tokens.text_accent,
            Self::Stopped(PublishState::Pending) => tokens.status_warning,
            Self::Stopped(PublishState::Failed) => tokens.status_danger,
            Self::Stopped(PublishState::Published) => tokens.status_success,
        }
    }
}

/// A slot of a ticket: its mark, time range, key and title, and duration.
/// The running slot is accented on an accent background.
pub(in crate::gui) fn slot_row<'a, Message: 'a>(
    mark: Mark,
    (start, stop): (String, String),
    key: &'a str,
    title: &'a str,
    duration: String,
) -> Element<'a, Message> {
    let is_running = matches!(mark, Mark::Running);
    let time_color = move |tokens: &Tokens| {
        if is_running {
            tokens.text_accent
        } else {
            tokens.text_secondary
        }
    };
    let value_color = move |tokens: &Tokens| {
        if is_running {
            tokens.text_accent
        } else {
            tokens.text_primary
        }
    };
    let time_range = row![
        BODY_LG.text(start).style(colored(time_color)),
        BODY_LG
            .text("–")
            .style(colored(|tokens| tokens.text_disabled)),
        BODY_LG.text(stop).style(colored(time_color)),
    ];
    let key = LABEL_STRONG
        .text(key)
        .wrapping(Wrapping::None)
        .style(colored(value_color));
    let title = ellipsized_text(BODY, title, tertiary);
    let duration_width =
        BODY_LG.mono_width(duration.chars().count().max(DURATION_MIN.chars().count()));
    let duration = BODY_LG
        .text(duration)
        .wrapping(Wrapping::None)
        .style(colored(value_color));
    let content = row![
        container(
            BODY_LG
                .text(mark.glyph())
                .style(colored(move |tokens| mark.color(tokens)))
        )
        .width(MARK_WIDTH),
        container(time_range).width(TIME_RANGE_WIDTH),
        row![key, container(title).clip(true).width(Length::Fill)]
            .spacing(space::S100)
            .align_y(Vertical::Center),
        container(duration).align_right(duration_width),
    ]
    .spacing(space::S100)
    .align_y(Vertical::Center);
    container(content)
        .width(Length::Fill)
        .padding([space::S075, space::S100])
        .style(move |theme| container::Style {
            background: is_running.then(|| tokens(theme).fill_accent_subtle.into()),
            border: Border::default().rounded(radius::SM),
            ..container::Style::default()
        })
        .into()
}

fn colored(color: impl Fn(&Tokens) -> Color) -> impl Fn(&iced::Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(tokens(theme))),
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
