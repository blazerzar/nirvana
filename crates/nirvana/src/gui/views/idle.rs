use iced::Element;
use iced::widget::space;

use crate::gui::app::Message;
use crate::gui::components::modal;

const WIDTH: f32 = 720.0;

/// The modal shown when the user is back after being idle while tracking.
pub(in crate::gui) fn idle_modal<'a>() -> Element<'a, Message> {
    modal(space(), space(), space(), WIDTH)
}
