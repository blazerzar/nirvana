use iced::alignment::Vertical;
use iced::widget::{column, container, row, rule, text};
use iced::{Element, Length, padding};

use super::border;
use crate::gui::app::Message;
use crate::gui::components::Button;
use crate::gui::design_system::typography::BODY;
use crate::gui::design_system::{space, tokens};
use crate::gui::state::{start, tracking};

/// The tracking buttons, with start, resume and switch disabled while the
/// start form is open, or only a disabled start and stop during setup.
pub(super) fn bottom_bar(
    tracking: &tracking::State,
    starting: bool,
    setup: bool,
) -> Element<'_, Message> {
    let mut start = Button::new("start").key("s");
    let mut stop = Button::new("stop").key("x");
    let content = if setup {
        let hint = container(
            BODY.text("available once a profile is connected")
                .style(|theme| text::Style {
                    color: Some(tokens(theme).text_tertiary),
                }),
        )
        .padding(padding::left(space::S100));
        row![start, stop, hint]
    } else {
        if tracking.is_tracking() {
            stop = stop
                .outline()
                .on_press(Message::Tracking(tracking::Message::Stop));
        } else {
            start = start.outline();
        }
        if !starting {
            start = start.on_press(Message::Start(start::Message::Open));
        }
        let mut resume = Button::new("resume").key("r");
        if let Some(key) = tracking.resumable() {
            resume = resume.hint(key);
            if !starting {
                resume = resume.on_press(Message::Tracking(tracking::Message::Resume));
            }
        }
        let mut switch = Button::new("switch").key("w");
        if let Some(key) = tracking.switchable() {
            switch = switch.hint(format!("⇄ {key}"));
            if !starting {
                switch = switch.on_press(Message::Tracking(tracking::Message::Switch));
            }
        }
        row![start, stop, resume, switch]
    };

    let bar = container(content.spacing(space::S100).align_y(Vertical::Center))
        .width(Length::Fill)
        .height(60)
        .padding([space::S0, space::S200])
        .align_y(Vertical::Center);

    column![border(rule::horizontal(1)), bar].into()
}
