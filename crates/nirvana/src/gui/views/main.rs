use iced::widget::{Column, container, responsive, row};
use iced::{Element, Length, Size};

use super::bottom_bar::bottom_bar;
use super::header::header;
use super::listing::listing;
use super::setup::{setup, setup_bottom_bar};
use super::tracking::tracking;
use super::with_toast;
use crate::gui::app::Message;
use crate::gui::design_system::tokens;
use crate::gui::profile::ProfileState;
use crate::gui::toast::Toast;

pub(in crate::gui) const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);
const TOAST_BOTTOM: f32 = 76.0;

pub(in crate::gui) fn main<'a>(
    profile: Option<&'a ProfileState>,
    toast: Option<&'a Toast>,
) -> Element<'a, Message> {
    let setup_step = profile.and_then(ProfileState::setup_step);
    let body = responsive(move |size| match setup_step {
        Some(step) => setup(size.width, step),
        None => row![tracking(), listing(size.width)].into(),
    });
    let bar = match setup_step {
        Some(_) => setup_bottom_bar(),
        None => bottom_bar(row![]),
    };
    let layout = Column::new().push(header(profile)).push(body).push(bar);
    let content = container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default));
    with_toast(content.into(), toast, TOAST_BOTTOM)
}
