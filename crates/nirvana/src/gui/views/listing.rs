use chrono::{Days, NaiveDate};
use iced::alignment::{Horizontal, Vertical};
use iced::widget::{Column, Container, column, container, row, rule, space, text};
use iced::{Element, Length, Padding, Theme};
use nirvana_core::{PublishState, Slot, UnpublishedSummary};

use super::{MAIN_WINDOW_SIZE, border};
use crate::gui::app::Message;
use crate::gui::components::{
    GroupSlot, Mark, ellipsized_text, ghost_button, link, pane_label, scroll_area, segmented,
    slot_row, ticket_group,
};
use crate::gui::design_system::typography::{BODY, BODY_LG, BODY_STRONG, CAPTION, LABEL_STRONG};
use crate::gui::design_system::{space as spacing, tokens};
use crate::gui::state::listing::{self, Day, Scope, View, group_by_ticket};
use crate::gui::state::{settings, tracking};
use crate::time::{format_duration, local_date, time_of_day};

const MIN_WIDTH: f32 = 400.0;
const WINDOW_SHARE: f32 = MIN_WIDTH / MAIN_WINDOW_SIZE.width;
const TODAY: &str = "TODAY";
const UNPUBLISHED: &str = "unpublished";
const EXPAND: &str = "«";
const COLLAPSE: &str = "»";
const NAMED_DAYS_MAX: usize = 5;
const NAMED_DAYS_BACK: Days = Days::new(6);

/// The expand control, the scope control, the total of the shown scope,
/// either the running slot and today's stopped slots, or the unpublished
/// slots by day, newest first, and the unpublished days and total with a link
/// to nirvana web.
pub(super) fn listing<'a>(
    state: &'a listing::State,
    tracking_state: &'a tracking::State,
    web_available: bool,
) -> Column<'a, Message> {
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
            unpublished_list(
                &days,
                state.view(),
                state.today(),
                running_day,
                tracking_state,
            ),
        ),
    };
    let total = LABEL_STRONG.text(format_duration(total)).style(primary);
    let scope = segmented(
        [
            (Scope::Today, "today".to_owned()),
            (
                Scope::Unpublished,
                unpublished_option(
                    state
                        .unpublished()
                        .map_or(0, |unpublished| unpublished.slots),
                ),
            ),
        ],
        state.scope(),
        |scope| Message::Listing(listing::Message::SelectScope(scope)),
    );
    let view = segmented(
        [
            (View::ByTime, "by time".to_owned()),
            (View::ByTicket, "by ticket".to_owned()),
        ],
        state.view(),
        |view| Message::Listing(listing::Message::SelectView(view)),
    );
    let controls = row![scope, space::horizontal(), view].spacing(spacing::S100);
    let expand = ghost_button(
        if state.is_expanded() {
            COLLAPSE
        } else {
            EXPAND
        },
        Message::Listing(listing::Message::ToggleExpanded),
    );
    let label = pane_label(Some(expand), &label, total);
    let header = container(column![label, controls].spacing(spacing::S100))
        .padding(Padding::from(spacing::S200).bottom(spacing::S100));
    let header_border = (state.scope() == Scope::Unpublished && !days.is_empty())
        .then(|| border(rule::horizontal(1)));
    Column::new()
        .push(header)
        .push(header_border)
        .push(container(list).height(Length::Fill))
        .push(footer(state.unpublished(), state.today(), web_available))
}

/// The days and total of the unpublished stopped slots, or `all published`,
/// once they are loaded, and a link to nirvana web while it is available.
fn footer<'a>(
    unpublished: Option<&UnpublishedSummary>,
    today: Option<NaiveDate>,
    web_available: bool,
) -> Element<'a, Message> {
    let summary = unpublished.map(|unpublished| {
        let total: Element<'a, Message> = if unpublished.days.is_empty() {
            BODY.text("all published").style(tertiary).into()
        } else {
            BODY.text(format_duration(unpublished.seconds))
                .style(primary)
                .into()
        };
        row![
            ellipsized_text(BODY, footer_days(&unpublished.days, today), secondary),
            total
        ]
        .spacing(spacing::S100)
        .align_y(Vertical::Center)
    });
    let web = row![
        BODY.text("edit logs and see your week in").style(tertiary),
        link(
            BODY.text(if web_available {
                "nirvana web ↗"
            } else {
                "nirvana web"
            }),
            web_available.then_some(Message::Settings(settings::Message::OpenWeb)),
        ),
    ]
    .spacing(spacing::S075);
    column![
        border(rule::horizontal(1)),
        container(Column::new().push(summary).push(web).spacing(spacing::S100))
            .padding([spacing::S150, spacing::S200])
    ]
    .into()
}

/// The running slot and today's stopped slots, or the empty state.
fn today_list<'a>(
    state: &'a listing::State,
    tracking_state: &'a tracking::State,
) -> Element<'a, Message> {
    let slots: Vec<_> = state.slots().iter().collect();
    let running = tracking_state.running();
    if running.is_none() && slots.is_empty() {
        return empty(
            "nothing tracked yet today",
            "logs appear here as you start and stop tickets.",
        );
    }
    scroll_area(
        listing::LIST,
        rows(state.view(), &slots, running, tracking_state)
            .padding(Padding::from([spacing::S0, spacing::S150]).bottom(spacing::S100)),
        Length::Fill,
    )
}

/// A section per day with its header, with the running slot in the section
/// of its day.
fn unpublished_list<'a>(
    days: &[Day<&'a (Slot, PublishState)>],
    view: View,
    today: Option<NaiveDate>,
    running_day: Option<NaiveDate>,
    tracking_state: &'a tracking::State,
) -> Element<'a, Message> {
    if days.is_empty() {
        return empty(
            "everything is published",
            "slots stay here until you publish them.",
        );
    }
    let sections = days.iter().enumerate().map(|(index, day)| {
        let running = (Some(day.date) == running_day)
            .then(|| tracking_state.running())
            .flatten();
        let rows = rows(view, &day.items, running, tracking_state).padding(
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
    scroll_area(listing::LIST, Column::with_children(sections), Length::Fill)
}

/// The `running` slot and the stopped `slots`, newest first, by time, or
/// grouped by ticket.
fn rows<'a>(
    view: View,
    slots: &[&'a (Slot, PublishState)],
    running: Option<&'a Slot>,
    tracking_state: &tracking::State,
) -> Column<'a, Message> {
    let elapsed = tracking_state.elapsed();
    match view {
        View::ByTime => Column::new()
            .push(running.map(|running| {
                let row = running_slot(running, elapsed);
                slot_row(
                    row.mark,
                    row.range,
                    &running.ticket.ticket_key,
                    &running.ticket.title,
                    row.duration,
                )
            }))
            .extend(slots.iter().map(|(slot, publish_state)| {
                let row = stopped_slot(slot, *publish_state);
                slot_row(
                    row.mark,
                    row.range,
                    &slot.ticket.ticket_key,
                    &slot.ticket.title,
                    row.duration,
                )
            })),
        View::ByTicket => {
            Column::with_children(ticket_groups(slots, running, elapsed)).spacing(spacing::S075)
        }
    }
}

/// A group per ticket of the `running` slot and the stopped `slots`, with
/// the running ticket first.
fn ticket_groups<'a>(
    slots: &[&'a (Slot, PublishState)],
    running: Option<&'a Slot>,
    elapsed: i64,
) -> impl Iterator<Item = Element<'a, Message>> {
    let seconds_of = |(slot, _): &&(Slot, PublishState)| slot.elapsed();
    group_by_ticket(
        slots.to_vec(),
        running.map(|running| running.ticket.id),
        |(slot, _)| slot.ticket.id,
        seconds_of,
    )
    .into_iter()
    .filter_map(move |group| {
        let running = running.filter(|running| running.ticket.id == group.id);
        let ticket = running
            .map(|running| &running.ticket)
            .or_else(|| group.items.first().map(|(slot, _)| &slot.ticket))?;
        let stopped: i64 = group.items.iter().map(seconds_of).sum();
        let total = stopped + running.map_or(0, |_| elapsed);
        let running = running.map(|running| running_slot(running, elapsed));
        let stopped = group
            .items
            .iter()
            .map(|(slot, publish_state)| stopped_slot(slot, *publish_state));
        Some(ticket_group(
            &ticket.ticket_key,
            &ticket.title,
            format_duration(total),
            running.into_iter().chain(stopped).collect(),
        ))
    })
}

fn running_slot(running: &Slot, elapsed: i64) -> GroupSlot {
    GroupSlot {
        mark: Mark::Running,
        range: (time_of_day(running.model.started_at), "now".to_owned()),
        duration: format_duration(elapsed),
    }
}

fn stopped_slot(slot: &Slot, publish_state: PublishState) -> GroupSlot {
    GroupSlot {
        mark: Mark::Stopped(publish_state),
        range: (
            time_of_day(slot.model.started_at),
            slot.model.stopped_at.map(time_of_day).unwrap_or_default(),
        ),
        duration: format_duration(slot.elapsed()),
    }
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
    row![
        border(rule::vertical(1)),
        pane(content, Length::Fixed(width))
    ]
    .into()
}

pub(super) fn full_pane<'a>(content: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    pane(content, Length::Fill).into()
}

fn pane<'a>(content: impl Into<Element<'a, Message>>, width: Length) -> Container<'a, Message> {
    container(content)
        .width(width)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_subtle))
}

/// Formats `days` as e.g. `UNPUBLISHED · 3 DAYS`.
fn unpublished_label(days: usize) -> String {
    match days {
        0 => UNPUBLISHED.to_uppercase(),
        1 => format!("{} · 1 DAY", UNPUBLISHED.to_uppercase()),
        days => format!("{} · {days} DAYS", UNPUBLISHED.to_uppercase()),
    }
}

/// Formats `days` by name when there are at most five in the week up to
/// `today`, e.g. `unpublished · Wed, Thu`, and as a count otherwise, e.g.
/// `unpublished · 10 days`.
fn footer_days(days: &[NaiveDate], today: Option<NaiveDate>) -> String {
    let in_last_week = |day: &NaiveDate| {
        today.is_some_and(|today| (today - NAMED_DAYS_BACK..=today).contains(day))
    };
    match days.len() {
        0 => UNPUBLISHED.to_owned(),
        count if count <= NAMED_DAYS_MAX && days.iter().all(in_last_week) => {
            let names: Vec<_> = days
                .iter()
                .map(|day| day.format("%a").to_string())
                .collect();
            format!("{UNPUBLISHED} · {}", names.join(", "))
        }
        1 => format!("{UNPUBLISHED} · 1 day"),
        count => format!("{UNPUBLISHED} · {count} days"),
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

/// A centered `title` with a `detail` line below it.
fn empty<'a>(title: &'a str, detail: &'a str) -> Element<'a, Message> {
    let content = column![
        BODY_LG.text(title).style(secondary),
        BODY.text(detail)
            .style(tertiary)
            .align_x(Horizontal::Center),
    ]
    .spacing(spacing::S050)
    .align_x(Horizontal::Center);
    container(content)
        .center_x(Length::Fill)
        .padding([spacing::S400, spacing::S200])
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
    fn footer_days_names_up_to_five_days_of_the_last_week() {
        let day = |day| NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
        let monday = Some(day(25));
        assert_eq!(footer_days(&[], monday), "unpublished");
        assert_eq!(
            footer_days(&[day(19), day(22), day(25)], monday),
            "unpublished · Tue, Fri, Mon"
        );
    }

    #[test]
    fn footer_days_counts_more_than_five_days() {
        let day = |day| NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
        let days = [day(19), day(20), day(21), day(22), day(23), day(24)];
        assert_eq!(footer_days(&days, Some(day(25))), "unpublished · 6 days");
    }

    #[test]
    fn footer_days_counts_days_before_the_last_week() {
        let day = |day| NaiveDate::from_ymd_opt(2026, 5, day).unwrap();
        assert_eq!(
            footer_days(&[day(18)], Some(day(25))),
            "unpublished · 1 day"
        );
        assert_eq!(
            footer_days(&[day(18), day(22)], Some(day(25))),
            "unpublished · 2 days"
        );
    }

    #[test]
    fn footer_days_counts_days_before_today_is_known() {
        let day = NaiveDate::from_ymd_opt(2026, 5, 22).unwrap();
        assert_eq!(footer_days(&[day], None), "unpublished · 1 day");
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
