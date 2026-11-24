use iced::widget::container;
use iced::widget::scrollable::{self, Rail, Scrollbar, Scroller, Status};
use iced::{Border, Element, Length, Padding, Theme};

use crate::gui::design_system::{radius, tokens};

const GUTTER_WIDTH: f32 = 10.0;
const THUMB_WIDTH: f32 = 6.0;
const THUMB_INSET: f32 = (GUTTER_WIDTH - THUMB_WIDTH) / 2.0;

/// A vertical scrollable with a thin thumb and no track. The thumb is inset
/// from every edge of the gutter.
pub(in crate::gui) fn scroll_area<'a, Message: 'a>(
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let scrollable = scrollable::Scrollable::new(content)
        .direction(scrollable::Direction::Vertical(
            Scrollbar::new()
                .width(GUTTER_WIDTH)
                .scroller_width(THUMB_WIDTH),
        ))
        .height(Length::Fill)
        .style(style);
    container(scrollable)
        .padding(Padding::ZERO.top(THUMB_INSET).bottom(THUMB_INSET))
        .height(Length::Fill)
        .into()
}

fn style(theme: &Theme, status: Status) -> scrollable::Style {
    let tokens = tokens(theme);
    let thumb = match status {
        Status::Dragged {
            is_vertical_scrollbar_dragged: true,
            ..
        } => tokens.text_tertiary,
        Status::Hovered {
            is_vertical_scrollbar_hovered: true,
            ..
        } => tokens.fill_neutral_strong,
        _ => tokens.text_disabled,
    };
    let rail = Rail {
        background: None,
        border: Border::default(),
        scroller: Scroller {
            background: thumb.into(),
            border: Border::default().rounded(radius::FULL),
        },
    };
    scrollable::Style {
        vertical_rail: rail,
        horizontal_rail: rail,
        gap: None,
        ..scrollable::default(theme, status)
    }
}
