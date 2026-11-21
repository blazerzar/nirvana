use iced::widget::{container, text};
use iced::{Border, Element};

use crate::gui::design_system::typography::BODY_STRONG;
use crate::gui::design_system::{radius, tokens};

const SIZE: f32 = 24.0;

pub(in crate::gui) fn number_badge<'a, Message: 'a>(
    number: u8,
    active: bool,
) -> Element<'a, Message> {
    let label = BODY_STRONG.text(number).style(move |theme| {
        let tokens = tokens(theme);
        text::Style {
            color: Some(if active {
                tokens.text_on_accent
            } else {
                tokens.text_secondary
            }),
        }
    });
    container(label)
        .center(SIZE)
        .style(move |theme| {
            let tokens = tokens(theme);
            let (background, border) = if active {
                (tokens.fill_accent, tokens.border_accent)
            } else {
                (tokens.bg_raised, tokens.border_default)
            };
            container::Style::default()
                .background(background)
                .border(Border {
                    color: border,
                    width: 1.0,
                    radius: radius::FULL.into(),
                })
        })
        .into()
}
