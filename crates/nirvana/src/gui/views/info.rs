use iced::widget::text::{LineHeight, Wrapping};
use iced::widget::{Column, button, column, container, row, scrollable, text};
use iced::{Color, Element, Length, Size};

use crate::gui::app::{Info, Message};
use crate::gui::design_system::{Tokens, tokens};

const TEXT_SIZE: f32 = 10.0;
const LINE_HEIGHT: f32 = 1.3;
const MONO_ADVANCE: f32 = 0.6;
const PADDING: f32 = 16.0;
const LABEL_WIDTH: f32 = 120.0;
const ROW_SPACING: f32 = 6.0;
const SECTION_SPACING: f32 = 16.0;
const BUTTON_PADDING: [f32; 2] = [5.0, 10.0];
const SHOW_LOGS: &str = "Show logs";
const MAX_WIDTH: f32 = 720.0;

pub(in crate::gui) fn info(info: &Info) -> Element<'_, Message> {
    let body: Element<'_, Message> = match &info.rows {
        Ok(rows) => Column::with_children(rows.iter().map(|(label, value)| {
            row![
                colored(text(*label), |t| t.text_secondary).width(LABEL_WIDTH),
                colored(text(value), |t| t.text_primary).wrapping(Wrapping::Glyph),
            ]
            .into()
        }))
        .spacing(ROW_SPACING)
        .into(),
        Err(e) => colored(text(e), |t| t.status_danger)
            .wrapping(Wrapping::Glyph)
            .into(),
    };
    let show_logs = info.log_file.as_ref().map(|_| {
        button(sized(text(SHOW_LOGS)))
            .padding(BUTTON_PADDING)
            .on_press(Message::ShowLogs)
            .style(|theme, _| button::Style {
                background: Some(tokens(theme).fill_accent.into()),
                text_color: tokens(theme).text_on_accent,
                ..button::Style::default()
            })
    });
    let content = column![body].push(show_logs).spacing(SECTION_SPACING);
    container(scrollable(container(content).padding(PADDING)))
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default))
        .into()
}

pub(in crate::gui) fn info_size(info: &Info) -> Size {
    let char_width = TEXT_SIZE * MONO_ADVANCE;
    let line = TEXT_SIZE * LINE_HEIGHT;
    let fits = |width: f32| (width / char_width).floor() as usize;
    let (body_width, body_height) = match &info.rows {
        Ok(rows) => {
            let max_chars = fits(MAX_WIDTH - 2.0 * PADDING - LABEL_WIDTH);
            let measured: Vec<_> = rows
                .iter()
                .map(|(_, value)| measure(value, max_chars))
                .collect();
            let chars = measured.iter().map(|(chars, _)| *chars).max().unwrap_or(0);
            let lines: usize = measured.iter().map(|(_, lines)| lines).sum();
            let spacing = measured.len().saturating_sub(1) as f32 * ROW_SPACING;
            (
                LABEL_WIDTH + chars as f32 * char_width,
                lines as f32 * line + spacing,
            )
        }
        Err(e) => {
            let (chars, lines) = measure(e, fits(MAX_WIDTH - 2.0 * PADDING));
            (chars as f32 * char_width, lines as f32 * line)
        }
    };
    let (button_width, button_height) = match info.log_file {
        Some(_) => (
            SHOW_LOGS.chars().count() as f32 * char_width + 2.0 * BUTTON_PADDING[1],
            SECTION_SPACING + line + 2.0 * BUTTON_PADDING[0],
        ),
        None => (0.0, 0.0),
    };
    Size::new(
        (body_width.max(button_width) + 2.0 * PADDING).ceil(),
        (body_height + button_height + 2.0 * PADDING).ceil(),
    )
}

/// Returns the widest line in characters and the number of lines `value`
/// takes when wrapped at `max_chars`.
fn measure(value: &str, max_chars: usize) -> (usize, usize) {
    value.split('\n').fold((0, 0), |(widest, lines), line| {
        let chars = line.chars().count();
        (
            widest.max(chars.min(max_chars)),
            lines + chars.div_ceil(max_chars).max(1),
        )
    })
}

fn sized(text: text::Text<'_>) -> text::Text<'_> {
    text.size(TEXT_SIZE)
        .line_height(LineHeight::Relative(LINE_HEIGHT))
}

fn colored(text: text::Text<'_>, color: fn(&Tokens) -> Color) -> text::Text<'_> {
    sized(text).style(move |theme| text::Style {
        color: Some(color(tokens(theme))),
    })
}
