use std::cmp::Reverse;
use std::sync::Arc;

use chrono::{Local, NaiveDate};
use iced::Task;
use iced::widget::operation::RelativeOffset;
use iced::widget::{Id, operation};
use nirvana_core::{Nirvana, NirvanaError, PublishState, Slot, UnpublishedSummary};
use tracing::error;

use crate::gui::failure::{Failure, chain};
use crate::time::{day_start, local_date};

const LOAD_FAILED: &str = "couldn't load slots";
pub(in crate::gui) const LIST: Id = Id::new("listing");

/// The shown scope and view, the stopped slots of today, the summary of the
/// unpublished stopped slots, and those slots by day while their scope is
/// shown, with their publish states. The unpublished scope is shown once its
/// slots are loaded.
#[derive(Default)]
pub(in crate::gui) struct State {
    scope: Scope,
    view: View,
    opening_unpublished: bool,
    today: Option<Today>,
    unpublished: Option<Vec<Day<(Slot, PublishState)>>>,
}

/// The slots the listing shows.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::gui) enum Scope {
    #[default]
    Today,
    Unpublished,
}

/// How the listing orders the slots.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::gui) enum View {
    #[default]
    ByTime,
    ByTicket,
}

#[derive(Clone, Debug)]
pub(in crate::gui) struct Today {
    date: NaiveDate,
    slots: Vec<(Slot, PublishState)>,
    unpublished: UnpublishedSummary,
}

/// The items of a day, newest first.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::gui) struct Day<T> {
    pub(in crate::gui) date: NaiveDate,
    pub(in crate::gui) items: Vec<T>,
}

/// The items of a ticket, newest first.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::gui) struct Ticket<T> {
    pub(in crate::gui) id: i64,
    pub(in crate::gui) items: Vec<T>,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Load,
    TodayLoaded(Today),
    UnpublishedLoaded(Vec<Day<(Slot, PublishState)>>),
    Failed(Failure),
    SelectScope(Scope),
    SelectView(View),
}

impl State {
    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        let nirvana = match nirvana {
            Some(Ok(nirvana)) => Some(nirvana.clone()),
            _ => None,
        };
        match message {
            Message::Load => {
                if let Some(nirvana) = nirvana {
                    let unpublished = (self.scope == Scope::Unpublished)
                        .then(|| load_unpublished(nirvana.clone()));
                    return Task::batch([load_today(nirvana)].into_iter().chain(unpublished));
                }
            }
            Message::TodayLoaded(today) => self.today = Some(today),
            Message::UnpublishedLoaded(days) => {
                if self.scope == Scope::Unpublished {
                    self.unpublished = Some(days);
                } else if self.opening_unpublished {
                    self.opening_unpublished = false;
                    self.scope = Scope::Unpublished;
                    self.unpublished = Some(days);
                    return operation::snap_to(LIST, RelativeOffset::START);
                }
            }
            Message::Failed(_) => {}
            Message::SelectScope(Scope::Today) => {
                self.opening_unpublished = false;
                if self.scope != Scope::Today {
                    self.scope = Scope::Today;
                    self.unpublished = None;
                    return operation::snap_to(LIST, RelativeOffset::START);
                }
            }
            Message::SelectScope(Scope::Unpublished) => {
                if self.scope != Scope::Unpublished
                    && let Some(nirvana) = nirvana
                {
                    self.opening_unpublished = true;
                    return load_unpublished(nirvana);
                }
            }
            Message::SelectView(view) if view != self.view => {
                self.view = view;
                return operation::snap_to(LIST, RelativeOffset::START);
            }
            Message::SelectView(_) => {}
        }
        Task::none()
    }

    pub(in crate::gui) fn scope(&self) -> Scope {
        self.scope
    }

    pub(in crate::gui) fn view(&self) -> View {
        self.view
    }

    pub(in crate::gui) fn today(&self) -> Option<NaiveDate> {
        self.today.as_ref().map(|today| today.date)
    }

    /// Returns the number, the summed seconds and the days of the
    /// unpublished stopped slots, once they are loaded.
    pub(in crate::gui) fn unpublished(&self) -> Option<&UnpublishedSummary> {
        self.today.as_ref().map(|today| &today.unpublished)
    }

    /// Returns the stopped slots of today, newest first.
    pub(in crate::gui) fn slots(&self) -> &[(Slot, PublishState)] {
        self.today.as_ref().map_or(&[], |today| &today.slots)
    }

    /// Returns the days with unpublished stopped slots, and the day the
    /// running slot started on, newest first. Empty unless the unpublished
    /// scope is shown.
    pub(in crate::gui) fn unpublished_days(
        &self,
        running: Option<NaiveDate>,
    ) -> Vec<Day<&(Slot, PublishState)>> {
        let days = self.unpublished.as_deref().unwrap_or_default();
        with_running_day(days, running)
    }
}

fn load_today(nirvana: Arc<Nirvana>) -> Task<Message> {
    let date = Local::now().date_naive();
    let from = match day_start(date) {
        Ok(from) => from,
        Err(e) => {
            error!("{e:#}");
            return Task::done(Message::Failed(Failure::new(LOAD_FAILED, None)));
        }
    };
    Task::future(async move {
        let listed = nirvana.list_slots(from, None, false).await?;
        let unpublished = nirvana.unpublished_summary().await?;
        Ok::<_, NirvanaError>((listed, unpublished))
    })
    .map(move |loaded| match loaded {
        Ok((listed, unpublished)) => Message::TodayLoaded(Today {
            date,
            slots: stopped_newest_first(listed),
            unpublished,
        }),
        Err(e) => {
            error!("{}", chain(&e));
            Message::Failed(Failure::from_error(LOAD_FAILED, &e))
        }
    })
}

fn load_unpublished(nirvana: Arc<Nirvana>) -> Task<Message> {
    Task::future(async move { nirvana.list_unpublished_slots().await }).map(|listed| match listed {
        Ok(listed) => match group_by_day(listed, |(slot, _)| local_date(slot.model.started_at)) {
            Ok(days) => Message::UnpublishedLoaded(days),
            Err(e) => {
                error!("{e:#}");
                Message::Failed(Failure::new(LOAD_FAILED, None))
            }
        },
        Err(e) => {
            error!("{}", chain(&e));
            Message::Failed(Failure::from_error(LOAD_FAILED, &e))
        }
    })
}

/// Keeps the stopped slots of `listed`, which is ordered by start, oldest
/// first, and returns them newest first.
fn stopped_newest_first(listed: Vec<(Slot, PublishState)>) -> Vec<(Slot, PublishState)> {
    listed
        .into_iter()
        .rev()
        .filter(|(slot, _)| slot.model.stopped_at.is_some())
        .collect()
}

/// Groups `items`, which are ordered by start, oldest first, by the day
/// `date_of` returns, newest day and item first.
fn group_by_day<T>(
    items: Vec<T>,
    date_of: impl Fn(&T) -> anyhow::Result<NaiveDate>,
) -> anyhow::Result<Vec<Day<T>>> {
    let mut days: Vec<Day<T>> = Vec::new();
    for item in items.into_iter().rev() {
        let date = date_of(&item)?;
        match days.last_mut() {
            Some(day) if day.date == date => day.items.push(item),
            _ => days.push(Day {
                date,
                items: vec![item],
            }),
        }
    }
    Ok(days)
}

/// Groups `items`, which are newest first, by the ticket `ticket_of`
/// returns. The ticket `first` comes first, also without items, then the
/// others by the summed `seconds_of` of their items, most first, and newest
/// first when equal.
pub(in crate::gui) fn group_by_ticket<T>(
    items: Vec<T>,
    first: Option<i64>,
    ticket_of: impl Fn(&T) -> i64,
    seconds_of: impl Fn(&T) -> i64,
) -> Vec<Ticket<T>> {
    let mut tickets: Vec<Ticket<T>> = first
        .map(|id| Ticket {
            id,
            items: Vec::new(),
        })
        .into_iter()
        .collect();
    for item in items {
        let id = ticket_of(&item);
        match tickets.iter_mut().find(|ticket| ticket.id == id) {
            Some(ticket) => ticket.items.push(item),
            None => tickets.push(Ticket {
                id,
                items: vec![item],
            }),
        }
    }
    tickets.sort_by_key(|ticket| {
        (
            Some(ticket.id) != first,
            Reverse(ticket.items.iter().map(&seconds_of).sum::<i64>()),
        )
    });
    tickets
}

/// Returns `days`, which are newest first, with an empty day for `running`
/// when it is not one of them.
fn with_running_day<T>(days: &[Day<T>], running: Option<NaiveDate>) -> Vec<Day<&T>> {
    let mut shown: Vec<Day<&T>> = days
        .iter()
        .map(|day| Day {
            date: day.date,
            items: day.items.iter().collect(),
        })
        .collect();
    if let Some(running) = running
        && !days.iter().any(|day| day.date == running)
    {
        let index = shown.partition_point(|day| day.date > running);
        shown.insert(
            index,
            Day {
                date: running,
                items: Vec::new(),
            },
        );
    }
    shown
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, day).unwrap()
    }

    fn day<T>(day: u32, items: Vec<T>) -> Day<T> {
        Day {
            date: date(day),
            items,
        }
    }

    #[test]
    fn group_by_day_returns_newest_day_and_item_first() {
        let items = vec![(18, 1), (18, 2), (20, 3), (22, 4), (22, 5)];

        let days = group_by_day(items, |(day, _)| Ok(date(*day))).unwrap();

        assert_eq!(
            days,
            [
                day(22, vec![(22, 5), (22, 4)]),
                day(20, vec![(20, 3)]),
                day(18, vec![(18, 2), (18, 1)]),
            ]
        );
    }

    #[test]
    fn group_by_day_fails_when_a_date_fails() {
        let result = group_by_day(vec![1], |_| anyhow::bail!("out of range"));
        assert!(result.is_err());
    }

    fn ticket(id: i64, items: Vec<(i64, i64)>) -> Ticket<(i64, i64)> {
        Ticket { id, items }
    }

    fn group(items: Vec<(i64, i64)>, first: Option<i64>) -> Vec<Ticket<(i64, i64)>> {
        group_by_ticket(items, first, |(id, _)| *id, |(_, seconds)| *seconds)
    }

    #[test]
    fn group_by_ticket_orders_by_seconds_and_keeps_items_newest_first() {
        let items = vec![(1, 10), (2, 30), (1, 15), (3, 5)];

        assert_eq!(
            group(items, None),
            [
                ticket(2, vec![(2, 30)]),
                ticket(1, vec![(1, 10), (1, 15)]),
                ticket(3, vec![(3, 5)]),
            ]
        );
    }

    #[test]
    fn group_by_ticket_keeps_newest_first_when_seconds_are_equal() {
        let items = vec![(1, 10), (2, 10)];

        assert_eq!(
            group(items, None),
            [ticket(1, vec![(1, 10)]), ticket(2, vec![(2, 10)])]
        );
    }

    #[test]
    fn group_by_ticket_puts_the_first_ticket_first() {
        let items = vec![(1, 30), (2, 5)];

        assert_eq!(
            group(items, Some(2)),
            [ticket(2, vec![(2, 5)]), ticket(1, vec![(1, 30)])]
        );
    }

    #[test]
    fn group_by_ticket_adds_the_first_ticket_without_items() {
        assert_eq!(
            group(vec![(1, 30)], Some(2)),
            [ticket(2, vec![]), ticket(1, vec![(1, 30)])]
        );
    }

    fn opening_unpublished() -> State {
        State {
            opening_unpublished: true,
            ..State::default()
        }
    }

    #[test]
    fn loaded_unpublished_slots_show_their_scope() {
        let mut state = opening_unpublished();

        let _ = state.update(Message::UnpublishedLoaded(Vec::new()), None);

        assert_eq!(state.scope(), Scope::Unpublished);
        assert!(state.unpublished.is_some());
    }

    #[test]
    fn selecting_today_while_opening_unpublished_drops_the_loaded_slots() {
        let mut state = opening_unpublished();
        let _ = state.update(Message::SelectScope(Scope::Today), None);

        let _ = state.update(Message::UnpublishedLoaded(vec![day(22, Vec::new())]), None);

        assert_eq!(state.scope(), Scope::Today);
        assert!(state.unpublished.is_none());
    }

    #[test]
    fn unpublished_slots_loaded_without_opening_are_dropped() {
        let mut state = State::default();

        let _ = state.update(Message::UnpublishedLoaded(vec![day(22, Vec::new())]), None);

        assert_eq!(state.scope(), Scope::Today);
        assert!(state.unpublished.is_none());
    }

    #[test]
    fn select_view_keeps_the_scope() {
        let mut state = opening_unpublished();
        let _ = state.update(Message::UnpublishedLoaded(Vec::new()), None);

        let _ = state.update(Message::SelectView(View::ByTicket), None);

        assert_eq!(state.view(), View::ByTicket);
        assert_eq!(state.scope(), Scope::Unpublished);
    }

    #[test]
    fn with_running_day_inserts_the_running_day_in_order() {
        let days = [day(22, vec![1]), day(18, vec![2])];

        let shown = with_running_day(&days, Some(date(20)));

        let dates: Vec<_> = shown.iter().map(|day| day.date).collect();
        assert_eq!(dates, [date(22), date(20), date(18)]);
        assert!(shown[1].items.is_empty());
    }

    #[test]
    fn with_running_day_keeps_a_day_that_is_shown() {
        let days = [day(22, vec![1])];

        let shown = with_running_day(&days, Some(date(22)));

        assert_eq!(shown, [day(22, vec![&1])]);
    }

    #[test]
    fn with_running_day_keeps_days_without_running_slot() {
        let days = [day(22, vec![1])];
        assert_eq!(with_running_day(&days, None), [day(22, vec![&1])]);
    }
}
