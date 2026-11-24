use iced::widget::Id;
use iced::widget::scrollable::{self, Rail, Scrollbar, Scroller, Status};
use iced::{Border, Color, Element, Length, Theme};

use crate::gui::design_system::{radius, tokens};

const GUTTER_WIDTH: f32 = 10.0;
const THUMB_INSET: f32 = 2.0;
const ACTIVE_THUMB_INSET: f32 = 1.0;

/// A vertical scrollable with a thin thumb and no track. The thumb is inset
/// from every edge of the gutter, less while hovered or dragged.
pub(in crate::gui) fn scroll_area<'a, Message: 'a>(
    id: Id,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    scrollable::Scrollable::new(content)
        .id(id)
        .direction(scrollable::Direction::Vertical(
            Scrollbar::new()
                .width(GUTTER_WIDTH)
                .scroller_width(GUTTER_WIDTH),
        ))
        .height(Length::Fill)
        .style(style)
        .into()
}

/// A transparent border clears the edge of the scroller it is drawn over,
/// which insets the thumb.
fn style(theme: &Theme, status: Status) -> scrollable::Style {
    let tokens = tokens(theme);
    let (thumb, inset) = match status {
        Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        } => (tokens.text_tertiary, ACTIVE_THUMB_INSET),
        Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } => (tokens.fill_neutral_strong, ACTIVE_THUMB_INSET),
        _ => (tokens.text_disabled, THUMB_INSET),
    };
    let rail = Rail {
        background: None,
        border: Border::default(),
        scroller: Scroller {
            background: thumb.into(),
            border: Border::default()
                .width(inset)
                .color(Color::TRANSPARENT)
                .rounded(radius::FULL),
        },
    };
    scrollable::Style {
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        ..scrollable::default(theme, status)
    }
}
