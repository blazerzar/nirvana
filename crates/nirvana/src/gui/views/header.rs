use std::cell::Cell;
use std::sync::LazyLock;

use iced::alignment::Vertical;
use iced::widget::canvas::{self, LineDash, Path, Stroke};
use iced::widget::text::LineHeight;
use iced::widget::{column, container, mouse_area, row, rule, space, stack, svg, text};
use iced::{Border, Color, Element, Length, Point, Rectangle, Renderer, Size, mouse, padding};

use super::border;
use crate::gui::app::Message;
use crate::gui::design_system::font::INTER_BOLD;
use crate::gui::design_system::{Tokens, tokens};
use crate::gui::info;
use crate::gui::profile::{Health, Profile, ProfileState};

static LOGO: LazyLock<svg::Handle> =
    LazyLock::new(|| svg::Handle::from_memory(include_bytes!("../../../assets/logo.svg")));

pub(super) fn header(profile: Option<&ProfileState>) -> Element<'_, Message> {
    let logo = mouse_area(svg(LOGO.clone()).width(24).height(24))
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

    let bar = container(
        row![logo, wordmark]
            .push(chip)
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
        canvas::Canvas::new(DashedBorder)
            .width(Length::Fill)
            .height(Length::Fill),
    ];

    container(chip).padding(padding::left(4)).into()
}

fn profile_chip(profile: &Profile) -> Element<'_, Message> {
    let health = profile.health();
    let dot = container(space().width(7).height(7)).style(move |theme| {
        container::Style::default()
            .background(health_color(tokens(theme), health))
            .border(Border::default().rounded(9999))
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

struct DashedBorder;

#[derive(Default)]
struct DashedBorderCache {
    cache: canvas::Cache,
    color: Cell<Option<Color>>,
}

impl canvas::Program<Message> for DashedBorder {
    type State = DashedBorderCache;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let color = tokens(theme).border_default;
        if state.color.replace(Some(color)) != Some(color) {
            state.cache.clear();
        }
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let border = Path::rounded_rectangle(
                Point::new(0.5, 0.5),
                Size::new(bounds.width - 1.0, bounds.height - 1.0),
                (bounds.height / 2.0).into(),
            );
            let stroke = Stroke {
                line_dash: LineDash {
                    segments: &[3.0, 3.0],
                    offset: 0,
                },
                ..Stroke::default().with_color(color).with_width(1.0)
            };
            frame.stroke(&border, stroke);
        });
        vec![geometry]
    }
}
