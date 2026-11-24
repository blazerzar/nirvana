use iced::alignment::Vertical;
use iced::widget::{Column, column, container, row, rule};
use iced::{Element, Length, Padding};

use super::slot_row::{
    Mark, accented, colored, duration_text, mark_glyph, slot_container, ticket, time_range,
};
use crate::gui::design_system::typography::{BODY, CAPTION, LABEL_STRONG};
use crate::gui::design_system::{space, tokens};

/// A slot of a ticket group: its mark, time range and duration.
pub(in crate::gui) struct GroupSlot {
    pub(in crate::gui) mark: Mark,
    pub(in crate::gui) range: (String, String),
    pub(in crate::gui) duration: String,
}

/// The key, title, slot count and `total` of a ticket, above its `slots`.
/// A running ticket is accented.
pub(in crate::gui) fn ticket_group<'a, Message: 'a>(
    key: &'a str,
    title: &'a str,
    total: String,
    slots: Vec<GroupSlot>,
) -> Element<'a, Message> {
    let is_running = slots.iter().any(|slot| slot.mark.is_running());
    let header = row![
        ticket(key, title, is_running),
        CAPTION
            .text(slot_count(slots.len()))
            .style(colored(|tokens| tokens.text_tertiary)),
        duration_text(
            LABEL_STRONG,
            total,
            accented(is_running, |tokens| tokens.text_primary)
        ),
    ]
    .spacing(space::S100)
    .align_y(Vertical::Center);
    let line = rule::vertical(1).style(|theme| rule::Style {
        color: tokens(theme).border_default,
        ..rule::default(theme)
    });
    let slots = Column::with_children(slots.into_iter().map(group_slot))
        .padding(Padding::ZERO.left(space::S050));
    column![
        container(header).padding([space::S075, space::S100]),
        container(row![line, slots]).padding(Padding::ZERO.left(space::S100)),
    ]
    .into()
}

fn group_slot<'a, Message: 'a>(slot: GroupSlot) -> Element<'a, Message> {
    let is_running = slot.mark.is_running();
    let content = row![
        mark_glyph(slot.mark),
        container(time_range(BODY, slot.range, is_running)).width(Length::Fill),
        duration_text(
            BODY,
            slot.duration,
            accented(is_running, |tokens| tokens.text_secondary)
        ),
    ]
    .spacing(space::S100)
    .align_y(Vertical::Center);
    slot_container(content, [space::S050, space::S100].into(), is_running)
}

/// Formats `slots` as e.g. `3 slots`.
fn slot_count(slots: usize) -> String {
    match slots {
        1 => "1 slot".to_owned(),
        slots => format!("{slots} slots"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_count_counts_slots() {
        assert_eq!(slot_count(1), "1 slot");
        assert_eq!(slot_count(3), "3 slots");
    }
}
