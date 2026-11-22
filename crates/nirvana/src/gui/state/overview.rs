use std::collections::BTreeMap;
use std::sync::Arc;

use chrono::{Datelike, Days, Local, Months, NaiveDate, Weekday};
use iced::Task;
use nirvana_core::{Nirvana, PublishState, Slot};
use tracing::error;

use crate::gui::failure::{Failure, chain};
use crate::time::{day_start, local_date};

const LOAD_FAILED: &str = "couldn't load the overview";
pub(in crate::gui) const EXPECTED_SECONDS_PER_DAY: i64 = 8 * 3600;
const WEEK_START: Weekday = Weekday::Mon;
const WORKDAYS_PER_WEEK: i64 = 5;

/// Today, this week and this month of the active profile.
#[derive(Default)]
pub(in crate::gui) struct State {
    loaded: Option<Loaded>,
}

/// The stopped slots by the day they started on, and the day the running
/// slot started on.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::gui) struct Loaded {
    today: NaiveDate,
    days: BTreeMap<NaiveDate, Total>,
    running: Option<NaiveDate>,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Load,
    Loaded(Loaded),
    Failed(Failure),
}

/// Seconds tracked, split by whether they are published.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::gui) struct Total {
    pub(in crate::gui) published: i64,
    pub(in crate::gui) unpublished: i64,
}

impl Total {
    pub(in crate::gui) fn total(self) -> i64 {
        self.published + self.unpublished
    }

    fn add(&mut self, state: PublishState, seconds: i64) {
        match state {
            PublishState::Published => self.published += seconds,
            PublishState::Unpublished | PublishState::Pending | PublishState::Failed => {
                self.unpublished += seconds;
            }
        }
    }
}

/// The total of a period against the seconds expected in it.
#[derive(Debug, PartialEq)]
pub(in crate::gui) struct Stat {
    pub(in crate::gui) label: &'static str,
    pub(in crate::gui) period: String,
    pub(in crate::gui) total: Total,
    pub(in crate::gui) target: i64,
}

/// A day of this week.
#[derive(Debug, PartialEq)]
pub(in crate::gui) struct Day {
    pub(in crate::gui) label: String,
    pub(in crate::gui) total: Total,
    pub(in crate::gui) is_today: bool,
}

impl State {
    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        match message {
            Message::Load => {
                if let Some(Ok(nirvana)) = nirvana {
                    return load(nirvana.clone());
                }
            }
            Message::Loaded(loaded) => self.loaded = Some(loaded),
            Message::Failed(_) => {}
        }
        Task::none()
    }

    /// Returns today, this week and this month, with the `elapsed` seconds
    /// of the running slot as unpublished.
    pub(in crate::gui) fn stats(&self, elapsed: i64) -> Option<[Stat; 3]> {
        self.loaded.as_ref().map(|loaded| loaded.stats(elapsed))
    }

    /// Returns the days of this week, with the `elapsed` seconds of the
    /// running slot as unpublished.
    pub(in crate::gui) fn week(&self, elapsed: i64) -> Option<[Day; 7]> {
        self.loaded.as_ref().map(|loaded| loaded.week(elapsed))
    }
}

impl Loaded {
    fn stats(&self, elapsed: i64) -> [Stat; 3] {
        let today = self.today;
        let week = week_start(today);
        let week_end = week + Days::new(7);
        let month = month_start(today);
        let month_end = month + Months::new(1);
        [
            Stat {
                label: "TODAY",
                period: today.format("%a %b %-d").to_string(),
                total: self.total(today, today + Days::new(1), elapsed),
                target: EXPECTED_SECONDS_PER_DAY,
            },
            Stat {
                label: "THIS WEEK",
                period: week_label(week, week_end - Days::new(1)),
                total: self.total(week, week_end, elapsed),
                target: WORKDAYS_PER_WEEK * EXPECTED_SECONDS_PER_DAY,
            },
            Stat {
                label: "THIS MONTH",
                period: today.format("%B %Y").to_string(),
                total: self.total(month, month_end, elapsed),
                target: workdays(month, month_end) * EXPECTED_SECONDS_PER_DAY,
            },
        ]
    }

    fn week(&self, elapsed: i64) -> [Day; 7] {
        let week = week_start(self.today);
        std::array::from_fn(|index| {
            let day = week + Days::new(index as u64);
            Day {
                label: day.format("%a %-d").to_string(),
                total: self.total(day, day + Days::new(1), elapsed),
                is_today: day == self.today,
            }
        })
    }

    /// Returns the total of the days from `from` to `to` (exclusive).
    fn total(&self, from: NaiveDate, to: NaiveDate, elapsed: i64) -> Total {
        let mut total = self
            .days
            .range(from..to)
            .fold(Total::default(), |sum, (_, day)| Total {
                published: sum.published + day.published,
                unpublished: sum.unpublished + day.unpublished,
            });
        if self.running.is_some_and(|day| (from..to).contains(&day)) {
            total.unpublished += elapsed;
        }
        total
    }
}

fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let today = Local::now().date_naive();
    let from = match day_start(week_start(today).min(month_start(today))) {
        Ok(from) => from,
        Err(e) => {
            error!("{e:#}");
            return Task::done(Message::Failed(Failure::new(LOAD_FAILED, None)));
        }
    };
    Task::future(async move { nirvana.list_slots(from, None, false).await }).map(move |listed| {
        match listed {
            Ok(listed) => match loaded(today, &listed) {
                Ok(loaded) => Message::Loaded(loaded),
                Err(e) => {
                    error!("{e:#}");
                    Message::Failed(Failure::new(LOAD_FAILED, None))
                }
            },
            Err(e) => {
                error!("{}", chain(&e));
                Message::Failed(Failure::from_error(LOAD_FAILED, &e))
            }
        }
    })
}

fn loaded(today: NaiveDate, listed: &[(Slot, PublishState)]) -> anyhow::Result<Loaded> {
    let mut days = BTreeMap::<NaiveDate, Total>::new();
    let mut running = None;
    for (slot, state) in listed {
        let day = local_date(slot.model.started_at)?;
        match slot.model.stopped_at {
            Some(_) => days.entry(day).or_default().add(*state, slot.elapsed()),
            None => running = Some(day),
        }
    }
    Ok(Loaded {
        today,
        days,
        running,
    })
}

fn week_start(date: NaiveDate) -> NaiveDate {
    date.week(WEEK_START).first_day()
}

fn month_start(date: NaiveDate) -> NaiveDate {
    date - Days::new(u64::from(date.day0()))
}

/// Returns the number of days from `from` to `to` (exclusive) that are not
/// on a weekend.
fn workdays(from: NaiveDate, to: NaiveDate) -> i64 {
    let days = from
        .iter_days()
        .take_while(|day| *day < to)
        .filter(|day| !matches!(day.weekday(), Weekday::Sat | Weekday::Sun))
        .count();
    i64::try_from(days).unwrap_or(i64::MAX)
}

/// Formats a week as e.g. `May 18 – 24`, or `Apr 27 – May 3` across months.
fn week_label(first: NaiveDate, last: NaiveDate) -> String {
    let last_format = if first.month() == last.month() {
        "%-d"
    } else {
        "%b %-d"
    };
    format!("{} – {}", first.format("%b %-d"), last.format(last_format))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(month: u32, day: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, month, day).unwrap()
    }

    fn total(published: i64, unpublished: i64) -> Total {
        Total {
            published,
            unpublished,
        }
    }

    /// Fri May 22, with a day before the month.
    fn loaded(running: Option<NaiveDate>) -> Loaded {
        Loaded {
            today: date(5, 22),
            days: BTreeMap::from([
                (date(4, 30), total(100, 0)),
                (date(5, 1), total(200, 10)),
                (date(5, 18), total(300, 20)),
                (date(5, 22), total(400, 30)),
            ]),
            running,
        }
    }

    #[test]
    fn total_counts_pending_and_failed_as_unpublished() {
        let mut sum = Total::default();
        sum.add(PublishState::Published, 1);
        sum.add(PublishState::Unpublished, 10);
        sum.add(PublishState::Pending, 100);
        sum.add(PublishState::Failed, 1000);
        assert_eq!(sum, total(1, 1110));
    }

    #[test]
    fn stats_sum_the_days_of_each_period() {
        let [today, week, month] = loaded(None).stats(0);
        assert_eq!(today.total, total(400, 30));
        assert_eq!(week.total, total(700, 50));
        assert_eq!(month.total, total(900, 60));
    }

    #[test]
    fn stats_add_the_running_slot_to_the_periods_of_its_day() {
        let [today, week, month] = loaded(Some(date(5, 18))).stats(5);
        assert_eq!(today.total, total(400, 30));
        assert_eq!(week.total, total(700, 55));
        assert_eq!(month.total, total(900, 65));
    }

    #[test]
    fn stats_label_the_periods() {
        let [today, week, month] = loaded(None).stats(0);
        assert_eq!(today.period, "Fri May 22");
        assert_eq!(week.period, "May 18 – 24");
        assert_eq!(month.period, "May 2026");
    }

    #[test]
    fn stats_expect_eight_hours_per_workday() {
        let [today, week, month] = loaded(None).stats(0);
        assert_eq!(today.target, 8 * 3600);
        assert_eq!(week.target, 40 * 3600);
        assert_eq!(month.target, 21 * 8 * 3600);
    }

    #[test]
    fn week_lists_the_days_from_monday() {
        let week = loaded(Some(date(5, 18))).week(5);
        assert_eq!(week[0].label, "Mon 18");
        assert_eq!(week[0].total, total(300, 25));
        assert_eq!(week[3].total, Total::default());
        assert_eq!(week[4].total, total(400, 30));
        assert!(week[4].is_today);
        assert!(!week[0].is_today);
    }

    #[test]
    fn week_starts_on_monday() {
        assert_eq!(week_start(date(5, 22)), date(5, 18));
        assert_eq!(week_start(date(5, 18)), date(5, 18));
        assert_eq!(week_start(date(5, 24)), date(5, 18));
    }

    #[test]
    fn month_starts_on_the_first() {
        assert_eq!(month_start(date(5, 22)), date(5, 1));
        assert_eq!(month_start(date(5, 1)), date(5, 1));
    }

    #[test]
    fn workdays_leave_out_weekends() {
        assert_eq!(workdays(date(5, 1), date(6, 1)), 21);
        assert_eq!(workdays(date(8, 1), date(9, 1)), 21);
    }

    #[test]
    fn week_label_names_both_months_across_months() {
        assert_eq!(week_label(date(4, 27), date(5, 3)), "Apr 27 – May 3");
    }
}
