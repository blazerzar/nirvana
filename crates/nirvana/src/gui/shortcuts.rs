use iced::event::{self, Event, Status};
use iced::keyboard::{self, key};
use iced::{Subscription, window};

use super::app::Message;
use super::state::{appearance, settings, tracking};

/// A shortcut pressed in a window, handled by the main window only.
#[derive(Clone)]
pub(super) enum Shortcut {
    Settings(settings::Message),
    Tracking(tracking::Message),
}

pub(super) fn listen() -> Subscription<Message> {
    event::listen_with(|event, status, window| match (event, status) {
        (Event::Keyboard(event), Status::Ignored) => shortcut(event, window),
        _ => None,
    })
}

fn shortcut(event: keyboard::Event, window: window::Id) -> Option<Message> {
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
    let latin = key.to_latin(physical_key);
    if modifiers.command()
        && modifiers.shift()
        && latin.is_some_and(|c| c.eq_ignore_ascii_case(&'t'))
    {
        return Some(Message::Appearance(appearance::Message::Toggle));
    }
    if !modifiers.is_empty() {
        return None;
    }
    let shortcut = match (key, latin) {
        (keyboard::Key::Named(key::Named::Escape), _) => {
            Shortcut::Settings(settings::Message::Close)
        }
        _ if is_comma(latin, physical_key) => Shortcut::Settings(settings::Message::Toggle),
        (_, Some('x')) => Shortcut::Tracking(tracking::Message::Stop),
        (_, Some('r')) => Shortcut::Tracking(tracking::Message::Resume),
        (_, Some('w')) => Shortcut::Tracking(tracking::Message::Switch),
        _ => return None,
    };
    Some(Message::Shortcut(window, shortcut))
}

/// Matches the comma by its character, or by its physical key on layouts
/// where the key has no Latin character.
fn is_comma(latin: Option<char>, physical_key: key::Physical) -> bool {
    match latin {
        Some(c) => c == ',',
        None => physical_key == key::Physical::Code(key::Code::Comma),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const COMMA_KEY: key::Physical = key::Physical::Code(key::Code::Comma);

    #[test]
    fn comma_matches_its_character_on_any_key() {
        assert!(is_comma(Some(','), key::Physical::Code(key::Code::KeyM)));
    }

    #[test]
    fn comma_key_with_another_latin_character_is_not_comma() {
        assert!(!is_comma(Some(';'), COMMA_KEY));
    }

    #[test]
    fn comma_key_without_latin_character_is_comma() {
        assert!(is_comma(None, COMMA_KEY));
    }
}
