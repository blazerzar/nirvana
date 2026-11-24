use chrono::NaiveDate;
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, column, container, row, rule, space, text};
use iced::{Element, Length, Padding, Theme};
use nirvana_core::{PublishState, Slot};

use super::{MAIN_WINDOW_SIZE, border};
use crate::gui::app::Message;
use crate::gui::components::{Mark, pane_label, scroll_area, segmented, slot_row};
use crate::gui::design_system::typography::{BODY, BODY_LG, BODY_STRONG, CAPTION, LABEL_STRONG};
use crate::gui::design_system::{space as spacing, tokens};
use crate::gui::state::listing::{self, Day, Scope};
use crate::gui::state::tracking;
use crate::time::{format_duration, local_date, time_of_day};

const MIN_WIDTH: f32 = 400.0;
const WINDOW_SHARE: f32 = MIN_WIDTH / MAIN_WINDOW_SIZE.width;
const TODAY: &str = "TODAY";
const UNPUBLISHED: &str = "unpublished";

/// The scope control, the total of the shown scope, and either the running
/// slot and today's stopped slots, or the unpublished slots by day, newest
/// first.
pub(super) fn listing<'a>(
    state: &'a listing::State,
    tracking_state: &'a tracking::State,
    available_width: f32,
) -> Element<'a, Message> {
    let running_day = tracking_state
        .running()
        .and_then(|running| local_date(running.model.started_at).ok());
    let days = state.unpublished_days(running_day);
    let (label, total, list) = match state.scope() {
        Scope::Today => (
            state.today().map_or_else(|| TODAY.to_owned(), today_label),
            tracking_state.today_total(),
            today_list(state, tracking_state),
        ),
        Scope::Unpublished => (
            unpublished_label(days.len()),
            days.iter()
                .map(|day| day_total(day, running_day, tracking_state))
                .sum(),
            unpublished_list(&days, state.today(), running_day, tracking_state),
        ),
    };
    let total = LABEL_STRONG.text(format_duration(total)).style(primary);
    let scope = segmented(
        [
            (Scope::Today, "today".to_owned()),
            (
                Scope::Unpublished,
                unpublished_option(state.unpublished_count()),
            ),
        ],
        state.scope(),
        |scope| Message::Listing(listing::Message::SelectScope(scope)),
    );
    let header = container(column![pane_label(&label, total), scope].spacing(spacing::S100))
        .padding(Padding::from(spacing::S200).bottom(spacing::S100));
    let header_border = (state.scope() == Scope::Unpublished).then(|| border(rule::horizontal(1)));
    side_pane(
        Column::new().push(header).push(header_border).push(list),
        available_width,
    )
}

/// The running slot and today's stopped slots, or the empty state.
fn today_list<'a>(
    state: &'a listing::State,
    tracking_state: &'a tracking::State,
) -> Element<'a, Message> {
    let slots = state.slots();
    if tracking_state.running().is_none() && slots.is_empty() {
        return empty();
    }
    scroll_area(
        listing::LIST,
        Column::new()
            .push(running_row(tracking_state))
            .extend(slots.iter().map(stopped_row))
            .padding(Padding::from([spacing::S0, spacing::S150]).bottom(spacing::S100)),
    )
}

/// A section per day with its header, with the running slot in the section
/// of its day.
fn unpublished_list<'a>(
    days: &[Day<&'a (Slot, PublishState)>],
    today: Option<NaiveDate>,
    running_day: Option<NaiveDate>,
    tracking_state: &'a tracking::State,
) -> Element<'a, Message> {
    if days.is_empty() {
        return container(BODY.text("everything is published.").style(tertiary))
            .padding([spacing::S100, spacing::S200])
            .into();
    }
    let sections = days.iter().enumerate().map(|(index, day)| {
        let running = (Some(day.date) == running_day)
            .then(|| running_row(tracking_state))
            .flatten();
        let rows = Column::new()
            .push(running)
            .extend(day.items.iter().map(|slot| stopped_row(slot)))
            .padding(
                Padding::from([spacing::S0, spacing::S150])
                    .top(spacing::S050)
                    .bottom(spacing::S075),
            );
        let total = day_total(day, running_day, tracking_state);
        let is_first = index == 0;
        column![
            day_header(day.date, Some(day.date) == today, total, !is_first),
            rows
        ]
        .into()
    });
    scroll_area(listing::LIST, Column::with_children(sections))
}

fn running_row(tracking_state: &tracking::State) -> Option<Element<'_, Message>> {
    tracking_state.running().map(|running| {
        slot_row(
            Mark::Running,
            (time_of_day(running.model.started_at), "now".to_owned()),
            &running.ticket.ticket_key,
            &running.ticket.title,
            format_duration(tracking_state.elapsed()),
        )
    })
}

fn stopped_row((slot, publish_state): &(Slot, PublishState)) -> Element<'_, Message> {
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
}

/// The date of a day, `today` when it is today, and its total, above a
/// border, and below one with `top_border`.
fn day_header<'a>(
    date: NaiveDate,
    is_today: bool,
    total: i64,
    top_border: bool,
) -> Element<'a, Message> {
    let content = row![BODY_STRONG.text(day_label(date)).style(primary)]
        .push(is_today.then(|| CAPTION.text("today").style(accent)))
        .push(space::horizontal())
        .push(BODY.text(format_duration(total)).style(secondary))
        .spacing(spacing::S100)
        .align_y(Vertical::Center);
    let header = container(content)
        .width(Length::Fill)
        .padding([spacing::S075, spacing::S200])
        .style(|theme| container::background(tokens(theme).bg_muted));
    Column::new()
        .push(top_border.then(|| border(rule::horizontal(1))))
        .push(header)
        .push(border(rule::horizontal(1)))
        .into()
}

/// Returns the seconds of the stopped slots of `day`, with the running slot
/// when it started on that day.
fn day_total(
    day: &Day<&(Slot, PublishState)>,
    running_day: Option<NaiveDate>,
    tracking_state: &tracking::State,
) -> i64 {
    let stopped: i64 = day.items.iter().map(|(slot, _)| slot.elapsed()).sum();
    let running = if Some(day.date) == running_day {
        tracking_state.elapsed()
    } else {
        0
    };
    stopped + running
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

/// Formats `days` as e.g. `UNPUBLISHED · 3 DAYS`.
fn unpublished_label(days: usize) -> String {
    match days {
        0 => UNPUBLISHED.to_uppercase(),
        1 => format!("{} · 1 DAY", UNPUBLISHED.to_uppercase()),
        days => format!("{} · {days} DAYS", UNPUBLISHED.to_uppercase()),
    }
}

/// Formats the unpublished scope option as e.g. `unpublished · 3`.
fn unpublished_option(slots: u64) -> String {
    match slots {
        0 => UNPUBLISHED.to_owned(),
        slots => format!("{UNPUBLISHED} · {slots}"),
    }
}

/// Formats `date` as e.g. `Fri 22 May`.
fn day_label(date: NaiveDate) -> String {
    date.format("%a %-d %b").to_string()
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
    .spacing(spacing::S050)
    .align_x(Horizontal::Center);
    container(content)
        .center_x(Length::Fill)
        .padding([spacing::S400, spacing::S100])
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

fn accent(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_accent),
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
    fn unpublished_label_counts_days() {
        assert_eq!(unpublished_label(0), "UNPUBLISHED");
        assert_eq!(unpublished_label(1), "UNPUBLISHED · 1 DAY");
        assert_eq!(unpublished_label(3), "UNPUBLISHED · 3 DAYS");
    }

    #[test]
    fn unpublished_option_counts_slots() {
        assert_eq!(unpublished_option(0), "unpublished");
        assert_eq!(unpublished_option(3), "unpublished · 3");
    }

    #[test]
    fn day_label_shows_weekday_day_and_month() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
        assert_eq!(day_label(date), "Fri 22 May");
    }

    #[test]
    fn today_label_is_uppercase_with_the_date() {
        let date = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
        assert_eq!(today_label(date), "TODAY · FRI MAY 22");
    }
}
