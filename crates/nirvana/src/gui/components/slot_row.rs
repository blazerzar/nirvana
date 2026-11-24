use iced::alignment::Vertical;
use iced::widget::text::Wrapping;
use iced::widget::{container, row, text};
use iced::{Border, Color, Element, Length, Padding, Theme};
use nirvana_core::PublishState;

use super::ellipsized_text;
use crate::gui::design_system::typography::{BODY, BODY_LG, LABEL_STRONG, MICRO, TextStyle};
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

    fn style(self) -> TextStyle {
        match self {
            Self::Stopped(PublishState::Unpublished | PublishState::Pending) => MICRO,
            Self::Running | Self::Stopped(PublishState::Failed | PublishState::Published) => {
                BODY_LG
            }
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

    pub(super) fn is_running(self) -> bool {
        matches!(self, Self::Running)
    }
}

/// A slot of a ticket: its mark, time range, key and title, and duration.
/// The running slot is accented on an accent background.
pub(in crate::gui) fn slot_row<'a, Message: 'a>(
    mark: Mark,
    range: (String, String),
    key: &'a str,
    title: &'a str,
    duration: String,
) -> Element<'a, Message> {
    let is_running = mark.is_running();
    let content = row![
        mark_glyph(mark),
        container(time_range(BODY_LG, range, is_running)).width(TIME_RANGE_WIDTH),
        ticket(key, title, is_running),
        duration_text(
            BODY_LG,
            duration,
            accented(is_running, |tokens| tokens.text_primary)
        ),
    ]
    .spacing(space::S100)
    .align_y(Vertical::Center);
    slot_container(content, [space::S075, space::S100].into(), is_running)
}

/// The key and title of a ticket, with the key accented while it is
/// running.
pub(super) fn ticket<'a, Message: 'a>(
    key: &'a str,
    title: &'a str,
    is_running: bool,
) -> Element<'a, Message> {
    let key = LABEL_STRONG
        .text(key)
        .wrapping(Wrapping::None)
        .style(colored(accented(is_running, |tokens| tokens.text_primary)));
    let title = ellipsized_text(BODY, title, tertiary);
    row![key, container(title).clip(true).width(Length::Fill)]
        .spacing(space::S100)
        .align_y(Vertical::Center)
        .into()
}

pub(super) fn mark_glyph<'a, Message: 'a>(mark: Mark) -> Element<'a, Message> {
    container(
        mark.style()
            .text(mark.glyph())
            .style(colored(move |tokens| mark.color(tokens))),
    )
    .width(MARK_WIDTH)
    .into()
}

/// The `start` and `stop` of a slot, accented while it is running.
pub(super) fn time_range<'a, Message: 'a>(
    style: TextStyle,
    (start, stop): (String, String),
    is_running: bool,
) -> Element<'a, Message> {
    let color = accented(is_running, |tokens| tokens.text_secondary);
    row![
        style.text(start).style(colored(color)),
        style
            .text("–")
            .style(colored(|tokens| tokens.text_disabled)),
        style.text(stop).style(colored(color)),
    ]
    .into()
}

/// The `duration`, right aligned in at least the width of `1h 10m`.
pub(super) fn duration_text<'a, Message: 'a>(
    style: TextStyle,
    duration: String,
    color: impl Fn(&Tokens) -> Color + 'a,
) -> Element<'a, Message> {
    let width = style.mono_width(duration.chars().count().max(DURATION_MIN.chars().count()));
    container(
        style
            .text(duration)
            .wrapping(Wrapping::None)
            .style(colored(color)),
    )
    .align_right(width)
    .into()
}

/// The `content` of a slot, on an accent background while it is running.
pub(super) fn slot_container<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
    padding: Padding,
    is_running: bool,
) -> Element<'a, Message> {
    container(content)
        .width(Length::Fill)
        .padding(padding)
        .style(move |theme| container::Style {
            background: is_running.then(|| tokens(theme).fill_accent_subtle.into()),
            border: Border::default().rounded(radius::SM),
            ..container::Style::default()
        })
        .into()
}

/// Returns the accent text color while running, and `otherwise` when not.
pub(super) fn accented(
    is_running: bool,
    otherwise: fn(&Tokens) -> Color,
) -> impl Fn(&Tokens) -> Color + Copy {
    move |tokens| {
        if is_running {
            tokens.text_accent
        } else {
            otherwise(tokens)
        }
    }
}

pub(super) fn colored(color: impl Fn(&Tokens) -> Color) -> impl Fn(&iced::Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(tokens(theme))),
    }
}

pub(super) fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
