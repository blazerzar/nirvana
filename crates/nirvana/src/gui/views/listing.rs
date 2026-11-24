use chrono::NaiveDate;
use iced::alignment::Horizontal;
use iced::widget::{Column, column, container, row, rule, text};
use iced::{Element, Length, Padding, Theme};

use super::{MAIN_WINDOW_SIZE, border};
use crate::gui::app::Message;
use crate::gui::components::{Mark, pane_label, scroll_area, slot_row};
use crate::gui::design_system::typography::{BODY, BODY_LG, LABEL_STRONG};
use crate::gui::design_system::{space, tokens};
use crate::gui::state::{listing, tracking};
use crate::time::{format_duration, time_of_day};

const MIN_WIDTH: f32 = 400.0;
const WINDOW_SHARE: f32 = MIN_WIDTH / MAIN_WINDOW_SIZE.width;
const TODAY: &str = "TODAY";

/// Today's total, the running slot and today's stopped slots, newest first.
pub(super) fn listing<'a>(
    state: &'a listing::State,
    tracking_state: &'a tracking::State,
    available_width: f32,
) -> Element<'a, Message> {
    let total = LABEL_STRONG
        .text(format_duration(tracking_state.today_total()))
        .style(primary);
    let label = state.today().map_or_else(|| TODAY.to_owned(), today_label);
    let header = container(pane_label(&label, total))
        .padding(Padding::from(space::S200).bottom(space::S100));
    let running = tracking_state.running().map(|running| {
        slot_row(
            Mark::Running,
            (time_of_day(running.model.started_at), "now".to_owned()),
            &running.ticket.ticket_key,
            &running.ticket.title,
            format_duration(tracking_state.elapsed()),
        )
    });
    let slots = state.slots();
    let list: Element<'a, Message> = if running.is_none() && slots.is_empty() {
        empty()
    } else {
        let rows = slots.iter().map(|(slot, publish_state)| {
            slot_row(
                Mark::Stopped(*publish_state),
                (
                    time_of_day(slot.model.started_at),
                    slot.model.stopped_at.map(time_of_day).unwrap_or_default(),
                ),
                &slot.ticket.ticket_key,
                &slot.ticket.title,
                format_duration(slot.elapsed()),
            )
        });
        scroll_area(
            Column::new()
                .push(running)
                .extend(rows)
                .padding(Padding::from([space::S0, space::S150]).bottom(space::S100)),
        )
    };
    side_pane(column![header, list], available_width)
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

/// Formats `today` as e.g. `TODAY · FRI MAY 22`.
fn today_label(today: NaiveDate) -> String {
    format!(
        "{TODAY} · {}",
        today.format("%a %b %-d").to_string().to_uppercase()
    )
}

fn empty<'a>() -> Element<'a, Message> {
    let content = column![
        BODY_LG.text("nothing tracked yet today").style(secondary),
        BODY.text("logs appear here as you start and stop tickets.")
            .style(tertiary),
    ]
    .spacing(space::S050)
    .align_x(Horizontal::Center);
    container(content)
        .center_x(Length::Fill)
        .padding([space::S400, space::S100])
        .into()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn today_label_is_uppercase_with_the_date() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
        assert_eq!(today_label(date), "TODAY · FRI MAY 22");
    }
}
