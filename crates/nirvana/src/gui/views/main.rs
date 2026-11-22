use iced::widget::{Column, container, responsive, row};
use iced::{Element, Length, Size};

use super::bottom_bar::bottom_bar;
use super::header::header;
use super::listing::listing;
use super::settings::{settings_bar, settings_view};
use super::setup::setup;
use super::tracking::tracking;
use super::with_toast;
use crate::gui::app::Message;
use crate::gui::design_system::tokens;
use crate::gui::profile::ProfileState;
use crate::gui::toast::Toast;
use crate::gui::{settings, tracking};

pub(in crate::gui) const MAIN_WINDOW_SIZE: Size = Size::new(960.0, 540.0);
const TOAST_BOTTOM: f32 = 76.0;

pub(in crate::gui) fn main<'a>(
    profile: Option<&'a ProfileState>,
    settings: &'a settings::State,
    tracking_state: &'a tracking::State,
    toast: Option<&'a Toast>,
) -> Element<'a, Message> {
    let header = header(profile, settings);
    let (body, bar): (Element<'a, Message>, _) = match settings.shown() {
        Some(idle) => (
            settings_view(idle, settings.is_idle_after_open()),
            settings_bar(),
        ),
        None => {
            let setup_step = profile.and_then(ProfileState::setup_step);
            let body = responsive(move |size| match setup_step {
                Some(step) => setup(size.width, step),
                None => row![tracking(), listing(size.width)].into(),
            });
            (
                body.into(),
                bottom_bar(tracking_state, setup_step.is_some()),
            )
        }
    };
    let layout = Column::new().push(header).push(body).push(bar);
    let content = container(layout)
        .width(Length::Fill)
        .height(Length::Fill)
        .style(|theme| container::background(tokens(theme).bg_default));
    with_toast(content.into(), toast, TOAST_BOTTOM)
}
