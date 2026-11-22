use std::cell::Cell;

use iced::widget::canvas::{self, Canvas, LineDash, Path, Stroke};
use iced::{Color, Element, Length, Point, Rectangle, Renderer, Size, Theme, mouse};

use crate::gui::design_system::tokens;

/// A dashed `border_default` border filling its space, to stack over content.
pub(in crate::gui) fn dashed_border<'a, Message: 'a>(radius: f32) -> Element<'a, Message> {
    Canvas::new(DashedBorder { radius })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct DashedBorder {
    radius: f32,
}

#[derive(Default)]
struct DashedBorderCache {
    cache: canvas::Cache,
    color: Cell<Option<Color>>,
}

impl<Message> canvas::Program<Message> for DashedBorder {
    type State = DashedBorderCache;

    fn draw(
        &self,
        state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let color = tokens(theme).border_default;
        if state.color.replace(Some(color)) != Some(color) {
            state.cache.clear();
        }
        let geometry = state.cache.draw(renderer, bounds.size(), |frame| {
            let border = Path::rounded_rectangle(
                Point::new(0.5, 0.5),
                Size::new(bounds.width - 1.0, bounds.height - 1.0),
                self.radius.into(),
            );
            let stroke = Stroke {
                line_dash: LineDash {
                    segments: &[3.0, 3.0],
                    offset: 0,
                },
                ..Stroke::default().with_color(color).with_width(1.0)
            };
            frame.stroke(&border, stroke);
        });
        vec![geometry]
    }
}
