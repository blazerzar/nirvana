use iced::widget::text::IntoFragment;
use iced::widget::{Space, button, column, container, row, text};
use iced::{Border, Color, Element, Length, Padding};

use crate::gui::design_system::typography::CAPTION;
use crate::gui::design_system::{radius, space, tokens};

const BORDER_WIDTH: f32 = 1.0;
const DOT_SIZE: f32 = 14.0;
const DOT_BORDER_WIDTH: f32 = 1.5;
const DOT_INNER_SIZE: f32 = 6.0;
const DOT_OFFSET: f32 = 3.0;

/// One option of a radio group: a dot, `label` and `caption`. The selected
/// option shows `child` below them.
pub(in crate::gui) fn radio_option<'a, Message: Clone + 'a>(
    label: impl Into<Element<'a, Message>>,
    caption: impl IntoFragment<'a>,
    is_on: bool,
    on_press: Message,
    child: Option<Element<'a, Message>>,
) -> Element<'a, Message> {
    let header = row![
        container(dot(is_on)).padding(Padding::ZERO.top(DOT_OFFSET)),
        column![
            label.into(),
            CAPTION.text(caption).style(|theme| text::Style {
                color: Some(tokens(theme).text_tertiary),
            }),
        ],
    ]
    .spacing(space::S100);
    let header = button(header)
        .width(Length::Fill)
        .padding([space::S100 - BORDER_WIDTH, space::S150 - BORDER_WIDTH])
        .on_press(on_press)
        .style(move |theme, status| {
            let tokens = tokens(theme);
            let background = match status {
                _ if is_on => Color::TRANSPARENT,
                button::Status::Hovered => tokens.fill_hover,
                button::Status::Pressed => tokens.fill_pressed,
                _ => Color::TRANSPARENT,
            };
            button::Style {
                background: Some(background.into()),
                text_color: tokens.text_primary,
                border: Border::default().rounded(radius::MD - BORDER_WIDTH),
                ..button::Style::default()
            }
        });
    let indent = space::S150 + DOT_SIZE + space::S100;
    let child = child.filter(|_| is_on).map(|child| {
        container(child).padding(Padding {
            top: space::S0,
            right: space::S150 - BORDER_WIDTH,
            bottom: space::S150 - BORDER_WIDTH,
            left: indent - BORDER_WIDTH,
        })
    });
    container(column![header].push(child))
        .width(Length::Fill)
        .padding(BORDER_WIDTH)
        .style(move |theme| {
            let tokens = tokens(theme);
            let (background, border) = if is_on {
                (tokens.fill_accent_subtle, tokens.border_accent)
            } else {
                (tokens.bg_raised, tokens.border_default)
            };
            container::Style::default()
                .background(background)
                .border(Border {
                    color: border,
                    width: BORDER_WIDTH,
                    radius: radius::MD.into(),
                })
        })
        .into()
}

fn dot<'a, Message: 'a>(is_on: bool) -> Element<'a, Message> {
    let inner: Element<'a, Message> = if is_on {
        container(Space::new())
            .width(DOT_INNER_SIZE)
            .height(DOT_INNER_SIZE)
            .style(|theme| {
                container::Style::default()
                    .background(tokens(theme).accent)
                    .border(Border::default().rounded(radius::FULL))
            })
            .into()
    } else {
        Space::new().into()
    };
    container(inner)
        .center(DOT_SIZE)
        .style(move |theme| {
            let tokens = tokens(theme);
            let color = if is_on {
                tokens.accent
            } else {
                tokens.fill_neutral_strong
            };
            container::Style::default().border(Border {
                color,
                width: DOT_BORDER_WIDTH,
                radius: radius::FULL.into(),
            })
        })
        .into()
}
