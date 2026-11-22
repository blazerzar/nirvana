use std::collections::BTreeMap;

use iced::{Size, Task, window};

use super::state::info::Info;

pub(super) enum Window {
    Main,
    Info(Info),
}

#[derive(Default)]
pub(super) struct Windows(BTreeMap<window::Id, Window>);

impl Windows {
    pub(super) fn open(&mut self, kind: Window, size: Size) -> Task<window::Id> {
        let (id, open) = window::open(window::Settings {
            size,
            ..window::Settings::default()
        });
        self.0.insert(id, kind);
        open
    }

    pub(super) fn get(&self, id: window::Id) -> Option<&Window> {
        self.0.get(&id)
    }

    pub(super) fn remove(&mut self, id: window::Id) -> Option<Window> {
        self.0.remove(&id)
    }

    pub(super) fn main(&self) -> Option<window::Id> {
        self.0
            .iter()
            .find_map(|(id, window)| matches!(window, Window::Main).then_some(*id))
    }

    pub(super) fn info(&self) -> Option<(window::Id, &Info)> {
        self.0.iter().find_map(|(id, window)| match window {
            Window::Info(info) => Some((*id, info)),
            Window::Main => None,
        })
    }
}
