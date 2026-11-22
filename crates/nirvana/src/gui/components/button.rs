use iced::alignment::Vertical;
use iced::widget::text::{Fragment, IntoFragment};
use iced::widget::{button, row, text};
use iced::{Border, Element, Length, Theme};

use super::keyhint;
use crate::gui::design_system::typography::{BODY, BODY_STRONG, LABEL, LABEL_SM, LABEL_STRONG};
use crate::gui::design_system::{radius, space, tokens};

#[derive(Clone, Copy, Default)]
pub(in crate::gui) enum Size {
    Sm,
    #[default]
    Lg,
}

/// Disabled until `on_press` is set.
pub(in crate::gui) struct Button<'a, Message> {
    label: Fragment<'a>,
    size: Size,
    primary: bool,
    outline: bool,
    key: Option<&'a str>,
    hint: Option<Fragment<'a>>,
    on_press: Option<Message>,
}

impl<'a, Message> Button<'a, Message> {
    pub(in crate::gui) fn new(label: impl IntoFragment<'a>) -> Self {
        Self {
            label: label.into_fragment(),
            size: Size::default(),
            primary: false,
            outline: false,
            key: None,
            hint: None,
            on_press: None,
        }
    }

    pub(in crate::gui) fn size(mut self, size: Size) -> Self {
        self.size = size;
        self
    }

    pub(in crate::gui) fn primary(mut self) -> Self {
        self.primary = true;
        self
    }

    /// Draws the border in the accent color.
    pub(in crate::gui) fn outline(mut self) -> Self {
        self.outline = true;
        self
    }

    pub(in crate::gui) fn key(mut self, key: &'a str) -> Self {
        self.key = Some(key);
        self
    }

    pub(in crate::gui) fn hint(mut self, hint: impl IntoFragment<'a>) -> Self {
        self.hint = Some(hint.into_fragment());
        self
    }

    pub(in crate::gui) fn on_press(mut self, message: Message) -> Self {
        self.on_press = Some(message);
        self
    }
}

impl<'a, Message: Clone + 'a> From<Button<'a, Message>> for Element<'a, Message> {
    fn from(button: Button<'a, Message>) -> Self {
        let disabled = button.on_press.is_none();
        let primary = button.primary;
        let outline = button.outline;
        let (height, padding, label_style) = match (button.size, primary) {
            (Size::Sm, false) => (30.0, space::S150, LABEL_SM),
            (Size::Sm, true) => (30.0, space::S150, BODY_STRONG),
            (Size::Lg, false) => (38.0, space::S200, LABEL),
            (Size::Lg, true) => (38.0, space::S200, LABEL_STRONG),
        };

        let key = button.key.map(|key| keyhint(key, disabled, primary));
        let label = label_style.text(button.label);
        let hint = button.hint.map(|hint| {
            BODY.text(hint).style(move |theme| {
                let tokens = tokens(theme);
                let color = if primary {
                    tokens.text_on_accent
                } else {
                    tokens.text_tertiary
                };
                text::Style {
                    color: Some(color.scale_alpha(tokens.opacity(!disabled))),
                }
            })
        });
        let content = row![]
            .push(key)
            .push(label)
            .push(hint)
            .spacing(space::S100)
            .height(Length::Fill)
            .align_y(Vertical::Center);

        iced::widget::button(content)
            .height(height)
            .padding([space::S0, padding])
            .on_press_maybe(button.on_press)
            .style(move |theme, status| style(theme, status, primary, outline))
            .into()
    }
}

fn style(theme: &Theme, status: button::Status, primary: bool, outline: bool) -> button::Style {
    let tokens = tokens(theme);
    let (active, hovered, pressed, text, border) = if primary {
        (
            tokens.fill_accent,
            tokens.fill_accent_hover,
            tokens.fill_accent_pressed,
            tokens.text_on_accent,
            tokens.border_accent,
        )
    } else {
        (
            tokens.bg_raised,
            tokens.fill_hover,
            tokens.fill_pressed,
            tokens.text_primary,
            if outline {
                tokens.border_accent
            } else {
                tokens.border_default
            },
        )
    };
    let (background, opacity) = match status {
        button::Status::Active => (active, 1.0),
        button::Status::Hovered => (hovered, 1.0),
        button::Status::Pressed => (pressed, 1.0),
        button::Status::Disabled => (active, tokens.disabled_opacity),
    };
    button::Style {
        background: Some(background.scale_alpha(opacity).into()),
        text_color: text.scale_alpha(opacity),
        border: Border {
            color: border.scale_alpha(opacity),
            width: 1.0,
            radius: radius::MD.into(),
        },
        ..button::Style::default()
    }
}
