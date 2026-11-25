use iced::widget::canvas::{self, Canvas, Path};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::gui::design_system::{Tokens, tokens};

/// The left strip of a rectangle with corners of `radius`, `width` wide and
/// filling its height, to stack over the rectangle. Its outer edge follows
/// the corners while its inner edge stays straight.
pub(in crate::gui) fn edge_marker<'a, Message: 'a>(
    width: f32,
    radius: f32,
    color: fn(&Tokens) -> Color,
) -> Element<'a, Message> {
    Canvas::new(EdgeMarker { radius, color })
        .width(width)
        .height(Length::Fill)
        .into()
}

struct EdgeMarker {
    radius: f32,
    color: fn(&Tokens) -> Color,
}

impl<Message> canvas::Program<Message> for EdgeMarker {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        frame.with_clip(Rectangle::with_size(bounds.size()), |frame| {
            let shape = Path::rounded_rectangle(
                Point::ORIGIN,
                Size::new((2.0 * self.radius).max(bounds.width), bounds.height),
                self.radius.into(),
            );
            frame.fill(&shape, (self.color)(tokens(theme)));
        });
        vec![frame.into_geometry()]
    }
}
