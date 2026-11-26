use iced::widget::{Column, container, responsive, row, stack};
use iced::{Element, Length, Size};

use super::bottom_bar::bottom_bar;
use super::listing::{full_pane, listing, side_pane};
use super::setup::setup;
use super::start::start;
use super::tracking::tracking;
use super::with_toast;
use crate::gui::app::Message;
use crate::gui::design_system::{Tokens, tokens};
use crate::gui::state::profile::ProfileState;
use crate::gui::state::toast::Toast;
use crate::gui::state::{listing as listing_state, start as start_form, tracking};

pub(in crate::gui) const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);
pub(super) const TOAST_BOTTOM: f32 = 76.0;

/// The main window: the header, then the shown screen and its bottom bar,
/// with the toast over them and the modal over the toast.
pub(in crate::gui) fn main<'a>(
    header: Element<'a, Message>,
    (body, bar): (Element<'a, Message>, Element<'a, Message>),
    modal: Option<Element<'a, Message>>,
    toast: Option<&'a Toast>,
) -> Element<'a, Message> {
    let layout = Column::new().push(header).push(body).push(bar);
    let content = container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default));
    stack![with_toast(content.into(), toast, TOAST_BOTTOM)]
        .push(modal)
        .into()
}

/// The setup steps, the start form next to the listing, the expanded
/// listing, or the tracking pane next to the listing, with the bottom bar.
pub(in crate::gui) fn main_screen<'a>(
    profile: Option<&'a ProfileState>,
    tracking_state: &'a tracking::State,
    start_state: &'a start_form::State,
    listing_state: &'a listing_state::State,
    theme_tokens: &'static Tokens,
    command_held: bool,
    web_available: bool,
) -> (Element<'a, Message>, Element<'a, Message>) {
    let setup_step = profile.and_then(ProfileState::setup_step);
    let body = responsive(move |size| match setup_step {
        Some(step) => setup(size.width, step, web_available),
        None if start_state.is_open() => row![
            start(start_state, tracking_state),
            side_pane(
                listing(listing_state, tracking_state, web_available),
                size.width
            )
        ]
        .into(),
        None if listing_state.is_expanded() => {
            full_pane(listing(listing_state, tracking_state, web_available))
        }
        None => row![
            tracking(tracking_state, theme_tokens, command_held),
            side_pane(
                listing(listing_state, tracking_state, web_available),
                size.width
            )
        ]
        .into(),
    });
    (
        body.into(),
        bottom_bar(
            tracking_state,
            listing_state.unpublished(),
            start_state.is_open(),
            setup_step.is_some(),
        ),
    )
}
