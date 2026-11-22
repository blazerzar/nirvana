use iced::alignment::Vertical;
use iced::widget::{row, text};
use iced::{Element, Theme};

use crate::gui::design_system::tokens;
use crate::gui::design_system::typography::CAPTION;

const IS_MAC: bool = cfg!(target_os = "macos");
const MODIFIER: &str = if IS_MAC { "⌘" } else { "^" };

/// The ⌘ or Ctrl shortcut of `key`.
pub(in crate::gui) fn shortcut_hint<'a, Message: 'a>(
    key: String,
    is_active: bool,
) -> Element<'a, Message> {
    let style = move |theme: &Theme| text::Style {
        color: Some(if is_active {
            tokens(theme).text_accent
        } else {
            tokens(theme).text_tertiary
        }),
    };
    let align = if IS_MAC {
        Vertical::Center
    } else {
        Vertical::Top
    };
    row![
        CAPTION.text(MODIFIER).style(style),
        CAPTION.text(key).style(style)
    ]
    .align_y(align)
    .into()
}
