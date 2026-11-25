use iced::Element;
use iced::widget::{Space, row, space, text};
use nirvana_core::UnpublishedSummary;

use crate::gui::app::Message;
use crate::gui::components::{Button, RETURN_KEY, modal};
use crate::gui::design_system::typography::HEADING_SM;
use crate::gui::design_system::{space as spacing, tokens};
use crate::time::format_duration;

const WIDTH: f32 = 640.0;

/// The publish modal with cancel and publish buttons.
pub(in crate::gui) fn publish_modal(
    unpublished: Option<&UnpublishedSummary>,
) -> Element<'_, Message> {
    let title = HEADING_SM.text("publish").style(|theme| text::Style {
        color: Some(tokens(theme).text_primary),
    });
    let cancel = Button::new("cancel")
        .key("esc")
        .on_press(Message::CloseModal);
    let label = unpublished.map_or_else(
        || "publish".to_owned(),
        |unpublished| format!("publish {}", format_duration(unpublished.seconds)),
    );
    let publish = Button::new(label).key(RETURN_KEY).primary();
    let footer = row![space::horizontal(), cancel, publish].spacing(spacing::S100);
    modal(title, Space::new(), footer, WIDTH)
}
