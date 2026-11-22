use chrono::{DateTime, Local};
use iced::alignment::Vertical;
use iced::widget::{Space, column, container, rich_text, row, space, span, text};
use iced::{Border, Element, Length, Theme};
use nirvana_core::Slot;

use crate::gui::app::Message;
use crate::gui::components::tracked_text;
use crate::gui::design_system::font::MONO_SEMIBOLD;
use crate::gui::design_system::typography::{
    BODY_LG, DISPLAY_KEY, DISPLAY_TIMER, LABEL_CAPS, PARAGRAPH, TITLE,
};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::tracking::{self, clock};

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
        Some(running) => tracking_now(running, state.elapsed(), state.pulse_opacity()),
        None => not_tracking(state.last(), tokens),
    };
    container(content)
        .padding(spacing::S300)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn tracking_now(running: &Slot, elapsed: i64, pulse: f32) -> Element<'_, Message> {
    let dot = container(space())
        .width(PULSE_DOT_SIZE)
        .height(PULSE_DOT_SIZE)
        .style(move |theme| container::Style {
            background: Some(tokens(theme).accent.scale_alpha(pulse).into()),
            border: Border::default().rounded(radius::FULL),
            ..container::Style::default()
        });
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
        space::vertical(),
        tracked_text(DISPLAY_TIMER, &clock(elapsed), primary),
    ]
    .into()
}

fn not_tracking<'a>(last: Option<&'a Slot>, tokens: &'static Tokens) -> Element<'a, Message> {
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
    .into()
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
