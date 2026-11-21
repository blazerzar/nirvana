use std::sync::LazyLock;

use iced::Element;
use iced::alignment::Vertical;
use iced::widget::text::LineHeight;
use iced::widget::{column, container, mouse_area, row, rule, svg, text};

use super::border;
use crate::gui::INTER_BOLD;
use crate::gui::app::Message;
use crate::gui::theme::tokens;

static LOGO: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../assets/logo.svg")));

pub(super) fn header<'a>() -> Element<'a, Message> {
    let logo = mouse_area(svg(LOGO.clone()).width(24).height(24)).on_press(Message::LogoClicked);
    let wordmark = text("nirvana")
        .font(INTER_BOLD)
        .size(16)
        .line_height(LineHeight::Relative(1.25))
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_primary),
        });

    let bar = container(row![logo, wordmark].spacing(12).align_y(Vertical::Center))
        .height(52)
        .padding([0, 16])
        .align_y(Vertical::Center);

    column![bar, border(rule::horizontal(1))].into()
}
