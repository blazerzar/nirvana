use iced::widget::toggler;
use iced::{Color, Element};

use crate::gui::design_system::tokens;

const HEIGHT: f32 = 20.0;
const KNOB_INSET: f32 = 0.1;

pub(in crate::gui) fn toggle<'a, Message: 'a>(
    is_on: bool,
    on_toggle: impl Fn(bool) -> Message + 'a,
) -> Element<'a, Message> {
    toggler(is_on)
        .size(HEIGHT)
        .on_toggle(on_toggle)
        .style(move |theme, _| {
            let tokens = tokens(theme);
            let background = if is_on {
                tokens.accent
            } else {
                tokens.fill_neutral_strong
            };
            toggler::Style {
                background: background.into(),
                background_border_width: 0.0,
                background_border_color: Color::TRANSPARENT,
                foreground: Color::WHITE.into(),
                foreground_border_width: 0.0,
                foreground_border_color: Color::TRANSPARENT,
                text_color: None,
                border_radius: None,
                padding_ratio: KNOB_INSET,
            }
        })
        .into()
}
