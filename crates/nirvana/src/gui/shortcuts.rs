use iced::event::{self, Event, Status};
use iced::keyboard::{self, key};
use iced::{Subscription, window};

use super::app::Message;
use super::state::{appearance, settings, start, tracking};

/// A shortcut pressed in a window, handled by the main window only.
#[derive(Clone)]
pub(super) enum Shortcut {
    Escape,
    Settings(settings::Message),
    Start(start::Message),
    Tracking(tracking::Message),
    /// Starts the shown recent ticket at the index.
    Recent(usize),
}

pub(super) fn listen() -> Subscription<Message> {
    event::listen_with(|event, status, window| match (event, status) {
        (Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)), _) => {
            Some(Message::CommandHeld(modifiers.command()))
        }
        (Event::Window(window::Event::Unfocused), _) => Some(Message::CommandHeld(false)),
        (Event::Keyboard(event), Status::Ignored) => shortcut(event, window),
        _ => None,
    })
}

fn shortcut(event: keyboard::Event, window: window::Id) -> Option<Message> {
    if let Some(index) = recent_shortcut(&event) {
        return Some(Message::Shortcut(window, Shortcut::Recent(index)));
    }
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
        (keyboard::Key::Named(key::Named::Escape), _) => Shortcut::Escape,
        _ if is_comma(latin, physical_key) => Shortcut::Settings(settings::Message::Toggle),
        (_, Some('s')) => Shortcut::Start(start::Message::Open),
        (_, Some('x')) => Shortcut::Tracking(tracking::Message::Stop),
        (_, Some('r')) => Shortcut::Tracking(tracking::Message::Resume),
        (_, Some('w')) => Shortcut::Tracking(tracking::Message::Switch),
        _ => return None,
    };
    Some(Message::Shortcut(window, shortcut))
}

/// Returns the zero-based index of the recent ticket that ⌘ or Ctrl with a
/// digit from 1 to 9 picks.
pub(super) fn recent_shortcut(event: &keyboard::Event) -> Option<usize> {
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
    if *modifiers != keyboard::Modifiers::COMMAND {
        return None;
    }
    let digit = key.to_latin(*physical_key)?.to_digit(10)?;
    let index = usize::try_from(digit).ok()?.checked_sub(1)?;
    Some(index)
}

/// Returns whether the event is an Enter press without modifiers.
pub(super) fn is_enter(event: &keyboard::Event) -> bool {
    matches!(
        event,
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key::Named::Enter),
            modifiers,
            repeat: false,
            ..
        } if modifiers.is_empty()
    )
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

    fn key_press(c: &str, modifiers: keyboard::Modifiers) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Character(c.into()),
            modified_key: keyboard::Key::Character(c.into()),
            physical_key: key::Physical::Code(key::Code::Digit1),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat: false,
        }
    }

    #[test]
    fn recent_shortcut_picks_by_command_and_digit() {
        assert_eq!(
            recent_shortcut(&key_press("1", keyboard::Modifiers::COMMAND)),
            Some(0)
        );
        assert_eq!(
            recent_shortcut(&key_press("5", keyboard::Modifiers::COMMAND)),
            Some(4)
        );
    }

    #[test]
    fn recent_shortcut_ignores_zero_and_other_modifiers() {
        assert_eq!(
            recent_shortcut(&key_press("0", keyboard::Modifiers::COMMAND)),
            None
        );
        assert_eq!(
            recent_shortcut(&key_press("1", keyboard::Modifiers::empty())),
            None
        );
        assert_eq!(
            recent_shortcut(&key_press(
                "1",
                keyboard::Modifiers::COMMAND | keyboard::Modifiers::SHIFT
            )),
            None
        );
    }

    fn enter_press(modifiers: keyboard::Modifiers, repeat: bool) -> keyboard::Event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(key::Named::Enter),
            modified_key: keyboard::Key::Named(key::Named::Enter),
            physical_key: key::Physical::Code(key::Code::Enter),
            location: keyboard::Location::Standard,
            modifiers,
            text: None,
            repeat,
        }
    }

    #[test]
    fn enter_matches_a_plain_press() {
        assert!(is_enter(&enter_press(keyboard::Modifiers::empty(), false)));
    }

    #[test]
    fn enter_ignores_modifiers_and_repeats() {
        assert!(!is_enter(&enter_press(keyboard::Modifiers::SHIFT, false)));
        assert!(!is_enter(&enter_press(keyboard::Modifiers::empty(), true)));
    }

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
