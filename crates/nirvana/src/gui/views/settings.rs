use iced::alignment::Vertical;
use iced::widget::{Column, column, container, row, rule, text, text_input};
use iced::{Color, Element, Length, Padding, Theme};
use nirvana_core::{IdleConfig, IdleMethod};

use super::start::input_style;
use super::{border, done_bar, screen_heading};
use crate::gui::app::{Message, Screen};
use crate::gui::components::{Size, select, toggle, toggle_chip, tracked_text};
use crate::gui::design_system::typography::{BODY, BODY_LG, CAPTION, LABEL_CAPS};
use crate::gui::design_system::{Tokens, space, tokens};
use crate::gui::state::settings::{self, IdleAfter, ServerStatus};

const COLUMN_GAP: f32 = 32.0;
const PORT_INPUT_WIDTH: f32 = 72.0;

pub(in crate::gui) fn settings_view<'a>(
    idle: &'a IdleConfig,
    settings: &'a settings::State,
) -> Element<'a, Message> {
    let heading = screen_heading("desktop settings", "this machine only")
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
                Some(caption("ask what happened when you return", true)),
                true,
                toggle(
                    idle.enabled,
                    Some(|enabled| Message::Settings(settings::Message::IdleEnabled(enabled))),
                ),
            ),
            setting(
                "idle after",
                None,
                idle.enabled,
                select(
                    Size::Sm,
                    IdleAfter::options(idle_after),
                    idle_after,
                    settings.is_idle_after_open(),
                    idle.enabled
                        .then_some(Message::Settings(settings::Message::ToggleIdleAfter)),
                    |after| Message::Settings(settings::Message::IdleAfter(after)),
                    Message::Settings(settings::Message::CloseIdleAfter),
                ),
            ),
            setting(
                "detect by",
                Some(caption("any selected method counts as idle", idle.enabled)),
                idle.enabled,
                methods,
            ),
        ],
    );
    let server_status = match settings.server_status() {
        ServerStatus::Starting => status(|tokens| tokens.text_tertiary, "starting"),
        ServerStatus::Running(_) => status(|tokens| tokens.status_success, "running"),
        ServerStatus::Off => caption("nirvana web is unavailable while off", true),
        ServerStatus::Failed(error) => status(|tokens| tokens.status_danger, error),
    };
    let port = text_input("", settings.port_input())
        .on_input(|input| Message::Settings(settings::Message::PortInput(input)))
        .font(BODY.font)
        .size(BODY.size)
        .line_height(BODY.line_height)
        .padding([space::S050, space::S100])
        .width(PORT_INPUT_WIDTH)
        .style(input_style);
    let server = section(
        "LOCAL WEB SERVER",
        [
            setting(
                "run web server",
                Some(server_status),
                true,
                toggle(
                    settings.is_server_enabled(),
                    (!matches!(settings.server_status(), ServerStatus::Starting)).then_some(
                        |enabled| Message::Settings(settings::Message::ServerEnabled(enabled)),
                    ),
                ),
            ),
            setting(
                "port",
                Some(caption("takes effect when the server restarts", true)),
                true,
                port,
            ),
        ],
    );
    let columns = row![
        container(tracking).width(Length::Fill),
        container(server).width(Length::Fill)
    ]
    .spacing(COLUMN_GAP)
    .padding([space::S100, space::S300]);

    column![heading, columns].height(Length::Fill).into()
}

pub(in crate::gui) fn settings_bar<'a>(web_available: bool) -> Element<'a, Message> {
    done_bar(
        "profiles, connections and publishing rules are in",
        Screen::Settings,
        web_available,
    )
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
    description: Option<Element<'a, Message>>,
    enabled: bool,
    control: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    let label = column![BODY_LG.text(label).style(move |theme| {
        let tokens = tokens(theme);
        text::Style {
            color: Some(tokens.text_primary.scale_alpha(tokens.opacity(enabled))),
        }
    })]
    .push(description)
    .spacing(space::S025);
    let line = row![container(label).width(Length::Fill), control.into()]
        .spacing(space::S150)
        .align_y(Vertical::Center)
        .padding([space::S100, space::S0]);

    column![line, border(rule::horizontal(1))].into()
}

/// A setting's description, dimmed while `enabled` is false.
fn caption<'a>(description: &'a str, enabled: bool) -> Element<'a, Message> {
    CAPTION
        .text(description)
        .style(move |theme| {
            let tokens = tokens(theme);
            text::Style {
                color: Some(tokens.text_tertiary.scale_alpha(tokens.opacity(enabled))),
            }
        })
        .into()
}

fn method_label(method: IdleMethod) -> &'static str {
    match method {
        IdleMethod::Lock => "lock",
        IdleMethod::Sleep => "sleep",
        IdleMethod::Input => "no input",
    }
}

/// A status caption after a dot in the color `dot` picks.
fn status<'a>(dot: fn(&Tokens) -> Color, description: &'a str) -> Element<'a, Message> {
    row![
        CAPTION.text("●").style(move |theme| text::Style {
            color: Some(dot(tokens(theme))),
        }),
        caption(description, true)
    ]
    .spacing(space::S050)
    .into()
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
