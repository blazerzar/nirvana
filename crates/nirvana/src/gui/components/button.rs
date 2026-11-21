use iced::alignment::Vertical;
use iced::widget::text::{Fragment, IntoFragment};
use iced::widget::{button, row, text};
use iced::{Border, Element, Length, Theme};

use super::keyhint;
use crate::gui::design_system::typography::{BODY, LABEL, LABEL_SM};
use crate::gui::design_system::{DISABLED_OPACITY, radius, space, tokens};

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
    key: Option<&'a str>,
    hint: Option<Fragment<'a>>,
    on_press: Option<Message>,
}

impl<'a, Message> Button<'a, Message> {
    pub(in crate::gui) fn new(label: impl IntoFragment<'a>) -> Self {
        Self {
            label: label.into_fragment(),
            size: Size::default(),
            key: None,
            hint: None,
            on_press: None,
        }
    }

    pub(in crate::gui) fn size(mut self, size: Size) -> Self {
        self.size = size;
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
        let opacity = if disabled { DISABLED_OPACITY } else { 1.0 };
        let (height, padding, label_style) = match button.size {
            Size::Sm => (30.0, space::S150, LABEL_SM),
            Size::Lg => (38.0, space::S200, LABEL),
        };

        let key = button.key.map(|key| keyhint(key, disabled));
        let label = label_style.text(button.label);
        let hint = button.hint.map(|hint| {
            BODY.text(hint).style(move |theme| text::Style {
                color: Some(tokens(theme).text_tertiary.scale_alpha(opacity)),
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
            .style(style)
            .into()
    }
}

fn style(theme: &Theme, status: button::Status) -> button::Style {
    let tokens = tokens(theme);
    let (background, opacity) = match status {
        button::Status::Active => (tokens.bg_raised, 1.0),
        button::Status::Hovered => (tokens.fill_hover, 1.0),
        button::Status::Pressed => (tokens.fill_pressed, 1.0),
        button::Status::Disabled => (tokens.bg_raised, DISABLED_OPACITY),
    };
    button::Style {
        background: Some(background.scale_alpha(opacity).into()),
        text_color: tokens.text_primary.scale_alpha(opacity),
        border: Border {
            color: tokens.border_default.scale_alpha(opacity),
            width: 1.0,
            radius: radius::MD.into(),
        },
        ..button::Style::default()
    }
}
