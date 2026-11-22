use iced::alignment::Vertical;
use iced::widget::{Column, column, container, row, rule, text};
use iced::{Element, Length, Padding, Theme};
use nirvana_core::{IdleConfig, IdleMethod};

use super::border;
use crate::gui::app::Message;
use crate::gui::components::{Button, Size, select, toggle, toggle_chip, tracked_text};
use crate::gui::design_system::typography::{BODY, BODY_LG, CAPTION, HEADING_SCREEN, LABEL_CAPS};
use crate::gui::design_system::{DISABLED_OPACITY, space, tokens};
use crate::gui::settings::{self, IdleAfter};

const COLUMN_GAP: f32 = 32.0;
const BAR_HEIGHT: f32 = 52.0;

pub(super) fn settings_view(idle: &IdleConfig, idle_after_open: bool) -> Element<'_, Message> {
    let heading = row![
        HEADING_SCREEN.text("desktop settings").style(primary),
        BODY.text("this machine only").style(tertiary),
    ]
    .spacing(space::S150)
    .align_y(Vertical::Bottom)
    .padding(Padding::from([space::S200, space::S300]).bottom(space::S075));

    let idle_after = IdleAfter::of(idle);
    let methods = row(IdleMethod::ALL.into_iter().map(|method| {
        toggle_chip(
            method_label(method),
            idle.methods.contains(&method),
            idle.enabled
                .then_some(Message::Settings(settings::Message::IdleMethod(method))),
        )
    }))
    .spacing(space::S050);
    let tracking = section(
        "TRACKING",
        [
            setting(
                "detect idle time",
                Some("ask what happened when you return"),
                true,
                toggle(idle.enabled, |enabled| {
                    Message::Settings(settings::Message::IdleEnabled(enabled))
                }),
            ),
            setting(
                "idle after",
                None,
                idle.enabled,
                select(
                    IdleAfter::options(idle_after),
                    idle_after,
                    idle_after_open,
                    idle.enabled
                        .then_some(Message::Settings(settings::Message::ToggleIdleAfter)),
                    |after| Message::Settings(settings::Message::IdleAfter(after)),
                    Message::Settings(settings::Message::CloseIdleAfter),
                ),
            ),
            setting(
                "detect by",
                Some("any selected method counts as idle"),
                idle.enabled,
                methods,
            ),
        ],
    );
    let columns = row![
        container(tracking).width(Length::Fill),
        iced::widget::space::horizontal()
    ]
    .spacing(COLUMN_GAP)
    .padding([space::S100, space::S300]);

    column![heading, columns].height(Length::Fill).into()
}

pub(super) fn settings_bar<'a>() -> Element<'a, Message> {
    let note = row![
        BODY.text("profiles, connections and publishing rules are in")
            .style(tertiary),
        BODY.text("nirvana web").style(|theme| text::Style {
            color: Some(tokens(theme).text_accent),
        }),
    ]
    .spacing(space::S075);
    let done = Button::new("done")
        .size(Size::Sm)
        .key("esc")
        .primary()
        .on_press(Message::Settings(settings::Message::Close));
    let bar = container(
        row![note, iced::widget::space::horizontal(), done]
            .spacing(space::S100)
            .align_y(Vertical::Center),
    )
    .height(BAR_HEIGHT)
    .padding(Padding::from([space::S0, space::S200]).left(space::S300))
    .align_y(Vertical::Center);

    column![border(rule::horizontal(1)), bar].into()
}

fn section<'a>(
    title: &str,
    rows: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    column![
        tracked_text(LABEL_CAPS, title, tertiary),
        Column::with_children(rows)
    ]
    .spacing(space::S050)
    .padding(Padding::ZERO.bottom(space::S150))
    .into()
}

/// A labelled control, dimmed while `enabled` is false.
fn setting<'a>(
    label: &'a str,
    description: Option<&'a str>,
    enabled: bool,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let opacity = if enabled { 1.0 } else { DISABLED_OPACITY };
    let label = column![BODY_LG.text(label).style(move |theme| text::Style {
        color: Some(tokens(theme).text_primary.scale_alpha(opacity)),
    })]
    .push(description.map(|description| {
        CAPTION.text(description).style(move |theme| text::Style {
            color: Some(tokens(theme).text_tertiary.scale_alpha(opacity)),
        })
    }))
    .spacing(space::S025);
    let line = row![container(label).width(Length::Fill), control.into()]
        .spacing(space::S150)
        .align_y(Vertical::Center)
        .padding([space::S100, space::S0]);

    column![line, border(rule::horizontal(1))].into()
}

fn method_label(method: IdleMethod) -> &'static str {
    match method {
        IdleMethod::Lock => "lock",
        IdleMethod::Sleep => "sleep",
        IdleMethod::Input => "no input",
    }
}

fn primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_primary),
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
