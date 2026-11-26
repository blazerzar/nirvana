use std::sync::Arc;

use iced::Task;
use nirvana_core::{Change, Nirvana, NirvanaError, Slot};
use tracing::{error, info};

use super::start::Recent;
use crate::gui::failure::{Failure, chain};
use crate::gui::idle::Period;
use crate::time::{format_duration, time_of_day};

const SAVE_FAILED: &str = "couldn't save the idle time";
const TRACKING_CHANGED: &str = "tracking changed while you were away";
const BEFORE_PORTION: u16 = 6;
const AWAY_PORTION: u16 = 16;
const NOW_PORTION: u16 = 7;

/// The idle period of the running slot and the answers to what the user did
/// while away and does now.
pub(in crate::gui) struct Dialog {
    period: Period,
    slot_id: i64,
    started_at: i64,
    running_key: String,
    running_connection_id: Option<i64>,
    away: Choice,
    away_pick: TicketPick,
    now: Choice,
    now_pick: TicketPick,
    saving: bool,
}

/// An answer: the running ticket, another ticket, or neither, which is a
/// break while away and stopping now.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(in crate::gui) enum Choice {
    #[default]
    Same,
    Other,
    Neither,
}

/// What a part of the period becomes once the answers are saved.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(in crate::gui) enum Segment<'a> {
    /// Time logged to the ticket.
    Tracked(&'a str),
    /// The ticket tracked from when the user is back.
    Running(&'a str),
    /// Another ticket, without a key yet.
    Unpicked,
    /// Time that is not tracked.
    Untracked,
}

/// A part of the preview from `from` to `to`, or on from `from` when `to` is
/// `None`, `portion` of the preview wide.
#[derive(Debug, PartialEq)]
pub(in crate::gui) struct Part<'a> {
    pub(in crate::gui) segment: Segment<'a>,
    pub(in crate::gui) from: i64,
    pub(in crate::gui) to: Option<i64>,
    pub(in crate::gui) portion: u16,
}

impl<'a> Part<'a> {
    fn new(segment: Segment<'a>, from: i64, to: Option<i64>, portion: u16) -> Self {
        Self {
            segment,
            from,
            to,
            portion,
        }
    }
}

/// A ticket picked from the recent tickets, or a typed key.
#[derive(Default)]
pub(in crate::gui) struct TicketPick {
    key: String,
    picked: Option<Recent>,
}

#[derive(Clone, Debug, PartialEq)]
struct Ticket {
    key: String,
    connection: Option<String>,
}

/// A choice with the ticket picked for [`Choice::Other`].
#[derive(Clone, Debug, PartialEq)]
enum Answer {
    Same,
    Other(Ticket),
    Neither,
}

/// A change to the slots, applied in order.
#[derive(Clone, Debug, PartialEq)]
enum Step {
    Stop(i64),
    Add(Ticket, i64, i64),
    Start(Ticket, i64),
    /// Moves the running slot to the ticket and stops it, or keeps it
    /// running when the stop is `None`.
    Move(Ticket, Option<i64>),
    /// Deletes the running slot.
    Delete,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Away(Choice),
    Now(Choice),
    AwayRecent(Recent),
    AwayKey(String),
    NowRecent(Recent),
    NowKey(String),
    /// Dismisses the dialog and keeps the running slot as it is.
    Keep,
    Save,
    /// The answers were saved, with the text of the toast when something
    /// changed.
    Saved(Result<Option<String>, Failure>),
}

impl Dialog {
    /// Opens the dialog for `period` of the `running` slot. The period starts
    /// at the earliest when the slot started.
    pub(in crate::gui) fn new(period: Period, running: &Slot) -> Self {
        let started_at = running.model.started_at;
        Self {
            period: Period {
                from: period.from.max(started_at),
                to: period.to,
            },
            slot_id: running.model.id,
            started_at,
            running_key: running.ticket.ticket_key.clone(),
            running_connection_id: running.ticket.connection_id,
            away: Choice::default(),
            away_pick: TicketPick::default(),
            now: Choice::default(),
            now_pick: TicketPick::default(),
            saving: false,
        }
    }

    pub(in crate::gui) fn update(
        &mut self,
        message: Message,
        nirvana: Option<&Result<Arc<Nirvana>, Failure>>,
    ) -> Task<Message> {
        if self.saving {
            return Task::none();
        }
        match message {
            Message::Away(choice) => self.away = choice,
            Message::Now(choice) => self.now = choice,
            Message::AwayRecent(recent) => self.away_pick.pick(recent),
            Message::AwayKey(key) => self.away_pick.type_key(key),
            Message::NowRecent(recent) => self.now_pick.pick(recent),
            Message::NowKey(key) => self.now_pick.type_key(key),
            Message::Keep => {
                info!(key = %self.running_key, "idle dialog dismissed, continuing tracking");
                return Task::done(Message::Saved(Ok(None)));
            }
            Message::Save => {
                if let (Some(Ok(nirvana)), Some((away, now))) = (nirvana, self.answers()) {
                    return self.save(nirvana.clone(), away, now);
                }
            }
            Message::Saved(_) => {}
        }
        Task::none()
    }

    pub(in crate::gui) fn period(&self) -> Period {
        self.period
    }

    /// Returns whether the slot started when the user went idle, so nothing
    /// of it is left before the period.
    fn starts_with_slot(&self) -> bool {
        self.period.from == self.started_at
    }

    /// Returns the slot from its start, the period and the time from when the
    /// user is back as they are once the answers are saved, with the parts
    /// that stay one slot joined.
    pub(in crate::gui) fn preview(&self) -> Vec<Part<'_>> {
        let Period { from, to } = self.period;
        let running = self.running_key.as_str();
        let kept = self.away == Choice::Same;
        let before_portion = if self.starts_with_slot() {
            0
        } else {
            BEFORE_PORTION
        };
        let (away_from, away_portion) = if kept {
            (self.started_at, before_portion + AWAY_PORTION)
        } else {
            (from, AWAY_PORTION)
        };
        let continued = self.continued_key();
        let mut parts = Vec::new();
        if !kept && before_portion > 0 {
            parts.push(Part::new(
                Segment::Tracked(running),
                self.started_at,
                Some(from),
                before_portion,
            ));
        }
        match (self.away, self.now, continued) {
            (Choice::Same, Choice::Same, _) => parts.push(Part::new(
                Segment::Running(running),
                away_from,
                None,
                away_portion + NOW_PORTION,
            )),
            (Choice::Neither, Choice::Neither, _) => parts.push(Part::new(
                Segment::Untracked,
                from,
                None,
                AWAY_PORTION + NOW_PORTION,
            )),
            (_, _, Some(key)) => parts.push(Part::new(
                Segment::Running(key),
                from,
                None,
                AWAY_PORTION + NOW_PORTION,
            )),
            _ => parts.extend([
                Part::new(
                    segment(self.away, &self.away_pick, running, Segment::Tracked),
                    away_from,
                    Some(to),
                    away_portion,
                ),
                Part::new(
                    segment(self.now, &self.now_pick, running, Segment::Running),
                    to,
                    None,
                    NOW_PORTION,
                ),
            ]),
        }
        parts
    }

    pub(in crate::gui) fn running_key(&self) -> &str {
        &self.running_key
    }

    pub(in crate::gui) fn away(&self) -> Choice {
        self.away
    }

    pub(in crate::gui) fn now(&self) -> Choice {
        self.now
    }

    pub(in crate::gui) fn away_pick(&self) -> &TicketPick {
        &self.away_pick
    }

    pub(in crate::gui) fn now_pick(&self) -> &TicketPick {
        &self.now_pick
    }

    /// Lets the answers be changed and saved again after saving failed.
    pub(in crate::gui) fn failed(&mut self) {
        self.saving = false;
    }

    /// Returns whether every answer that needs a ticket has one.
    pub(in crate::gui) fn can_save(&self) -> bool {
        !self.saving && self.answers().is_some()
    }

    /// Returns the key of the other ticket picked for both while away and now.
    fn continued_key(&self) -> Option<&str> {
        let (away, now) = self.answers()?;
        continues(&away, &now)
            .then(|| self.away_pick.key())
            .flatten()
    }

    fn answers(&self) -> Option<(Answer, Answer)> {
        Some((
            answer(self.away, &self.away_pick)?,
            answer(self.now, &self.now_pick)?,
        ))
    }

    /// Returns the typed keys of the answers once each, which may not be
    /// stored tickets yet.
    fn typed_keys(&self) -> Vec<String> {
        let mut keys: Vec<_> = [(self.away, &self.away_pick), (self.now, &self.now_pick)]
            .into_iter()
            .filter(|(choice, pick)| *choice == Choice::Other && pick.picked.is_none())
            .filter_map(|(_, pick)| pick.key().map(str::to_owned))
            .collect();
        keys.dedup();
        keys
    }

    /// Loads the typed tickets first, so a key that is not found changes no
    /// slot, then applies the steps.
    fn save(&mut self, nirvana: Arc<Nirvana>, away: Answer, now: Answer) -> Task<Message> {
        let text = summary(&self.running_key, self.period, &away, &now);
        if (&away, &now) == (&Answer::Same, &Answer::Same) {
            info!(key = %self.running_key, "continuing tracking");
            return Task::done(Message::Saved(Ok(Some(text))));
        }
        self.saving = true;
        let typed = self.typed_keys();
        let (period, slot_id, starts_with_slot) =
            (self.period, self.slot_id, self.starts_with_slot());
        let (key, connection_id) = (self.running_key.clone(), self.running_connection_id);
        Task::future(async move {
            for key in typed {
                nirvana
                    .load_ticket(&key, None)
                    .await
                    .map_err(|e| failure(&e))?;
            }
            let running = nirvana.running_slot().await.map_err(|e| failure(&e))?;
            if running.map(|slot| slot.model.id) != Some(slot_id) {
                return Err(Failure::new(SAVE_FAILED, Some(TRACKING_CHANGED)));
            }
            let running = Ticket {
                key,
                connection: connection_name(&nirvana, connection_id)
                    .await
                    .map_err(|e| failure(&e))?,
            };
            for step in steps(&running, period, starts_with_slot, &away, &now) {
                apply(&nirvana, slot_id, step)
                    .await
                    .map_err(|e| failure(&e))?;
            }
            Ok(Some(text))
        })
        .map(Message::Saved)
    }
}

impl TicketPick {
    /// Returns the typed key, which is empty when a recent ticket is picked.
    pub(in crate::gui) fn typed(&self) -> &str {
        match self.picked {
            Some(_) => "",
            None => &self.key,
        }
    }

    pub(in crate::gui) fn is_picked(&self, recent: &Recent) -> bool {
        self.picked.as_ref() == Some(recent)
    }

    /// Returns the key, or `None` when nothing is picked or typed.
    pub(in crate::gui) fn key(&self) -> Option<&str> {
        Some(self.key.trim()).filter(|key| !key.is_empty())
    }

    fn pick(&mut self, recent: Recent) {
        self.key.clone_from(&recent.key);
        self.picked = Some(recent);
    }

    fn type_key(&mut self, key: String) {
        self.key = key.to_uppercase();
        self.picked = None;
    }

    fn ticket(&self) -> Option<Ticket> {
        Some(Ticket {
            key: self.key()?.to_owned(),
            connection: self
                .picked
                .as_ref()
                .and_then(|recent| recent.connection.clone()),
        })
    }
}

fn answer(choice: Choice, pick: &TicketPick) -> Option<Answer> {
    Some(match choice {
        Choice::Same => Answer::Same,
        Choice::Other => Answer::Other(pick.ticket()?),
        Choice::Neither => Answer::Neither,
    })
}

fn segment<'a>(
    choice: Choice,
    pick: &'a TicketPick,
    running: &'a str,
    tracked: fn(&'a str) -> Segment<'a>,
) -> Segment<'a> {
    match choice {
        Choice::Same => tracked(running),
        Choice::Other => pick.key().map_or(Segment::Unpicked, tracked),
        Choice::Neither => Segment::Untracked,
    }
}

async fn connection_name(
    nirvana: &Nirvana,
    connection_id: Option<i64>,
) -> Result<Option<String>, NirvanaError> {
    let Some(id) = connection_id else {
        return Ok(None);
    };
    Ok(nirvana
        .list_connections()
        .await?
        .into_iter()
        .find(|connection| connection.model.id == id)
        .map(|connection| connection.model.name))
}

/// Returns the changes that make the slots match the answers, for the slot
/// on `running` that was running through `period`. When the slot
/// `starts_with_slot`, there is nothing of it to keep before the period.
fn steps(
    running: &Ticket,
    period: Period,
    starts_with_slot: bool,
    away: &Answer,
    now: &Answer,
) -> Vec<Step> {
    let kept = *away == Answer::Same;
    let continued = continues(away, now);
    let mut steps = Vec::new();
    match away {
        Answer::Same => {}
        Answer::Other(ticket) if starts_with_slot => {
            steps.push(Step::Move(
                ticket.clone(),
                (!continued).then_some(period.to),
            ));
        }
        Answer::Other(ticket) if continued => {
            steps.push(Step::Start(ticket.clone(), period.from));
        }
        Answer::Other(ticket) => steps.extend([
            Step::Stop(period.from),
            Step::Add(ticket.clone(), period.from, period.to),
        ]),
        Answer::Neither if starts_with_slot => steps.push(Step::Delete),
        Answer::Neither => steps.push(Step::Stop(period.from)),
    }
    match now {
        Answer::Same if !kept => steps.push(Step::Start(running.clone(), period.to)),
        Answer::Other(_) if continued => {}
        Answer::Other(ticket) => steps.push(Step::Start(ticket.clone(), period.to)),
        Answer::Neither if kept => steps.push(Step::Stop(period.to)),
        Answer::Same | Answer::Neither => {}
    }
    steps
}

/// Returns whether the user was and is on the same other ticket.
fn continues(away: &Answer, now: &Answer) -> bool {
    matches!((away, now), (Answer::Other(away), Answer::Other(now)) if away == now)
}

/// Returns the text of the toast shown once the answers are saved.
fn summary(running: &str, period: Period, away: &Answer, now: &Answer) -> String {
    if (away, now) == (&Answer::Same, &Answer::Same) {
        return format!("continuing tracking {running}");
    }
    if let (Answer::Other(ticket), true) = (away, continues(away, now)) {
        return format!("tracking {} from {}", ticket.key, time_of_day(period.from));
    }
    let duration = format_duration(period.to - period.from);
    let away = match away {
        Answer::Same => format!("kept {duration} on {running}"),
        Answer::Other(ticket) => format!("logged {duration} to {}", ticket.key),
        Answer::Neither => format!("left out {duration} away"),
    };
    let now = match now {
        Answer::Same => format!("now tracking {running}"),
        Answer::Other(ticket) => format!("now tracking {}", ticket.key),
        Answer::Neither => "tracking stopped".to_owned(),
    };
    format!("{away} · {now}")
}

async fn apply(nirvana: &Nirvana, slot_id: i64, step: Step) -> Result<(), NirvanaError> {
    match step {
        Step::Stop(at) => {
            if let Some(slot) = nirvana.stop_slot(Some(at)).await? {
                info!(
                    key = %slot.ticket.ticket_key,
                    slot_id = slot.model.id,
                    stopped_at = slot.model.stopped_at,
                    "slot stopped");
            }
        }
        Step::Add(ticket, from, to) => {
            let slot = nirvana
                .add_slot(&ticket.key, from, to, ticket.connection.as_deref())
                .await?;
            info!(
                key = %slot.ticket.ticket_key,
                slot_id = slot.model.id,
                started_at = slot.model.started_at,
                stopped_at = slot.model.stopped_at,
                "slot added");
        }
        Step::Move(ticket, to) => {
            let slot = nirvana
                .edit_slot(
                    slot_id,
                    Some(&ticket.key),
                    ticket.connection.as_deref(),
                    None,
                    to.map_or(Change::Skip, Change::Set),
                )
                .await?;
            info!(
                key = %slot.ticket.ticket_key,
                slot_id = slot.model.id,
                stopped_at = slot.model.stopped_at,
                "slot moved");
        }
        Step::Delete => {
            nirvana.delete_slot(slot_id).await?;
            info!(slot_id, "slot deleted");
        }
        Step::Start(ticket, at) => {
            let started = nirvana
                .start_slot(&ticket.key, Some(at), ticket.connection.as_deref())
                .await?;
            let slot = &started.started;
            info!(
                key = %slot.ticket.ticket_key,
                slot_id = slot.model.id,
                started_at = slot.model.started_at,
                "slot started");
        }
    }
    Ok(())
}

fn failure(error: &NirvanaError) -> Failure {
    error!("{}", chain(error));
    Failure::from_error(SAVE_FAILED, error)
}

#[cfg(test)]
mod tests {
    use super::*;

    const PERIOD: Period = Period {
        from: 100,
        to: 1060,
    };
    const RUNNING: &str = "NIR-12";

    fn ticket(key: &str) -> Ticket {
        Ticket {
            key: key.into(),
            connection: None,
        }
    }

    fn running() -> Ticket {
        ticket(RUNNING)
    }

    fn other(key: &str) -> Answer {
        Answer::Other(ticket(key))
    }

    #[test]
    fn working_and_continuing_changes_nothing() {
        assert_eq!(
            steps(&running(), PERIOD, false, &Answer::Same, &Answer::Same),
            []
        );
    }

    #[test]
    fn working_then_another_ticket_switches_when_back() {
        assert_eq!(
            steps(&running(), PERIOD, false, &Answer::Same, &other("NIR-9")),
            [Step::Start(ticket("NIR-9"), 1060)]
        );
    }

    #[test]
    fn working_then_stopping_stops_when_back() {
        assert_eq!(
            steps(&running(), PERIOD, false, &Answer::Same, &Answer::Neither),
            [Step::Stop(1060)]
        );
    }

    #[test]
    fn another_ticket_while_away_logs_the_period_to_it() {
        assert_eq!(
            steps(&running(), PERIOD, false, &other("OPS-241"), &Answer::Same),
            [
                Step::Stop(100),
                Step::Add(ticket("OPS-241"), 100, 1060),
                Step::Start(running(), 1060),
            ]
        );
        assert_eq!(
            steps(
                &running(),
                PERIOD,
                false,
                &other("OPS-241"),
                &other("NIR-9")
            ),
            [
                Step::Stop(100),
                Step::Add(ticket("OPS-241"), 100, 1060),
                Step::Start(ticket("NIR-9"), 1060),
            ]
        );
        assert_eq!(
            steps(
                &running(),
                PERIOD,
                false,
                &other("OPS-241"),
                &Answer::Neither
            ),
            [Step::Stop(100), Step::Add(ticket("OPS-241"), 100, 1060)]
        );
    }

    #[test]
    fn a_break_leaves_the_period_out() {
        assert_eq!(
            steps(&running(), PERIOD, false, &Answer::Neither, &Answer::Same),
            [Step::Stop(100), Step::Start(running(), 1060)]
        );
        assert_eq!(
            steps(&running(), PERIOD, false, &Answer::Neither, &other("NIR-9")),
            [Step::Stop(100), Step::Start(ticket("NIR-9"), 1060)]
        );
        assert_eq!(
            steps(
                &running(),
                PERIOD,
                false,
                &Answer::Neither,
                &Answer::Neither
            ),
            [Step::Stop(100)]
        );
    }

    #[test]
    fn another_ticket_from_the_slot_start_moves_the_slot() {
        assert_eq!(
            steps(&running(), PERIOD, true, &other("OPS-241"), &Answer::Same),
            [
                Step::Move(ticket("OPS-241"), Some(1060)),
                Step::Start(running(), 1060),
            ]
        );
    }

    #[test]
    fn the_same_ticket_while_away_and_now_starts_it_when_idle() {
        assert_eq!(
            steps(
                &running(),
                PERIOD,
                false,
                &other("OPS-241"),
                &other("OPS-241")
            ),
            [Step::Start(ticket("OPS-241"), 100)]
        );
    }

    #[test]
    fn the_same_ticket_from_the_slot_start_moves_the_running_slot() {
        assert_eq!(
            steps(
                &running(),
                PERIOD,
                true,
                &other("OPS-241"),
                &other("OPS-241")
            ),
            [Step::Move(ticket("OPS-241"), None)]
        );
    }

    #[test]
    fn a_break_from_the_slot_start_deletes_the_slot() {
        assert_eq!(
            steps(&running(), PERIOD, true, &Answer::Neither, &Answer::Neither),
            [Step::Delete]
        );
        assert_eq!(
            steps(&running(), PERIOD, true, &Answer::Neither, &other("NIR-9")),
            [Step::Delete, Step::Start(ticket("NIR-9"), 1060)]
        );
    }

    #[test]
    fn summary_tells_what_was_saved() {
        assert_eq!(
            summary(RUNNING, PERIOD, &Answer::Same, &Answer::Same),
            "continuing tracking NIR-12"
        );
        assert_eq!(
            summary(RUNNING, PERIOD, &Answer::Same, &Answer::Neither),
            "kept 16m on NIR-12 · tracking stopped"
        );
        assert_eq!(
            summary(RUNNING, PERIOD, &other("OPS-241"), &other("NIR-9")),
            "logged 16m to OPS-241 · now tracking NIR-9"
        );
        assert_eq!(
            summary(RUNNING, PERIOD, &Answer::Neither, &Answer::Neither),
            "left out 16m away · tracking stopped"
        );
        assert_eq!(
            summary(RUNNING, PERIOD, &other("OPS-241"), &other("OPS-241")),
            format!("tracking OPS-241 from {}", time_of_day(PERIOD.from))
        );
    }

    #[test]
    fn another_ticket_needs_a_key() {
        let mut pick = TicketPick::default();
        assert_eq!(answer(Choice::Other, &pick), None);
        pick.type_key(" ops-241 ".into());
        assert_eq!(answer(Choice::Other, &pick), Some(other("OPS-241")));
    }

    #[test]
    fn segments_show_the_answers() {
        let mut pick = TicketPick::default();
        assert_eq!(
            segment(Choice::Same, &pick, RUNNING, Segment::Tracked),
            Segment::Tracked(RUNNING)
        );
        assert_eq!(
            segment(Choice::Same, &pick, RUNNING, Segment::Running),
            Segment::Running(RUNNING)
        );
        assert_eq!(
            segment(Choice::Other, &pick, RUNNING, Segment::Tracked),
            Segment::Unpicked
        );
        assert_eq!(
            segment(Choice::Neither, &pick, RUNNING, Segment::Running),
            Segment::Untracked
        );
        pick.type_key("ops-241".into());
        assert_eq!(
            segment(Choice::Other, &pick, RUNNING, Segment::Running),
            Segment::Running("OPS-241")
        );
    }

    fn dialog(started_at: i64, away: Choice, now: Choice) -> Dialog {
        let mut away_pick = TicketPick::default();
        away_pick.type_key("OPS-241".into());
        let mut now_pick = TicketPick::default();
        now_pick.type_key("NIR-9".into());
        Dialog {
            period: PERIOD,
            slot_id: 1,
            started_at,
            running_key: RUNNING.into(),
            running_connection_id: None,
            away,
            away_pick,
            now,
            now_pick,
            saving: false,
        }
    }

    #[test]
    fn working_and_continuing_previews_one_running_slot() {
        assert_eq!(
            dialog(40, Choice::Same, Choice::Same).preview(),
            [Part::new(Segment::Running(RUNNING), 40, None, 29)]
        );
    }

    #[test]
    fn working_previews_the_slot_kept_through_the_period() {
        assert_eq!(
            dialog(40, Choice::Same, Choice::Other).preview(),
            [
                Part::new(Segment::Tracked(RUNNING), 40, Some(1060), 22),
                Part::new(Segment::Running("NIR-9"), 1060, None, 7),
            ]
        );
    }

    #[test]
    fn another_ticket_previews_the_slot_stopped_when_idle() {
        assert_eq!(
            dialog(40, Choice::Other, Choice::Same).preview(),
            [
                Part::new(Segment::Tracked(RUNNING), 40, Some(100), 6),
                Part::new(Segment::Tracked("OPS-241"), 100, Some(1060), 16),
                Part::new(Segment::Running(RUNNING), 1060, None, 7),
            ]
        );
    }

    #[test]
    fn a_break_and_stopping_previews_nothing_tracked_from_the_period() {
        assert_eq!(
            dialog(100, Choice::Neither, Choice::Neither).preview(),
            [Part::new(Segment::Untracked, 100, None, 23)]
        );
    }

    #[test]
    fn working_then_stopping_previews_the_kept_slot_then_nothing_tracked() {
        assert_eq!(
            dialog(40, Choice::Same, Choice::Neither).preview(),
            [
                Part::new(Segment::Tracked(RUNNING), 40, Some(1060), 22),
                Part::new(Segment::Untracked, 1060, None, 7),
            ]
        );
    }

    #[test]
    fn a_break_and_stopping_previews_the_slot_before_the_period() {
        assert_eq!(
            dialog(40, Choice::Neither, Choice::Neither).preview(),
            [
                Part::new(Segment::Tracked(RUNNING), 40, Some(100), 6),
                Part::new(Segment::Untracked, 100, None, 23),
            ]
        );
    }

    #[test]
    fn the_same_ticket_while_away_and_now_previews_it_running_from_the_period() {
        let mut dialog = dialog(40, Choice::Other, Choice::Other);
        dialog.now_pick.type_key("OPS-241".into());
        assert_eq!(
            dialog.preview(),
            [
                Part::new(Segment::Tracked(RUNNING), 40, Some(100), 6),
                Part::new(Segment::Running("OPS-241"), 100, None, 23),
            ]
        );
    }

    #[test]
    fn a_slot_started_with_the_period_previews_nothing_before_it() {
        assert_eq!(
            dialog(100, Choice::Neither, Choice::Same).preview(),
            [
                Part::new(Segment::Untracked, 100, Some(1060), 16),
                Part::new(Segment::Running(RUNNING), 1060, None, 7),
            ]
        );
    }

    #[test]
    fn the_same_typed_key_is_loaded_once() {
        let mut dialog = dialog(40, Choice::Other, Choice::Other);
        assert_eq!(dialog.typed_keys(), ["OPS-241", "NIR-9"]);
        dialog.now_pick.type_key("ops-241".into());
        assert_eq!(dialog.typed_keys(), ["OPS-241"]);
    }

    #[test]
    fn a_picked_ticket_keeps_its_connection() {
        let mut pick = TicketPick::default();
        pick.pick(Recent {
            id: 1,
            key: "OPS-241".into(),
            title: "staging".into(),
            connection: Some("acme".into()),
            tag: None,
        });
        assert_eq!(pick.typed(), "");
        assert_eq!(
            answer(Choice::Other, &pick),
            Some(Answer::Other(Ticket {
                key: "OPS-241".into(),
                connection: Some("acme".into()),
            }))
        );
    }
}
