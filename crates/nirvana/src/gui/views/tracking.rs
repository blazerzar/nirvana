use chrono::{DateTime, Local};
use iced::alignment::Vertical;
use iced::widget::{Row, Space, column, container, rich_text, row, space, span, text};
use iced::{Border, Element, Length, Theme, padding};
use nirvana_core::Slot;

use crate::gui::app::Message;
use crate::gui::components::{recent_chip, tracked_text};
use crate::gui::design_system::font::MONO_SEMIBOLD;
use crate::gui::design_system::typography::{
    BODY, BODY_LG, CAPTION, DISPLAY_KEY, DISPLAY_TIMER, LABEL_CAPS, PARAGRAPH, TITLE,
};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::tracking::{self, clock};
use crate::time::format_duration;

const PULSE_DOT_SIZE: f32 = 7.0;
const HINT_MAX_WIDTH: f32 = 400.0;
const RESUME_HINT: &str =
    "resume it, or pick a ticket to start. logs stay on this machine until you publish.";
const NOTHING_TODAY_HINT: &str = concat!(
    "nothing tracked today yet. pick a recent ticket or press s to start. ",
    "logs stay on this machine until you publish.",
);

pub(super) fn tracking<'a>(
    state: &'a tracking::State,
    tokens: &'static Tokens,
) -> Element<'a, Message> {
    let content = match state.running() {
        Some(running) => tracking_now(running, state),
        None => not_tracking(state, tokens),
    };
    container(content)
        .padding(spacing::S300)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn tracking_now<'a>(running: &'a Slot, state: &'a tracking::State) -> Element<'a, Message> {
    let pulse = state.pulse_opacity();
    let dot = container(space())
        .width(PULSE_DOT_SIZE)
        .height(PULSE_DOT_SIZE)
        .style(move |theme| container::Style {
            background: Some(tokens(theme).accent.scale_alpha(pulse).into()),
            border: Border::default().rounded(radius::FULL),
            ..container::Style::default()
        });
    let recent = recent_chips(state);
    let since = format!("TRACKING · SINCE {}", time_of_day(running.model.started_at));
    let label = row![dot, tracked_text(LABEL_CAPS, &since, tertiary)]
        .spacing(spacing::S100)
        .align_y(Vertical::Center);
    column![
        label,
        Space::new().height(spacing::S150),
        tracked_text(DISPLAY_KEY, &running.ticket.ticket_key, accent),
        Space::new().height(spacing::S075),
        TITLE.text(&running.ticket.title).style(primary),
    ]
    .push(recent.map(|recent| {
        column![
            CAPTION.text("switch to").style(tertiary),
            Space::new().height(spacing::S075),
            recent,
        ]
        .padding(padding::top(spacing::S200))
    }))
    .extend([
        space::vertical().into(),
        tracked_text(DISPLAY_TIMER, &clock(state.elapsed()), primary),
        Space::new().height(spacing::S100).into(),
        BODY.text(format!("today {}", format_duration(state.today_total())))
            .style(tertiary)
            .into(),
    ])
    .into()
}

fn not_tracking<'a>(state: &'a tracking::State, tokens: &'static Tokens) -> Element<'a, Message> {
    let last = state.last();
    let recent = recent_chips(state);
    let last_line = last.map(|last| {
        let stopped = last.model.stopped_at.map(time_of_day).unwrap_or_default();
        column![
            Space::new().height(spacing::S150),
            rich_text![
                span::<(), _>("last: "),
                span(&last.ticket.ticket_key)
                    .color(tokens.text_accent)
                    .font(MONO_SEMIBOLD),
                span(format!(" {}", last.ticket.title)),
                span(format!(" · stopped at {stopped}")).color(tokens.text_tertiary),
            ]
            .font(BODY_LG.font)
            .size(BODY_LG.size)
            .line_height(BODY_LG.line_height)
            .style(secondary),
        ]
    });
    let hint = if last.is_some() {
        RESUME_HINT
    } else {
        NOTHING_TODAY_HINT
    };
    column![
        tracked_text(LABEL_CAPS, "○ NOT TRACKING", tertiary),
        space::vertical(),
        tracked_text(DISPLAY_TIMER, &clock(0), disabled),
    ]
    .push(last_line)
    .push(Space::new().height(spacing::S075))
    .push(container(PARAGRAPH.text(hint).style(secondary)).max_width(HINT_MAX_WIDTH))
    .push(recent.map(|recent| container(recent).padding(padding::top(spacing::S200))))
    .into()
}

/// The recent tickets as chips that wrap onto new lines and start the ticket
/// when clicked, or `None` when there are none.
fn recent_chips(state: &tracking::State) -> Option<Element<'_, Message>> {
    let recent = state.recent();
    if recent.is_empty() {
        return None;
    }
    let chips = Row::with_children(recent.into_iter().map(|recent| {
        recent_chip(
            recent,
            Message::Tracking(tracking::Message::Start(recent.clone())),
        )
    }))
    .spacing(spacing::S075)
    .wrap()
    .vertical_spacing(spacing::S075);
    Some(chips.into())
}

/// Formats `timestamp` as local `HH:MM`.
pub(super) fn time_of_day(timestamp: i64) -> String {
    DateTime::from_timestamp(timestamp, 0)
        .map(|utc| utc.with_timezone(&Local).format("%H:%M").to_string())
        .unwrap_or_default()
}

fn primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_primary),
    }
}

fn secondary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_secondary),
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}

fn disabled(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_disabled),
    }
}

fn accent(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_accent),
    }
}
