use iced::alignment::Vertical;
use iced::widget::{Row, row, space, text};
use iced::{Element, Theme};

use super::tracked_text;
use crate::gui::design_system::typography::LABEL_CAPS;
use crate::gui::design_system::{space as spacing, tokens};

/// A caps label after `leading`, with `right` at the other end of the line.
pub(in crate::gui) fn pane_label<'a, Message: 'a>(
    leading: Option<Element<'a, Message>>,
    label: &str,
    right: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let label = Row::new()
        .push(leading)
        .push(tracked_text(LABEL_CAPS, label, tertiary))
        .spacing(spacing::S075)
        .align_y(Vertical::Center);
    row![label, space::horizontal(), right.into()]
        .align_y(Vertical::Center)
        .into()
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
