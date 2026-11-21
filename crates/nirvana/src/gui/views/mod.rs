mod bottom_bar;
mod header;
mod info;
mod listing;
mod main;
mod tracking;

use iced::widget::rule::{self, Rule};
pub(super) use info::{info, info_size};
pub(super) use main::{MAIN_WINDOW_SIZE, main};

use crate::gui::theme::tokens;

fn border(line: Rule<'_>) -> Rule<'_> {
    line.style(|theme| rule::Style {
        color: tokens(theme).border_default,
        ..rule::default(theme)
    })
}
