use iced::widget::text::IntoFragment;
use iced::widget::{container, text};
use iced::{Border, Element, Padding};

use crate::gui::design_system::typography::KBD;
use crate::gui::design_system::{radius, space, tokens};

pub(in crate::gui) const RETURN_KEY: &str = "↵";

/// On a primary button the hint has no fill and takes the button's text color.
pub(in crate::gui) fn keyhint<'a, Message: 'a>(
    key: impl IntoFragment<'a>,
    disabled: bool,
    primary: bool,
) -> Element<'a, Message> {
    let key = key.into_fragment();
    let top = if key == RETURN_KEY {
        space::S025
    } else {
        space::S0
    };
    let label = KBD.text(key).style(move |theme| {
        let tokens = tokens(theme);
        let opacity = tokens.opacity(!disabled);
        let color = if primary {
            tokens.text_on_accent
        } else {
            tokens.text_accent
        };
        text::Style {
            color: Some(color.scale_alpha(opacity)),
        }
    });
    container(label)
        .padding(Padding::from([space::S0, space::S050]).top(top))
        .style(move |theme| {
            let tokens = tokens(theme);
            let opacity = tokens.opacity(!disabled);
            let style = container::Style::default();
            let style = if primary {
                style
            } else {
                style.background(tokens.fill_accent_subtle.scale_alpha(opacity))
            };
            style.border(Border {
                color: tokens.border_accent.scale_alpha(opacity),
                width: 1.0,
                radius: radius::SM.into(),
            })
        })
        .into()
}
