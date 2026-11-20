use std::sync::LazyLock;

use iced::Element;
use iced::alignment::Vertical;
use iced::widget::text::LineHeight;
use iced::widget::{column, container, row, rule, svg, text};

use super::{BORDER_DEFAULT, INTER_BOLD, Message, TEXT_PRIMARY};

static LOGO: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../assets/logo.svg")));

pub(super) fn header<'a>() -> Element<'a, Message> {
    let logo = svg(LOGO.clone()).width(24).height(24);
    let wordmark = text("nirvana")
        .font(INTER_BOLD)
        .size(16)
        .line_height(LineHeight::Relative(1.25))
        .color(TEXT_PRIMARY);

    let bar = container(row![logo, wordmark].spacing(12).align_y(Vertical::Center))
        .height(52)
        .padding([0, 16])
        .align_y(Vertical::Center);

    let border = rule::horizontal(1).style(|theme| rule::Style {
        color: BORDER_DEFAULT,
        ..rule::default(theme)
    });

    column![bar, border].into()
}
