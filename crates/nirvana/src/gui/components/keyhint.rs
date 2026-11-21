use iced::widget::{container, text};
use iced::{Border, Element};

use crate::gui::design_system::typography::KBD;
use crate::gui::design_system::{DISABLED_OPACITY, radius, space, tokens};

pub(in crate::gui) fn keyhint<'a, Message: 'a>(
    key: &'a str,
    disabled: bool,
) -> Element<'a, Message> {
    let opacity = if disabled { DISABLED_OPACITY } else { 1.0 };
    let label = KBD.text(key).style(move |theme| text::Style {
        color: Some(tokens(theme).text_accent.scale_alpha(opacity)),
    });
    container(label)
        .padding([space::S0, space::S050])
        .style(move |theme| {
            let tokens = tokens(theme);
            container::Style::default()
                .background(tokens.fill_accent_subtle.scale_alpha(opacity))
                .border(Border {
                    color: tokens.border_accent.scale_alpha(opacity),
                    width: 1.0,
                    radius: radius::SM.into(),
                })
        })
        .into()
}
