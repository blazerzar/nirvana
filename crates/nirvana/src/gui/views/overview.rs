use iced::border::Radius;
use iced::widget::{Container, column, container, responsive, row, space, stack, text};
use iced::{Border, Color, Element, Length, Padding, Pixels, Theme};

use super::{done_bar, screen_heading};
use crate::gui::app::{Message, Screen};
use crate::gui::components::{target_line, tracked_text};
use crate::gui::design_system::typography::{CAPTION, CAPTION_STRONG, LABEL_CAPS, MICRO, STAT};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::overview::{Day, EXPECTED_SECONDS_PER_DAY, Stat, Total};
use crate::time::format_duration;

const BAR_HEIGHT: f32 = 6.0;
const BAR_BORDER: f32 = 1.0;
/// Round ends, the most the renderer draws on a segment inside the border.
const SEGMENT_RADIUS: f32 = (BAR_HEIGHT - 2.0 * BAR_BORDER) / 2.0;
/// The chart is at least this tall in seconds, so the expected day fits.
const CHART_MIN_SECONDS: i64 = 9 * 3600;
const DAY_LABEL_HEIGHT: f32 = 20.0;
const SWATCH_SIZE: f32 = 8.0;

pub(in crate::gui) fn overview_view(
    profile: &str,
    stats: Option<[Stat; 3]>,
    week: Option<[Day; 7]>,
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

    column![heading]
        .push(cards)
        .push(week.map(week_chart))
        .height(Length::Fill)
        .into()
}

pub(in crate::gui) fn overview_bar<'a>(web_available: bool) -> Element<'a, Message> {
    done_bar(
        "per-ticket breakdown and month view are in",
        Screen::Overview,
        web_available,
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
    let filled = (published + unpublished > 0).then(|| {
        let end = if rest == 0 { SEGMENT_RADIUS } else { 0.0 };
        split_bar(
            Axis::Horizontal,
            (published, |tokens| tokens.fill_neutral_strong),
            (unpublished, |tokens| tokens.accent),
            SEGMENT_RADIUS,
            end,
        )
        .width(Length::FillPortion(published + unpublished))
    });
    let segments = row![]
        .push(filled)
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

fn week_chart<'a>(days: [Day; 7]) -> Element<'a, Message> {
    let legend = row![
        swatch(|tokens| tokens.fill_neutral_strong),
        MICRO.text("published").style(tertiary),
        space().width(spacing::S150),
        swatch(|tokens| tokens.accent),
        MICRO.text("unpublished").style(tertiary),
    ]
    .spacing(spacing::S050)
    .align_y(iced::alignment::Vertical::Center);
    let heading = row![
        tracked_text(LABEL_CAPS, "THIS WEEK BY DAY", tertiary),
        space::horizontal(),
        legend,
    ];

    let max = days
        .iter()
        .map(|day| day.total.total())
        .max()
        .unwrap_or_default()
        .max(CHART_MIN_SECONDS);
    let bars = row(days.iter().map(|day| day_bar(day.total, max))).spacing(spacing::S150);
    let value_height = CAPTION.line_height.to_absolute(Pixels(CAPTION.size)).0 + spacing::S050;
    let line = target_line(
        EXPECTED_SECONDS_PER_DAY as f32 / max as f32,
        value_height,
        format_duration(EXPECTED_SECONDS_PER_DAY),
    );
    let labels = row(days.into_iter().map(day_label)).spacing(spacing::S150);
    let chart = column![stack![bars, line].height(Length::Fill), labels];

    column![heading, chart]
        .spacing(spacing::S100)
        .height(Length::Fill)
        .padding(Padding::from([spacing::S200, spacing::S300]).bottom(spacing::S150))
        .into()
}

/// The total of a day above its bar, unpublished on top of published, with
/// the bar's height as its share of `max`.
fn day_bar<'a>(total: Total, max: i64) -> Element<'a, Message> {
    let max = max / 60;
    let minutes = |seconds: i64| (seconds / 60).clamp(0, max);
    let [rest, published, unpublished] = [
        max - minutes(total.total()),
        minutes(total.published),
        minutes(total.unpublished),
    ]
    .map(|minutes| u16::try_from(minutes).unwrap_or(u16::MAX));
    let value = if total.total() > 0 {
        CAPTION.text(format_duration(total.total())).style(primary)
    } else {
        CAPTION
            .text("—")
            .style(|theme| colored(theme, |tokens| tokens.text_disabled))
    };
    let bar = (published + unpublished > 0).then(|| {
        split_bar(
            Axis::Vertical,
            (unpublished, |tokens| tokens.accent),
            (published, |tokens| tokens.fill_neutral_strong),
            radius::SM,
            radius::SM,
        )
        .height(Length::FillPortion(published + unpublished))
    });

    column![]
        .push((rest > 0).then(|| space().height(Length::FillPortion(rest))))
        .push(
            container(value)
                .center_x(Length::Fill)
                .padding(Padding::ZERO.bottom(spacing::S050)),
        )
        .push(bar)
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

#[derive(Clone, Copy)]
enum Axis {
    Horizontal,
    Vertical,
}

/// A segment's portion of its bar and its color.
type Part = (u16, fn(&Tokens) -> Color);

/// A bar of a `first` and a `second` segment along `axis`, with
/// `start_radius` and `end_radius` corners at its ends. The whole bar is
/// drawn in the color of the shorter segment and the longer segment is laid
/// over its end, so no background shows between the segments and the end of
/// the shorter segment keeps its full corners. The longer segment's inner
/// corners take the part of the radius the shorter segment is too short to
/// cover, so it stays inside the bar's rounded outline. The renderer limits a
/// corner to half of the shorter side of its quad.
fn split_bar<'a>(
    axis: Axis,
    first: Part,
    second: Part,
    start_radius: f32,
    end_radius: f32,
) -> Container<'a, Message> {
    let corners = move |start: f32, end: f32| match axis {
        Axis::Horizontal => Radius::default().left(start).right(end),
        Axis::Vertical => Radius::default().top(start).bottom(end),
    };
    let fill = move |color: fn(&Tokens) -> Color, corners: Radius| {
        container(space())
            .width(Length::Fill)
            .height(Length::Fill)
            .style(move |theme| {
                container::background(color(tokens(theme))).border(Border {
                    radius: corners,
                    ..Border::default()
                })
            })
    };
    let along = move |piece: Container<'a, Message>, portion: u16| match axis {
        Axis::Horizontal => piece.width(Length::FillPortion(portion)),
        Axis::Vertical => piece.height(Length::FillPortion(portion)),
    };

    let first_is_longer = first.0 >= second.0;
    let (longer, shorter) = if first_is_longer {
        (first, second)
    } else {
        (second, first)
    };
    container(responsive(move |size| {
        let length = match axis {
            Axis::Horizontal => size.width,
            Axis::Vertical => size.height,
        };
        let shorter_length = length * f32::from(shorter.0) / f32::from(first.0 + second.0);
        let shared_radius = if first_is_longer {
            end_radius
        } else {
            start_radius
        };
        let inner_radius = (shared_radius - shorter_length).max(0.0);
        let (start, end) = match (shorter.0, first_is_longer) {
            (0, _) => (start_radius, end_radius),
            (_, true) => (start_radius, inner_radius),
            (_, false) => (inner_radius, end_radius),
        };
        let longer = Some(along(fill(longer.1, corners(start, end)), longer.0));
        let gap = (shorter.0 > 0).then(|| along(container(space()), shorter.0));
        let (head, tail) = if first_is_longer {
            (longer, gap)
        } else {
            (gap, longer)
        };
        let over: Element<'a, Message> = match axis {
            Axis::Horizontal => row![].push(head).push(tail).into(),
            Axis::Vertical => column![].push(head).push(tail).into(),
        };
        let under = fill(shorter.1, corners(start_radius, end_radius));

        stack![under, over].into()
    }))
    .width(Length::Fill)
    .height(Length::Fill)
}

fn day_label<'a>(day: Day) -> Element<'a, Message> {
    let label = if day.is_today {
        CAPTION_STRONG.text(day.label).style(primary)
    } else {
        CAPTION.text(day.label).style(tertiary)
    };
    container(label)
        .center_x(Length::Fill)
        .height(DAY_LABEL_HEIGHT)
        .padding(Padding::ZERO.top(spacing::S075))
        .into()
}

fn swatch<'a>(color: fn(&Tokens) -> Color) -> Element<'a, Message> {
    container(space())
        .width(SWATCH_SIZE)
        .height(SWATCH_SIZE)
        .style(move |theme| {
            container::background(color(tokens(theme))).border(Border {
                radius: radius::XS.into(),
                ..Border::default()
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

fn primary(theme: &Theme) -> text::Style {
    colored(theme, |tokens| tokens.text_primary)
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
