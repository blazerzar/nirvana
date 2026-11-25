use iced::widget::{Id, column, container, opaque, stack};
use iced::{Border, Element, Length, Padding};

use super::scroll_area;
use crate::gui::design_system::{radius, space, tokens};

const SCROLL: Id = Id::new("modal");
const BORDER_WIDTH: f32 = 1.0;
const FOOTER_HEIGHT: f32 = 78.0;

/// A card over a dimmed window that blocks the mouse to the window. The card
/// is at most `max_width` wide and keeps a margin to the window edges. When
/// the window is too small, only the body scrolls, between the header and the
/// footer.
pub(in crate::gui) fn modal<'a, Message: 'a>(
    header: impl Into<Element<'a, Message>>,
    body: impl Into<Element<'a, Message>>,
    footer: impl Into<Element<'a, Message>>,
    max_width: f32,
) -> Element<'a, Message> {
    let header = container(header)
        .width(Length::Fill)
        .padding(Padding::new(space::S300).bottom(space::S200));
    let body = container(scroll_area(
        SCROLL,
        container(body).padding(Padding::from([space::S0, space::S300])),
        Length::Shrink,
    ))
    .padding(Padding::ZERO.bottom(FOOTER_HEIGHT));
    let footer = container(
        container(footer)
            .width(Length::Fill)
            .height(FOOTER_HEIGHT)
            .padding(Padding::new(space::S300).top(space::S200)),
    )
    .align_bottom(Length::Fill);
    let card = container(stack![column![header, body], footer])
        .width(Length::Fill)
        .max_width(max_width)
        .style(|theme| {
            let tokens = tokens(theme);
            container::Style::default()
                .background(tokens.bg_raised)
                .border(Border {
                    color: tokens.border_default,
                    width: BORDER_WIDTH,
                    radius: radius::LG.into(),
                })
                .shadow(tokens.shadow_lg)
        });
    opaque(
        container(card)
            .padding(space::S300)
            .center(Length::Fill)
            .style(|theme| container::background(tokens(theme).bg_overlay)),
    )
}
