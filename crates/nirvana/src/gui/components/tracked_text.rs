use iced::widget::{Row, text};
use iced::{Element, Theme};

use crate::gui::design_system::typography::TextStyle;

const NO_BREAK_SPACE: &str = "\u{a0}";

/// Single line text with the style's letter spacing, one widget per character.
pub(in crate::gui) fn tracked_text<'a, Message: 'a>(
    style: TextStyle,
    content: &str,
    color: fn(&Theme) -> text::Style,
) -> Element<'a, Message> {
    let characters = content.chars().map(|character| {
        let glyph = if character == ' ' {
            NO_BREAK_SPACE.to_owned()
        } else {
            character.to_string()
        };
        style.text(glyph).style(color).into()
    });
    Row::with_children(characters)
        .spacing(style.tracking * style.size)
        .into()
}
