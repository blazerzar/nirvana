use iced::widget::{Row, button, container};
use iced::{Border, Element, Shadow};

use crate::gui::design_system::typography::{CAPTION, CAPTION_STRONG};
use crate::gui::design_system::{radius, space, tokens};

/// A segmented control: one option of `options` is selected, and pressing
/// another publishes `on_select` with its value.
pub(in crate::gui) fn segmented<'a, T: Copy + PartialEq + 'a, Message: Clone + 'a>(
    options: impl IntoIterator<Item = (T, String)>,
    selected: T,
    on_select: impl Fn(T) -> Message,
) -> Element<'a, Message> {
    let options = options.into_iter().map(|(value, label)| {
        let is_selected = value == selected;
        let label = if is_selected {
            CAPTION_STRONG.text(label)
        } else {
            CAPTION.text(label)
        };
        button(label)
            .padding([space::S050, space::S100])
            .on_press(on_select(value))
            .style(move |theme, status| {
                let tokens = tokens(theme);
                let (background, text, shadow) = match (is_selected, status) {
                    (true, _) => (
                        Some(tokens.bg_raised),
                        tokens.text_primary,
                        tokens.shadow_sm,
                    ),
                    (false, button::Status::Hovered) => (
                        Some(tokens.fill_hover),
                        tokens.text_primary,
                        Shadow::default(),
                    ),
                    (false, button::Status::Pressed) => (
                        Some(tokens.fill_pressed),
                        tokens.text_primary,
                        Shadow::default(),
                    ),
                    (false, _) => (None, tokens.text_secondary, Shadow::default()),
                };
                button::Style {
                    background: background.map(Into::into),
                    text_color: text,
                    border: Border::default().rounded(radius::SM),
                    shadow,
                    ..button::Style::default()
                }
            })
            .into()
    });
    container(Row::with_children(options).spacing(space::S025))
        .padding(space::S025)
        .style(|theme| {
            let tokens = tokens(theme);
            container::Style {
                background: Some(tokens.bg_default.into()),
                border: Border::default()
                    .width(1)
                    .color(tokens.border_default)
                    .rounded(radius::MD),
                ..container::Style::default()
            }
        })
        .into()
}
