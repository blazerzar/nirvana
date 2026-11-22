use iced::border::Radius;
use iced::widget::{column, container, row, space, text};
use iced::{Border, Color, Element, Length, Padding, Theme};

use super::{done_bar, screen_heading};
use crate::gui::app::{Message, Screen};
use crate::gui::components::tracked_text;
use crate::gui::design_system::typography::{CAPTION, LABEL_CAPS, MICRO, STAT};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::overview::{Stat, Total};
use crate::time::format_duration;

const BAR_HEIGHT: f32 = 6.0;
const BAR_BORDER: f32 = 1.0;
const SEGMENT_RADIUS: f32 = radius::SM - BAR_BORDER;

pub(in crate::gui) fn overview_view(
    profile: &str,
    stats: Option<[Stat; 3]>,
) -> Element<'_, Message> {
    let heading = screen_heading(
        "overview",
        format!("profile {profile} · includes the running slot"),
    )
    .padding(Padding::from([spacing::S200, spacing::S300]).bottom(spacing::S050));
    let cards = stats.map(|stats| {
        row(stats.into_iter().map(card))
            .spacing(spacing::S150)
            .padding(Padding::from([spacing::S0, spacing::S300]).top(spacing::S150))
    });

    column![heading].push(cards).height(Length::Fill).into()
}

pub(in crate::gui) fn overview_bar<'a>() -> Element<'a, Message> {
    done_bar(
        "per-ticket breakdown and month view are in",
        Screen::Overview,
    )
}

fn card<'a>(stat: Stat) -> Element<'a, Message> {
    let label = row![
        tracked_text(LABEL_CAPS, stat.label, tertiary),
        space::horizontal(),
        MICRO.text(stat.period).style(tertiary),
    ];
    let total = STAT
        .text(format_duration(stat.total.total()))
        .style(|theme| text::Style {
            color: Some(tokens(theme).text_primary),
        });
    let status = if stat.total.unpublished > 0 {
        row![
            CAPTION
                .text("●")
                .style(|theme| colored(theme, |tokens| tokens.accent)),
            CAPTION
                .text(format!(
                    "{} unpublished",
                    format_duration(stat.total.unpublished)
                ))
                .style(tertiary),
        ]
    } else {
        row![
            CAPTION
                .text("✓")
                .style(|theme| colored(theme, |tokens| tokens.status_success)),
            CAPTION.text("published").style(tertiary),
        ]
    };
    let footer = row![
        CAPTION
            .text(format!("of {}", format_duration(stat.target)))
            .style(tertiary),
        space::horizontal(),
        status.spacing(spacing::S050),
    ];

    let content = column![
        column![label, total, bar(stat.total, stat.target)].spacing(spacing::S100),
        footer
    ]
    .spacing(spacing::S075);
    container(content)
        .width(Length::Fill)
        .padding([spacing::S150, spacing::S200])
        .style(|theme| {
            let tokens = tokens(theme);
            container::background(tokens.bg_raised).border(Border {
                color: tokens.border_default,
                width: 1.0,
                radius: radius::MD.into(),
            })
        })
        .into()
}

/// The published and then the unpublished share of `target`, filling at most
/// the whole bar.
fn bar<'a>(total: Total, target: i64) -> Element<'a, Message> {
    let [published, unpublished, rest] = portions(total, target);
    let segment = |portion: u16, color: fn(&Tokens) -> Color, first: bool, last: bool| {
        let mut corners = Radius::default();
        if first {
            corners = corners.left(SEGMENT_RADIUS);
        }
        if last {
            corners = corners.right(SEGMENT_RADIUS);
        }
        (portion > 0).then(|| {
            container(space())
                .width(Length::FillPortion(portion))
                .height(Length::Fill)
                .style(move |theme| {
                    container::background(color(tokens(theme))).border(Border {
                        radius: corners,
                        ..Border::default()
                    })
                })
        })
    };
    let segments = row![]
        .push(segment(
            published,
            |tokens| tokens.fill_neutral_strong,
            true,
            unpublished == 0 && rest == 0,
        ))
        .push(segment(
            unpublished,
            |tokens| tokens.accent,
            published == 0,
            rest == 0,
        ))
        .push((rest > 0).then(|| space().width(Length::FillPortion(rest))));
    container(segments)
        .width(Length::Fill)
        .height(BAR_HEIGHT)
        .padding(BAR_BORDER)
        .style(|theme| {
            let tokens = tokens(theme);
            container::background(tokens.bg_default).border(Border {
                color: tokens.border_default,
                width: BAR_BORDER,
                radius: radius::SM.into(),
            })
        })
        .into()
}

/// Returns the minutes of the published, the unpublished and the remaining
/// part of `target`, with the published part first and nothing past
/// `target`.
fn portions(total: Total, target: i64) -> [u16; 3] {
    let target = target.max(0) / 60;
    let published = (total.published / 60).clamp(0, target);
    let unpublished = (total.unpublished / 60).clamp(0, target - published);
    [published, unpublished, target - published - unpublished]
        .map(|minutes| u16::try_from(minutes).unwrap_or(u16::MAX))
}

fn colored(theme: &Theme, color: fn(&Tokens) -> Color) -> text::Style {
    text::Style {
        color: Some(color(tokens(theme))),
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    colored(theme, |tokens| tokens.text_tertiary)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(published: i64, unpublished: i64) -> Total {
        Total {
            published,
            unpublished,
        }
    }

    #[test]
    fn portions_split_the_target_in_minutes() {
        assert_eq!(portions(total(3600, 1800), 8 * 3600), [60, 30, 390]);
    }

    #[test]
    fn portions_stop_at_the_target() {
        assert_eq!(portions(total(3600, 9 * 3600), 8 * 3600), [60, 420, 0]);
        assert_eq!(portions(total(9 * 3600, 3600), 8 * 3600), [480, 0, 0]);
    }
}
