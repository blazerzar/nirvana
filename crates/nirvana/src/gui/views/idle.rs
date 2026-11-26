use iced::alignment::Vertical;
use iced::font::Font;
use iced::widget::text::{Span, Wrapping};
use iced::widget::{Row, button, column, container, rich_text, row, space, span, text, text_input};
use iced::{Border, Element, Length, Padding, Theme};

use super::main::TOAST_BOTTOM;
use super::start::input_style;
use super::with_toast;
use crate::gui::app::Message;
use crate::gui::components::{
    Button, RETURN_KEY, Size, modal, radio_option, status_icon, tracked_text,
};
use crate::gui::design_system::typography::{
    BODY, BODY_LG, BODY_STRONG, CAPTION, LABEL_CAPS, LABEL_STRONG, TITLE_SM,
};
use crate::gui::design_system::{Tokens, radius, space as spacing, tokens};
use crate::gui::state::idle_dialog::{self, Choice, Dialog, TicketPick};
use crate::gui::state::settings::IdleAfter;
use crate::gui::state::start::Recent;
use crate::gui::state::toast::{Kind, Toast};
use crate::time::{format_duration, time_of_day};

const WIDTH: f32 = 720.0;
const RECENT_MAX: usize = 4;
const KEY_INPUT_WIDTH: f32 = 92.0;
const KEY_INPUT_HEIGHT: f32 = 24.0;
const ICON_OFFSET: f32 = 2.0;
const SUBTITLE_GAP: f32 = 2.0;
const NOT_RECENT: &str = "not a recent ticket · checked when saved";

/// Asks what the user did while away from the running ticket, and what they
/// do now, with the toast over the modal so a failed save shows above it.
pub(in crate::gui) fn idle_modal<'a>(
    dialog: &'a Dialog,
    recent: Vec<&'a Recent>,
    idle_after: Option<IdleAfter>,
    toast: Option<&'a Toast>,
    theme_tokens: &'static Tokens,
) -> Element<'a, Message> {
    let period = dialog.period();
    let key = dialog.running_key();
    let duration = format_duration(period.to - period.from);
    let (from, to) = (time_of_day(period.from), time_of_day(period.to));
    let recent: Vec<_> = recent.into_iter().take(RECENT_MAX).collect();

    let subtitle = rich_text([
        span(format!("no input from {from} to {to} while ")),
        accent_key(key, theme_tokens),
        span(" was running. tell nirvana what that time was."),
    ])
    .font(BODY.font)
    .size(BODY.size)
    .line_height(BODY.line_height)
    .style(tertiary);
    let header = row![
        container(status_icon(Kind::Warning)).padding(Padding::ZERO.top(ICON_OFFSET)),
        column![
            TITLE_SM
                .text(format!("you were away for {duration}"))
                .style(primary),
            subtitle,
        ]
        .spacing(SUBTITLE_GAP),
    ]
    .spacing(spacing::S150);

    let away = dialog.away();
    let away_options = [
        radio_option(
            label("working on ", Some(key), away == Choice::Same, theme_tokens),
            format!("keep the {duration} on {key}"),
            away == Choice::Same,
            choose_away(Choice::Same),
            None,
        ),
        radio_option(
            label("something else", None, away == Choice::Other, theme_tokens),
            format!("log the {duration} to another ticket"),
            away == Choice::Other,
            choose_away(Choice::Other),
            Some(ticket_pick(
                dialog.away_pick(),
                &recent,
                idle_dialog::Message::AwayRecent,
                idle_dialog::Message::AwayKey,
            )),
        ),
        radio_option(
            label("on a break", None, away == Choice::Neither, theme_tokens),
            format!("leave it out · {key} stops at {from}"),
            away == Choice::Neither,
            choose_away(Choice::Neither),
            None,
        ),
    ];
    let now = dialog.now();
    let continue_caption = if away == Choice::Same {
        "the timer keeps running".to_owned()
    } else {
        format!("starts again at {to}")
    };
    let now_options = [
        radio_option(
            label("continue ", Some(key), now == Choice::Same, theme_tokens),
            continue_caption,
            now == Choice::Same,
            choose_now(Choice::Same),
            None,
        ),
        radio_option(
            label("something else", None, now == Choice::Other, theme_tokens),
            format!("start another ticket from {to}"),
            now == Choice::Other,
            choose_now(Choice::Other),
            Some(ticket_pick(
                dialog.now_pick(),
                &recent,
                idle_dialog::Message::NowRecent,
                idle_dialog::Message::NowKey,
            )),
        ),
        radio_option(
            label(
                "nothing · stop tracking",
                None,
                now == Choice::Neither,
                theme_tokens,
            ),
            format!("no timer after {to}"),
            now == Choice::Neither,
            choose_now(Choice::Neither),
            None,
        ),
    ];
    let body = row![
        question(&format!("what were you doing, {from}–{to}?"), away_options),
        question("what are you doing now?", now_options),
    ]
    .spacing(spacing::S200)
    .padding([spacing::S100, spacing::S0]);

    with_toast(
        modal(header, body, footer(dialog, idle_after), WIDTH),
        toast,
        TOAST_BOTTOM,
    )
}

fn question<'a>(
    title: &str,
    options: impl IntoIterator<Item = Element<'a, Message>>,
) -> Element<'a, Message> {
    let title = container(tracked_text(LABEL_CAPS, title, tertiary))
        .padding(Padding::ZERO.bottom(spacing::S025));
    column![title]
        .extend(options)
        .spacing(spacing::S075)
        .width(Length::Fill)
        .into()
}

/// The label of an option, with the running ticket `key` after it.
fn label<'a>(
    label: &'a str,
    key: Option<&'a str>,
    is_on: bool,
    theme_tokens: &'static Tokens,
) -> Element<'a, Message> {
    let style = if is_on { LABEL_STRONG } else { BODY_LG };
    let mut spans = vec![span(label)];
    spans.extend(key.map(|key| accent_key(key, theme_tokens)));
    rich_text(spans)
        .font(style.font)
        .size(style.size)
        .line_height(style.line_height)
        .wrapping(Wrapping::None)
        .style(primary)
        .into()
}

fn accent_key<'a>(key: &'a str, theme_tokens: &'static Tokens) -> Span<'a, (), Font> {
    span(key)
        .font(BODY_STRONG.font)
        .color(theme_tokens.text_accent)
}

/// The recent tickets other than the running one and a key input, with the
/// title of the picked ticket below them.
fn ticket_pick<'a>(
    pick: &'a TicketPick,
    recent: &[&'a Recent],
    on_recent: fn(Recent) -> idle_dialog::Message,
    on_key: fn(String) -> idle_dialog::Message,
) -> Element<'a, Message> {
    let chips = recent.iter().map(|&recent| {
        ticket_chip(
            &recent.key,
            pick.is_picked(recent),
            Message::IdleDialog(on_recent(recent.clone())),
        )
    });
    let text_height = BODY.line_height.to_absolute(BODY.size.into()).0;
    let input = text_input("other key", pick.typed())
        .on_input(move |key| Message::IdleDialog(on_key(key)))
        .font(BODY.font)
        .size(BODY.size)
        .line_height(BODY.line_height)
        .padding([(KEY_INPUT_HEIGHT - text_height) / 2.0, spacing::S100])
        .width(KEY_INPUT_WIDTH)
        .style(input_style);
    let picks = Row::with_children(chips)
        .push(input)
        .spacing(spacing::S050)
        .align_y(Vertical::Center)
        .wrap()
        .vertical_spacing(spacing::S050);
    let hint = pick.key().map(|key| {
        let title = recent
            .iter()
            .find(|recent| recent.key.eq_ignore_ascii_case(key))
            .map(|recent| recent.title.as_str());
        let color = if title.is_some() { secondary } else { tertiary };
        CAPTION
            .text(title.unwrap_or(NOT_RECENT))
            .wrapping(Wrapping::None)
            .style(color)
    });
    column![picks].push(hint).spacing(spacing::S050).into()
}

/// A recent ticket key, filled with the accent when picked.
fn ticket_chip<'a>(key: &'a str, is_picked: bool, on_press: Message) -> Element<'a, Message> {
    let style = if is_picked { BODY_STRONG } else { BODY };
    button(style.text(key).wrapping(Wrapping::None))
        .height(KEY_INPUT_HEIGHT)
        .padding([spacing::S025, spacing::S100])
        .on_press(on_press)
        .style(move |theme, status| {
            let tokens = tokens(theme);
            let (background, text, border) = match status {
                _ if is_picked => (
                    tokens.fill_accent,
                    tokens.text_on_accent,
                    tokens.border_accent,
                ),
                button::Status::Hovered => (
                    tokens.fill_hover,
                    tokens.text_primary,
                    tokens.border_default,
                ),
                button::Status::Pressed => (
                    tokens.fill_pressed,
                    tokens.text_primary,
                    tokens.border_default,
                ),
                _ => (
                    tokens.bg_raised,
                    tokens.text_secondary,
                    tokens.border_default,
                ),
            };
            button::Style {
                background: Some(background.into()),
                text_color: text,
                border: Border {
                    color: border,
                    width: 1.0,
                    radius: radius::SM.into(),
                },
                ..button::Style::default()
            }
        })
        .into()
}

fn footer(dialog: &Dialog, idle_after: Option<IdleAfter>) -> Element<'_, Message> {
    let save = Button::new("save").size(Size::Sm).key(RETURN_KEY).primary();
    let save = match dialog.can_save() {
        true => save.on_press(Message::IdleDialog(idle_dialog::Message::Save)),
        false => save,
    };
    let note = idle_after.map(|idle_after| {
        CAPTION
            .text(format!(
                "asks after {idle_after} without input · change it in settings"
            ))
            .style(tertiary)
    });
    row![]
        .push(note)
        .extend([
            space::horizontal().into(),
            Button::new("keep as is")
                .size(Size::Sm)
                .key("esc")
                .on_press(Message::IdleDialog(idle_dialog::Message::Keep))
                .into(),
            save.into(),
        ])
        .spacing(spacing::S100)
        .align_y(Vertical::Center)
        .into()
}

fn choose_away(choice: Choice) -> Message {
    Message::IdleDialog(idle_dialog::Message::Away(choice))
}

fn choose_now(choice: Choice) -> Message {
    Message::IdleDialog(idle_dialog::Message::Now(choice))
}

fn primary(theme: &Theme) -> text::Style {
    text::Style {
        color: Some(tokens(theme).text_primary),
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
