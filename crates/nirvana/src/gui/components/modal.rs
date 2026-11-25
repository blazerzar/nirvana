use iced::widget::{Id, column, container, opaque, rule, stack};
use iced::{Border, Element, Length, Padding};

use super::scroll_area;
use crate::gui::design_system::{radius, space, tokens};

const SCROLL: Id = Id::new("modal");
const BORDER_WIDTH: f32 = 1.0;
const FOOTER_HEIGHT: f32 = 54.0;

/// A card over a dimmed window that blocks the mouse to the window. The card
/// is at most `max_width` wide and keeps a margin to the window edges. When
/// the window is too small, only the body scrolls, between the header and the
/// footer, which is as tall as a row of small buttons.
pub(in crate::gui) fn modal<'a, Message: 'a>(
    header: impl Into<Element<'a, Message>>,
    body: impl Into<Element<'a, Message>>,
    footer: impl Into<Element<'a, Message>>,
    max_width: f32,
) -> Element<'a, Message> {
    let header = column![
        container(header).width(Length::Fill).padding(space::S200),
        border(),
    ];
    let body = container(scroll_area(
        SCROLL,
        container(body).padding([space::S100, space::S200]),
        Length::Shrink,
    ))
    .padding(Padding::ZERO.bottom(BORDER_WIDTH + FOOTER_HEIGHT));
    let footer = container(column![
        border(),
        container(footer)
            .width(Length::Fill)
            .height(FOOTER_HEIGHT)
            .padding([space::S150, space::S200]),
    ])
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
            .padding(space::S200)
            .center(Length::Fill)
            .style(|theme| container::background(tokens(theme).bg_overlay)),
    )
}

fn border<'a>() -> rule::Rule<'a> {
    rule::horizontal(BORDER_WIDTH).style(|theme| rule::Style {
        color: tokens(theme).border_default,
        ..rule::default(theme)
    })
}
