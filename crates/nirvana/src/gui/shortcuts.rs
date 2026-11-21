use iced::keyboard;

use super::app::Message;
use super::appearance;

pub(super) fn shortcut(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed {
        key,
        physical_key,
        modifiers,
        repeat: false,
        ..
    } = event
    else {
        return None;
    };
    let is_t = key
        .to_latin(physical_key)
        .is_some_and(|c| c.eq_ignore_ascii_case(&'t'));
    (modifiers.command() && modifiers.shift() && is_t)
        .then_some(Message::Appearance(appearance::Message::Toggle))
}
