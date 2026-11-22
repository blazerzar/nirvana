use iced::alignment::Vertical;
use iced::widget::text::Wrapping;
use iced::widget::text_input::{self, Icon, Side};
use iced::widget::{Column, Space, button, column, container, row, space, text};
use iced::{Border, Color, Element, Length, Theme};

use crate::gui::app::Message;
use crate::gui::components::{Button, OnEscape, RETURN_KEY, Size, select, tracked_text};
use crate::gui::design_system::font::MONO;
use crate::gui::design_system::typography::{BODY, BODY_LG, BODY_STRONG, CAPTION, LABEL_CAPS};
use crate::gui::design_system::{radius, space as spacing, tokens};
use crate::gui::state::start::{self, Recent, TICKET_INPUT};

const INPUT_HEIGHT: f32 = 36.0;
const RECENT_KEY_WIDTH: f32 = 64.0;
const RECENT_BORDER_WIDTH: f32 = 1.0;

pub(super) fn start(state: &start::State) -> Element<'_, Message> {
    let text_height = BODY_LG.line_height.to_absolute(BODY_LG.size.into()).0;
    let input = text_input::TextInput::new("e.g. NIR-12", state.key())
        .id(TICKET_INPUT)
        .on_input(|key| Message::Start(start::Message::KeyChanged(key)))
        .on_submit(Message::Start(start::Message::Start))
        .icon(Icon {
            font: MONO,
            code_point: '›',
            size: Some(BODY_LG.size.into()),
            spacing: spacing::S075,
            side: Side::Left,
        })
        .font(BODY_LG.font)
        .size(BODY_LG.size)
        .line_height(BODY_LG.line_height)
        .padding([(INPUT_HEIGHT - text_height) / 2.0, spacing::S150])
        .style(input_style);
    let input = OnEscape::new(input, Message::Start(start::Message::Cancel));
    let connection = state.connection_choices().map(|choices| {
        let connection = select(
            Size::Lg,
            choices,
            state.connection().clone(),
            state.is_connection_open(),
            Some(Message::Start(start::Message::ToggleConnection)),
            |choice| Message::Start(start::Message::Connection(choice)),
            Message::Start(start::Message::CloseConnection),
        );
        column![field_label("connection"), connection].spacing(spacing::S050)
    });
    let ticket = row![
        column![field_label("ticket"), input]
            .spacing(spacing::S050)
            .width(Length::Fill)
    ]
    .push(connection)
    .spacing(spacing::S100);

    let recent = Column::with_children(
        state
            .recent()
            .iter()
            .map(|recent| recent_row(recent, recent.key.eq_ignore_ascii_case(state.key().trim()))),
    )
    .spacing(spacing::S025);

    let has_key = !state.key().trim().is_empty();
    let mut start_button = Button::new("start").key(RETURN_KEY).primary();
    if has_key && !state.is_starting() {
        start_button = start_button.on_press(Message::Start(start::Message::Start));
    }
    let actions = row![
        space::horizontal(),
        Button::new("cancel")
            .key("esc")
            .on_press(Message::Start(start::Message::Cancel)),
        start_button,
    ]
    .spacing(spacing::S100);

    let content = column![
        tracked_text(LABEL_CAPS, "START TRACKING", tertiary),
        Space::new().height(spacing::S150),
        ticket,
        Space::new().height(spacing::S150),
        field_label("recent"),
        Space::new().height(spacing::S050),
        recent,
        space::vertical(),
        actions,
    ];
    container(content)
        .padding(spacing::S300)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

fn field_label(label: &str) -> Element<'_, Message> {
    CAPTION.text(label).style(tertiary).into()
}

fn recent_row(recent: &Recent, is_selected: bool) -> Element<'_, Message> {
    let key = container(
        BODY_STRONG
            .text(&recent.key)
            .wrapping(Wrapping::None)
            .style(move |theme| text::Style {
                color: Some(if is_selected {
                    tokens(theme).text_accent
                } else {
                    tokens(theme).text_primary
                }),
            }),
    )
    .width(RECENT_KEY_WIDTH);
    let title = BODY
        .text(&recent.title)
        .wrapping(Wrapping::None)
        .style(move |theme| text::Style {
            color: Some(if is_selected {
                tokens(theme).text_primary
            } else {
                tokens(theme).text_secondary
            }),
        });
    button(
        row![key, container(title).clip(true)]
            .spacing(spacing::S100)
            .align_y(Vertical::Center),
    )
    .width(Length::Fill)
    .padding([spacing::S075 + RECENT_BORDER_WIDTH, spacing::S100])
    .on_press(Message::Start(start::Message::Recent(recent.clone())))
    .style(move |theme, status| recent_style(theme, status, is_selected))
    .into()
}

fn recent_style(theme: &Theme, status: button::Status, is_selected: bool) -> button::Style {
    let tokens = tokens(theme);
    let (background, border) = match status {
        _ if is_selected => (tokens.fill_accent_subtle, tokens.border_accent),
        button::Status::Hovered => (tokens.fill_hover, Color::TRANSPARENT),
        button::Status::Pressed => (tokens.fill_pressed, Color::TRANSPARENT),
        _ => (Color::TRANSPARENT, Color::TRANSPARENT),
    };
    button::Style {
        background: Some(background.into()),
        border: Border {
            color: border,
            width: RECENT_BORDER_WIDTH,
            radius: radius::SM.into(),
        },
        ..button::Style::default()
    }
}

fn input_style(theme: &Theme, status: text_input::Status) -> text_input::Style {
    let tokens = tokens(theme);
    let border = match status {
        text_input::Status::Focused { .. } => tokens.border_focus,
        _ => tokens.border_default,
    };
    text_input::Style {
        background: tokens.bg_raised.into(),
        border: Border {
            color: border,
            width: 1.0,
            radius: radius::SM.into(),
        },
        icon: tokens.text_accent,
        placeholder: tokens.text_disabled,
        value: tokens.text_primary,
        selection: tokens.fill_accent_subtle,
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
