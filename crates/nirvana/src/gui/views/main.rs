use iced::widget::{Column, container, responsive, row};
use iced::{Element, Length, Size};

use super::bottom_bar::bottom_bar;
use super::listing::listing;
use super::setup::setup;
use super::start::start;
use super::tracking::tracking;
use super::with_toast;
use crate::gui::app::Message;
use crate::gui::design_system::{Tokens, tokens};
use crate::gui::state::profile::ProfileState;
use crate::gui::state::toast::Toast;
use crate::gui::state::{start as start_form, tracking};

pub(in crate::gui) const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);
const TOAST_BOTTOM: f32 = 76.0;

/// The main window: the header, then the shown screen and its bottom bar.
pub(in crate::gui) fn main<'a>(
    header: Element<'a, Message>,
    (body, bar): (Element<'a, Message>, Element<'a, Message>),
    toast: Option<&'a Toast>,
) -> Element<'a, Message> {
    let layout = Column::new().push(header).push(body).push(bar);
    let content = container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default));
    with_toast(content.into(), toast, TOAST_BOTTOM)
}

/// The setup steps, or the tracking pane or start form next to the listing,
/// with the bottom bar.
pub(in crate::gui) fn main_screen<'a>(
    profile: Option<&'a ProfileState>,
    tracking_state: &'a tracking::State,
    start_state: &'a start_form::State,
    theme_tokens: &'static Tokens,
    command_held: bool,
) -> (Element<'a, Message>, Element<'a, Message>) {
    let setup_step = profile.and_then(ProfileState::setup_step);
    let body = responsive(move |size| match setup_step {
        Some(step) => setup(size.width, step),
        None if start_state.is_open() => {
            row![start(start_state, tracking_state), listing(size.width)].into()
        }
        None => row![
            tracking(tracking_state, theme_tokens, command_held),
            listing(size.width)
        ]
        .into(),
    });
    (
        body.into(),
        bottom_bar(tracking_state, start_state.is_open(), setup_step.is_some()),
    )
}
