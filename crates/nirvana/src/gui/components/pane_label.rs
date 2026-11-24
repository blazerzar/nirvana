use iced::alignment::Vertical;
use iced::widget::{row, space, text};
use iced::{Element, Theme};

use super::tracked_text;
use crate::gui::design_system::tokens;
use crate::gui::design_system::typography::LABEL_CAPS;

/// A caps label with `right` at the other end of the line.
pub(in crate::gui) fn pane_label<'a, Message: 'a>(
    label: &str,
    right: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    row![
        tracked_text(LABEL_CAPS, label, tertiary),
        space::horizontal(),
        right.into()
    ]
    .align_y(Vertical::Center)
    .into()
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
