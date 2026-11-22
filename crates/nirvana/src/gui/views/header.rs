use std::sync::LazyLock;

use iced::alignment::Vertical;
use iced::widget::text::LineHeight;
use iced::widget::{column, container, mouse_area, row, rule, space, stack, svg, text};
use iced::{Border, Color, Element, padding};

use super::border;
use crate::gui::app::Message;
use crate::gui::components::{Button, Size as ButtonSize, dashed_border};
use crate::gui::design_system::font::INTER_BOLD;
use crate::gui::design_system::{Tokens, radius, tokens};
use crate::gui::state::profile::{Health, Profile, ProfileState};
use crate::gui::state::{info, settings};

static LOGO: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../assets/logo.svg")));

pub(super) fn header<'a>(
    profile: Option<&'a ProfileState>,
    settings: &settings::State,
) -> Element<'a, Message> {
    let logo = mouse_area(
        svg(LOGO.clone())
            .width(24)
            .height(24)
            .style(|theme, _| svg::Style {
                color: Some(tokens(theme).brand),
            }),
    )
    .on_press(Message::Info(info::Message::LogoClicked));
    let wordmark = text("nirvana")
        .font(INTER_BOLD)
        .size(16)
        .line_height(LineHeight::Relative(1.25))
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_primary),
        });

    let chip = profile.map(|profile| match profile {
        ProfileState::Missing => no_profile_chip(),
        ProfileState::Active(profile) => profile_chip(profile),
    });

    let open_web = Button::new("open web ↗").size(ButtonSize::Sm).key("o");
    let settings_label = if settings.shown().is_some() {
        "done"
    } else {
        "settings"
    };
    let settings_button = Button::new(settings_label).size(ButtonSize::Sm).key(",");
    let settings_button = if settings.is_available() {
        settings_button.on_press(Message::Settings(settings::Message::Toggle))
    } else {
        settings_button
    };

    let bar = container(
        row![logo, wordmark]
            .push(chip)
            .push(space::horizontal())
            .push(open_web)
            .push(settings_button)
            .spacing(12)
            .align_y(Vertical::Center),
    )
    .height(52)
    .padding([0, 16])
    .align_y(Vertical::Center);

    column![bar, border(rule::horizontal(1))].into()
}

fn no_profile_chip<'a>() -> Element<'a, Message> {
    let label = text("no profile yet")
        .size(12)
        .line_height(LineHeight::Relative(1.5))
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_tertiary),
        });
    let chip = stack![
        container(label).padding([4, 8]),
        dashed_border(radius::FULL),
    ];

    container(chip).padding(padding::left(4)).into()
}

fn profile_chip(profile: &Profile) -> Element<'_, Message> {
    let health = profile.health();
    let has_connections = profile.has_connections();
    let dot = container(space().width(7).height(7)).style(move |theme| {
        let color = health_color(tokens(theme), health);
        let ring = container::Style::default().border(Border {
            color,
            width: 1.0,
            radius: 9999.into(),
        });
        if has_connections {
            ring.background(color)
        } else {
            ring
        }
    });
    let name = text(&profile.name)
        .size(12)
        .line_height(LineHeight::Relative(1.5))
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_secondary),
        });
    let chip = container(row![dot, name].spacing(6).align_y(Vertical::Center))
        .padding([4, 8])
        .style(|theme| {
            container::Style::default().border(Border {
                color: tokens(theme).border_default,
                width: 1.0,
                radius: 9999.into(),
            })
        });

    container(chip).padding(padding::left(4)).into()
}

fn health_color(tokens: &Tokens, health: Option<Health>) -> Color {
    match health {
        None => tokens.fill_neutral_strong,
        Some(Health::Healthy) => tokens.status_success,
        Some(Health::Degraded) => tokens.status_warning,
        Some(Health::Down) => tokens.status_danger,
    }
}
