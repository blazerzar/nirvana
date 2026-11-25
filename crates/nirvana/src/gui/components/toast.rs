use std::sync::LazyLock;

use iced::border::Border;
use iced::widget::{Space, button, column, container, row, stack, svg, text};
use iced::{Color, Element, Length, Padding};

use super::{edge_marker, ghost_style};
use crate::gui::design_system::typography::{BODY, CAPTION};
use crate::gui::design_system::{Tokens, font_size, line_height, radius, space, tokens};
use crate::gui::state::toast::{Kind, Toast};

const MIN_WIDTH: f32 = 300.0;
const MAX_WIDTH: f32 = 500.0;
const BORDER_WIDTH: f32 = 1.0;
const MARKER_WIDTH: f32 = 3.0;
const ICON_SIZE: f32 = 16.0;
const CLOSE_SIZE: f32 = 24.0;
const CLOSE_OFFSET: f32 = 3.0;
const ICON_OFFSET: f32 = 1.0;
const DETAIL_GAP: f32 = 2.0;
const TEXT_END: f32 = 4.0;

static SUCCESS: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../assets/icons/toast/success.svg"))
});
static WARNING: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../assets/icons/toast/warning.svg"))
});
static ERROR: LazyLock<svg::Handle> = LazyLock::new(|| {
    svg::Handle::from_memory(include_bytes!("../../../assets/icons/toast/error.svg"))
});

fn status_color(kind: Kind) -> fn(&Tokens) -> Color {
    match kind {
        Kind::Success => |tokens| tokens.status_success,
        Kind::Warning => |tokens| tokens.status_warning,
        Kind::Error => |tokens| tokens.status_danger,
    }
}

pub(in crate::gui) fn toast<'a, Message: Clone + 'a>(
    toast: &'a Toast,
    on_close: Message,
) -> Element<'a, Message> {
    let kind = toast.kind;
    let handle = match kind {
        Kind::Success => &SUCCESS,
        Kind::Warning => &WARNING,
        Kind::Error => &ERROR,
    };
    let icon = svg(LazyLock::force(handle).clone())
        .width(ICON_SIZE)
        .height(ICON_SIZE)
        .style(move |theme, _| svg::Style {
            color: Some(status_color(kind)(tokens(theme))),
        });
    let message = BODY.text(&toast.text).style(|theme| text::Style {
        color: Some(tokens(theme).text_primary),
    });
    let detail = toast.detail.as_deref().map(|detail| {
        container(CAPTION.text(detail).style(|theme| text::Style {
            color: Some(tokens(theme).text_secondary),
        }))
        .padding(Padding::ZERO.top(DETAIL_GAP))
    });
    let content = row![
        container(icon).padding(Padding::ZERO.top(CLOSE_OFFSET + ICON_OFFSET)),
        column![message]
            .push(detail)
            .padding(Padding::ZERO.top(CLOSE_OFFSET).right(TEXT_END)),
    ]
    .spacing(space::S100);
    let body = container(column![
        Space::new().width(MIN_WIDTH),
        container(content).padding(Padding {
            top: BORDER_WIDTH + space::S150 - CLOSE_OFFSET,
            right: BORDER_WIDTH + space::S100 + CLOSE_SIZE + space::S100,
            bottom: BORDER_WIDTH + space::S150,
            left: MARKER_WIDTH + space::S150,
        }),
    ])
    .max_width(MAX_WIDTH)
    .style(|theme| {
        let tokens = tokens(theme);
        container::Style::default()
            .background(tokens.bg_raised)
            .border(Border {
                color: tokens.border_default,
                width: BORDER_WIDTH,
                radius: radius::MD.into(),
            })
            .shadow(tokens.shadow_lg)
    });
    let marker = edge_marker(MARKER_WIDTH, radius::MD, status_color(kind));
    let close = container(close_button(on_close))
        .align_right(Length::Fill)
        .padding(
            Padding::ZERO
                .top(BORDER_WIDTH + space::S150 - CLOSE_OFFSET)
                .right(BORDER_WIDTH + space::S100),
        );
    stack![body, marker, close].into()
}

fn close_button<'a, Message: Clone + 'a>(on_close: Message) -> Element<'a, Message> {
    let glyph = text("×")
        .size(font_size::XL)
        .line_height(line_height::SNUG)
        .center();
    button(container(glyph).center(CLOSE_SIZE))
        .padding(0)
        .on_press(on_close)
        .style(ghost_style)
        .into()
}
