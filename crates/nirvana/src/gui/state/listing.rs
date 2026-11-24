use std::sync::Arc;

use chrono::{Local, NaiveDate};
use iced::Task;
use nirvana_core::{Nirvana, PublishState, Slot};
use tracing::error;

use crate::gui::failure::{Failure, chain};
use crate::time::day_start;

const LOAD_FAILED: &str = "couldn't load today's slots";

/// The stopped slots of today with their publish states.
#[derive(Default)]
pub(in crate::gui) struct State {
    loaded: Option<Loaded>,
}

#[derive(Clone, Debug)]
pub(in crate::gui) struct Loaded {
    today: NaiveDate,
    slots: Vec<(Slot, PublishState)>,
}

#[derive(Clone)]
pub(in crate::gui) enum Message {
    Load,
    Loaded(Loaded),
    Failed(Failure),
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

    pub(in crate::gui) fn today(&self) -> Option<NaiveDate> {
        self.loaded.as_ref().map(|loaded| loaded.today)
    }

    /// Returns the stopped slots of today, newest first.
    pub(in crate::gui) fn slots(&self) -> &[(Slot, PublishState)] {
        self.loaded.as_ref().map_or(&[], |loaded| &loaded.slots)
    }
}

fn load(nirvana: Arc<Nirvana>) -> Task<Message> {
    let today = Local::now().date_naive();
    let from = match day_start(today) {
        Ok(from) => from,
        Err(e) => {
            error!("{e:#}");
            return Task::done(Message::Failed(Failure::new(LOAD_FAILED, None)));
        }
    };
    Task::future(async move { nirvana.list_slots(from, None, false).await }).map(move |listed| {
        match listed {
            Ok(listed) => Message::Loaded(Loaded {
                today,
                slots: stopped_newest_first(listed),
            }),
            Err(e) => {
                error!("{}", chain(&e));
                Message::Failed(Failure::from_error(LOAD_FAILED, &e))
            }
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
