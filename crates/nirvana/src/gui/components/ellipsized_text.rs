use std::borrow::Cow;

use iced::widget::responsive;
use iced::widget::text::{self, Wrapping};
use iced::{Element, Length, Theme};

use crate::gui::design_system::font::MONO_ADVANCE;
use crate::gui::design_system::typography::TextStyle;

const ELLIPSIS: char = '…';

/// Single line monospace text that ends with an ellipsis when it does not
/// fit the width it is given.
pub(in crate::gui) fn ellipsized_text<'a, Message: 'a>(
    style: TextStyle,
    content: &'a str,
    color: fn(&Theme) -> text::Style,
) -> Element<'a, Message> {
    responsive(move |size| {
        let fitting = fitting_chars(size.width, style.size);
        style
            .text(ellipsize(content, fitting))
            .wrapping(Wrapping::None)
            .style(color)
            .into()
    })
    .height(Length::Shrink)
    .into()
}

/// Returns how many monospace characters of `size` fit in `width`.
fn fitting_chars(width: f32, size: f32) -> usize {
    (width / (size * MONO_ADVANCE) + 0.01) as usize
}

/// Shortens `content` to `max_chars` characters, the last one an ellipsis.
fn ellipsize(content: &str, max_chars: usize) -> Cow<'_, str> {
    if content.chars().count() <= max_chars {
        return Cow::Borrowed(content);
    }
    let kept: String = content.chars().take(max_chars.saturating_sub(1)).collect();
    match max_chars {
        0 => Cow::Borrowed(""),
        _ => Cow::Owned(format!("{}{ELLIPSIS}", kept.trim_end())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ellipsize_keeps_text_that_fits() {
        assert_eq!(ellipsize("Daily standup", 13), "Daily standup");
    }

    #[test]
    fn ellipsize_ends_shortened_text_with_an_ellipsis() {
        assert_eq!(ellipsize("Daily standup", 8), "Daily s…");
    }

    #[test]
    fn ellipsize_drops_spaces_before_the_ellipsis() {
        assert_eq!(ellipsize("Daily standup", 7), "Daily…");
    }

    #[test]
    fn ellipsize_shows_nothing_without_room() {
        assert_eq!(ellipsize("Daily standup", 0), "");
    }

    #[test]
    fn fitting_chars_counts_whole_characters() {
        assert_eq!(fitting_chars(72.0, 12.0), 10);
        assert_eq!(fitting_chars(71.0, 12.0), 9);
    }
}
