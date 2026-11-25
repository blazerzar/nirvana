use chrono::NaiveDate;
use iced::alignment::Vertical;
use iced::widget::text::Wrapping;
use iced::widget::{Column, Row, Space, column, container, row, rule, space, stack, text};
use iced::{Border, Color, Element, Length, Padding, Theme};
use nirvana_core::Slot;

use super::border;
use super::listing::days_label;
use crate::gui::app::Message;
use crate::gui::components::{
    Button, RETURN_KEY, Size, edge_marker, ellipsized_text, modal, tracked_text,
};
use crate::gui::design_system::typography::{
    BODY, BODY_LG, CAPTION, LABEL_CAPS, LABEL_STRONG, TITLE_SM,
};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::publish::{self, Phase, Row as TicketRow, Status, slots};
use crate::time::format_duration;

const WIDTH: f32 = 640.0;
const KEY_MIN_WIDTH: f32 = 64.0;
const SLOTS_WIDTH: f32 = 50.0;
const LOGGED_WIDTH: f32 = 76.0;
const STATUS_WIDTH: f32 = 88.0;
const SUBTITLE_GAP: f32 = 2.0;
const ERROR_MARKER_WIDTH: f32 = 2.0;
const NOTE_MARKER_WIDTH: f32 = 3.0;

/// The slots to publish by ticket with their total, then while publishing
/// and after some slots failed, the status of each ticket.
pub(in crate::gui) fn publish_modal<'a>(
    state: &'a publish::State,
    running: Option<&'a Slot>,
    today: Option<NaiveDate>,
) -> Element<'a, Message> {
    let phase = state.phase();
    let summary = state.summary();
    let title = match (phase, summary) {
        (Phase::Sending, _) => "publishing…".to_owned(),
        (Phase::Done, Some(summary)) => summary.toast().1,
        _ => "publish".to_owned(),
    };
    let days = state.days();
    let subtitle = match days.as_slice() {
        [] => slots(state.count()),
        days => format!("{} · {}", slots(state.count()), days_label(days, today)),
    };
    let header = column![
        TITLE_SM
            .text(title)
            .style(colored(|tokens| tokens.text_primary)),
        ellipsized_text(BODY, subtitle, tertiary),
    ]
    .spacing(SUBTITLE_GAP);

    let rows = state.rows();
    let sent_seconds = summary.map_or(0, |summary| summary.seconds);
    let total = match phase {
        Phase::Done => ("sent", sent_seconds),
        Phase::Review | Phase::Sending => ("total", state.seconds()),
    };
    let body = Column::new()
        .push(
            summary
                .filter(|summary| phase == Phase::Done && summary.failed > 0)
                .map(|summary| failed_note(summary.failed)),
        )
        .push(table(&rows, phase, total))
        .push(
            running
                .filter(|_| phase == Phase::Review)
                .map(|slot| running_note(&slot.ticket.ticket_key)),
        )
        .spacing(spacing::S150);

    modal(header, body, footer(state, phase, summary), WIDTH)
}

fn table<'a>(
    rows: &[TicketRow<'a>],
    phase: Phase,
    (total_label, total): (&'a str, i64),
) -> Element<'a, Message> {
    let with_status = phase != Phase::Review;
    let heading = columns(
        tracked_text(LABEL_CAPS, "ticket", tertiary),
        tracked_text(LABEL_CAPS, "slots", tertiary),
        tracked_text(LABEL_CAPS, "logged", tertiary),
        with_status.then(|| tracked_text(LABEL_CAPS, "status", tertiary)),
    );
    let rows = rows.iter().map(|row| ticket_row(row, phase));
    let total = columns(
        LABEL_STRONG
            .text(total_label)
            .style(colored(|tokens| tokens.text_primary))
            .into(),
        space::horizontal().into(),
        LABEL_STRONG
            .text(format_duration(total))
            .wrapping(Wrapping::None)
            .style(colored(|tokens| tokens.text_accent))
            .into(),
        with_status.then(|| space::horizontal().into()),
    );
    column![
        container(heading).padding([spacing::S075, spacing::S0]),
        border(rule::horizontal(1)),
        Column::with_children(rows).padding([spacing::S050, spacing::S0]),
        border(rule::horizontal(1)),
        container(total).padding([spacing::S075, spacing::S0]),
    ]
    .into()
}

/// The ticket column, then the slots and logged columns and the status
/// column, right aligned.
fn columns<'a>(
    ticket: Element<'a, Message>,
    slots: Element<'a, Message>,
    logged: Element<'a, Message>,
    status: Option<Element<'a, Message>>,
) -> Row<'a, Message> {
    let cell = |content, width| container(content).align_right(width);
    row![
        container(ticket).width(Length::Fill),
        cell(slots, SLOTS_WIDTH),
        cell(logged, LOGGED_WIDTH),
    ]
    .push(status.map(|status| cell(status, STATUS_WIDTH)))
    .align_y(Vertical::Center)
}

fn ticket_row<'a>(row: &TicketRow<'a>, phase: Phase) -> Element<'a, Message> {
    let failed = matches!(row.status, Some(Status::Failed(_)));
    let sent = row.status == Some(Status::Sent);
    let key = column![
        Space::new().width(KEY_MIN_WIDTH),
        LABEL_STRONG
            .text(row.key)
            .wrapping(Wrapping::None)
            .style(colored(move |tokens| {
                if failed {
                    tokens.status_danger
                } else {
                    tokens.text_accent
                }
            })),
    ];
    let ticket = row![
        key,
        container(ellipsized_text(BODY, row.title, secondary))
            .clip(true)
            .width(Length::Fill),
    ]
    .spacing(spacing::S100)
    .align_y(Vertical::Center);
    let line = columns(
        ticket.into(),
        BODY_LG.text(row.slots.to_string()).style(secondary).into(),
        BODY_LG
            .text(format_duration(row.seconds))
            .wrapping(Wrapping::None)
            .style(colored(move |tokens| {
                if sent {
                    tokens.text_secondary
                } else {
                    tokens.text_primary
                }
            }))
            .into(),
        (phase != Phase::Review).then(|| status(row.status)),
    );
    let error = match (phase, row.status) {
        (Phase::Done, Some(Status::Failed(error) | Status::Partial(Some(error)))) => {
            Some(error_line(error))
        }
        _ => None,
    };
    Column::new()
        .push(container(line).padding([spacing::S050, spacing::S0]))
        .push(error)
        .into()
}

fn status<'a>(status: Option<Status<'a>>) -> Element<'a, Message> {
    let (label, color): (&str, fn(&Tokens) -> Color) = match status {
        None => ("", |tokens| tokens.text_tertiary),
        Some(Status::Sending) => ("sending", |tokens| tokens.text_secondary),
        Some(Status::Sent) => ("✓ sent", |tokens| tokens.status_success),
        Some(Status::Partial(_)) => ("◐ partial", |tokens| tokens.status_warning),
        Some(Status::Failed(_)) => ("× failed", |tokens| tokens.status_danger),
    };
    CAPTION
        .text(label)
        .wrapping(Wrapping::None)
        .style(colored(color))
        .into()
}

/// A line in statusDanger on a subtle background with a marker on its left.
fn error_line(error: &str) -> Element<'_, Message> {
    let text = container(
        CAPTION
            .text(error)
            .style(colored(|tokens| tokens.status_danger)),
    )
    .width(Length::Fill)
    .padding(Padding::from([spacing::S050, spacing::S100]).left(ERROR_MARKER_WIDTH + spacing::S150))
    .style(|theme| {
        container::Style::default()
            .background(tokens(theme).bg_subtle)
            .border(Border::default().rounded(radius::SM))
    });
    let marker = edge_marker(ERROR_MARKER_WIDTH, radius::SM, |tokens| {
        tokens.status_danger
    });
    container(stack![text, marker])
        .padding(Padding::ZERO.bottom(spacing::S075))
        .into()
}

/// The note above the table after some slots failed.
fn failed_note<'a>(failed: usize) -> Element<'a, Message> {
    let text = BODY
        .text(match failed {
            1 => "1 slot wasn't sent and stays unpublished. retry it now or publish it later."
                .to_owned(),
            failed => format!(
                "{failed} slots weren't sent and stay unpublished. retry them now or publish them later."
            ),
        })
        .style(secondary);
    let note = container(text)
        .width(Length::Fill)
        .padding(
            Padding::from([spacing::S100, spacing::S150]).left(NOTE_MARKER_WIDTH + spacing::S150),
        )
        .style(|theme| {
            let tokens = tokens(theme);
            container::Style::default()
                .background(tokens.bg_subtle)
                .border(Border {
                    color: tokens.border_default,
                    width: 1.0,
                    radius: radius::SM.into(),
                })
        });
    let marker = edge_marker(NOTE_MARKER_WIDTH, radius::SM, |tokens| {
        tokens.status_warning
    });
    stack![note, marker].into()
}

fn running_note(key: &str) -> Element<'_, Message> {
    row![
        BODY.text(key).style(colored(|tokens| tokens.text_accent)),
        BODY.text(" is still running and won't be published.")
            .style(tertiary),
    ]
    .into()
}

fn footer<'a>(
    state: &publish::State,
    phase: Phase,
    summary: Option<publish::Summary>,
) -> Element<'a, Message> {
    let publish = Message::Publish(publish::Message::Publish);
    let button = |label: String| {
        let button = Button::new(label).size(Size::Sm).key(RETURN_KEY).primary();
        match state.can_publish() {
            true => button.on_press(publish.clone()),
            false => button,
        }
    };
    let close = |label| {
        Button::new(label)
            .size(Size::Sm)
            .key("esc")
            .on_press(Message::CloseModal)
    };
    let content: Row<'a, Message> = match phase {
        Phase::Review => row![
            space::horizontal(),
            close("cancel"),
            button(format!("publish {}", format_duration(state.seconds()))),
        ],
        Phase::Sending => row![
            space::horizontal(),
            Button::new("publishing…").size(Size::Sm).primary(),
        ],
        Phase::Done => row![
            BODY.text("sent slots won't be sent again").style(tertiary),
            space::horizontal(),
            close("close"),
        ]
        .push(
            summary
                .filter(|summary| summary.failed > 0)
                .map(|summary| button(format!("retry {} failed", summary.failed))),
        ),
    };
    content
        .spacing(spacing::S100)
        .align_y(Vertical::Center)
        .into()
}

fn colored(color: impl Fn(&Tokens) -> Color) -> impl Fn(&Theme) -> text::Style {
    move |theme| text::Style {
        color: Some(color(tokens(theme))),
    }
}

fn secondary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_secondary),
    }
}

fn tertiary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_tertiary),
    }
}
