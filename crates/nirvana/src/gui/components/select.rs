use std::fmt::Display;

use iced::alignment::Vertical;
use iced::widget::{Column, Space, button, container, row, stack, text};
use iced::{Border, Color, Element, Length, Theme};

use super::Size;
use super::dropdown::Dropdown;
use crate::gui::design_system::typography::{BODY, BODY_LG, BODY_STRONG, MICRO};
use crate::gui::design_system::{radius, space, tokens};

const SM_TRIGGER_HEIGHT: f32 = 26.0;
const SM_ITEM_HEIGHT: f32 = 28.0;
const LG_TRIGGER_HEIGHT: f32 = 36.0;
const LG_ITEM_HEIGHT: f32 = 32.0;
const CHECK_WIDTH: f32 = 12.0;

/// A desktop select. The menu opens below the trigger while `is_open` and
/// marks `selected` with a check. Disabled until `on_toggle` is set.
pub(in crate::gui) fn select<'a, T, Message>(
    size: Size,
    options: Vec<T>,
    selected: T,
    is_open: bool,
    on_toggle: Option<Message>,
    on_select: impl Fn(T) -> Message,
    on_dismiss: Message,
) -> Element<'a, Message>
where
    T: Display + PartialEq + 'a,
    Message: Clone + 'a,
{
    let (trigger_height, padding, label_style, item_height) = match size {
        Size::Sm => (SM_TRIGGER_HEIGHT, space::S100, BODY, SM_ITEM_HEIGHT),
        Size::Lg => (LG_TRIGGER_HEIGHT, space::S150, BODY_LG, LG_ITEM_HEIGHT),
    };
    let enabled = on_toggle.is_some();
    let chevron = MICRO
        .text(if is_open { "▲" } else { "▼" })
        .style(move |theme| {
            let tokens = tokens(theme);
            text::Style {
                color: Some(tokens.text_tertiary.scale_alpha(tokens.opacity(enabled))),
            }
        });
    let trigger = button(
        row![label_style.text(selected.to_string()), chevron]
            .spacing(space::S100)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .height(trigger_height)
    .padding([space::S0, padding])
    .on_press_maybe(on_toggle)
    .style(move |theme, status| trigger_style(theme, status, is_open));

    let labels: Vec<String> = options.iter().map(ToString::to_string).collect();
    let widest = labels
        .iter()
        .max_by_key(|label| label.chars().count())
        .cloned()
        .unwrap_or_default();
    let items = options.into_iter().zip(labels).map(|(option, label)| {
        let is_selected = option == selected;
        item(label, &widest, is_selected, item_height, on_select(option))
    });
    let menu = container(Column::with_children(items).spacing(space::S025))
        .padding(space::S050)
        .style(|theme| {
            let tokens = tokens(theme);
            container::Style::default()
                .background(tokens.bg_raised)
                .border(Border {
                    color: tokens.border_default,
                    width: 1.0,
                    radius: radius::MD.into(),
                })
                .shadow(tokens.shadow_lg)
        });

    Dropdown::new(trigger, menu, is_open, space::S050, on_dismiss).into()
}

/// An option of the menu, as wide as an option showing `widest`, so every
/// option has the same width and the menu fits its longest label.
fn item<'a, Message: Clone + 'a>(
    label: String,
    widest: &str,
    is_selected: bool,
    height: f32,
    on_press: Message,
) -> Element<'a, Message> {
    let (check, label) = if is_selected {
        (BODY_STRONG.text("✓"), BODY_STRONG.text(label))
    } else {
        (BODY.text(""), BODY.text(label))
    };
    let width = BODY_STRONG.text(widest.to_owned()).style(|_| text::Style {
        color: Some(Color::TRANSPARENT),
    });
    let content = stack![
        row![Space::new().width(CHECK_WIDTH), width].spacing(space::S100),
        row![container(check).width(CHECK_WIDTH), label].spacing(space::S100),
    ];
    button(
        container(content)
            .height(Length::Fill)
            .align_y(Vertical::Center),
    )
    .height(height)
    .padding([space::S0, space::S100])
    .on_press(on_press)
    .style(move |theme, status| item_style(theme, status, is_selected))
    .into()
}

fn trigger_style(theme: &Theme, status: button::Status, is_open: bool) -> button::Style {
    let tokens = tokens(theme);
    let (background, opacity) = match status {
        button::Status::Hovered | button::Status::Pressed if !is_open => (tokens.fill_hover, 1.0),
        button::Status::Disabled => (tokens.bg_raised, tokens.disabled_opacity),
        _ => (tokens.bg_raised, 1.0),
    };
    let border = if is_open {
        tokens.border_focus
    } else {
        tokens.border_default
    };
    button::Style {
        background: Some(background.scale_alpha(opacity).into()),
        text_color: tokens.text_primary.scale_alpha(opacity),
        border: Border {
            color: border.scale_alpha(opacity),
            width: 1.0,
            radius: radius::SM.into(),
        },
        ..button::Style::default()
    }
}

fn item_style(theme: &Theme, status: button::Status, is_selected: bool) -> button::Style {
    let tokens = tokens(theme);
    let (background, text) = match status {
        _ if is_selected => (tokens.fill_accent_subtle, tokens.text_accent),
        button::Status::Hovered => (tokens.fill_hover, tokens.text_primary),
        button::Status::Pressed => (tokens.fill_pressed, tokens.text_primary),
        _ => (Color::TRANSPARENT, tokens.text_primary),
    };
    button::Style {
        background: Some(background.into()),
        text_color: text,
        border: Border {
            radius: radius::SM.into(),
            ..Border::default()
        },
        ..button::Style::default()
    }
}
