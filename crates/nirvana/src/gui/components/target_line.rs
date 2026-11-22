use iced::alignment::Vertical;
use iced::widget::canvas::{self, Canvas, LineDash, Path, Stroke};
use iced::widget::text::Alignment;
use iced::{Element, Length, Pixels, Point, Rectangle, Renderer, Theme, mouse};

use crate::gui::design_system::tokens;
use crate::gui::design_system::typography::MICRO;

/// The distance from the top of the label to the line.
const LABEL_OFFSET: f32 = 15.0;

/// A labelled dashed line across its space, at `share` of the height above
/// the bottom after leaving out `top` pixels at the top.
pub(in crate::gui) fn target_line<'a, Message: 'a>(
    share: f32,
    top: f32,
    label: String,
) -> Element<'a, Message> {
    Canvas::new(TargetLine { share, top, label })
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
}

struct TargetLine {
    share: f32,
    top: f32,
    label: String,
}

impl<Message> canvas::Program<Message> for TargetLine {
    type State = ();

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let tokens = tokens(theme);
        let mut frame = canvas::Frame::new(renderer, bounds.size());
        let y = (bounds.height - (bounds.height - self.top) * self.share).round() + 0.5;
        let line = Path::line(Point::new(0.0, y), Point::new(bounds.width, y));
        let stroke = Stroke {
            line_dash: LineDash {
                segments: &[3.0, 3.0],
                offset: 0,
            },
            ..Stroke::default()
                .with_color(tokens.fill_neutral_strong)
                .with_width(1.0)
        };
        frame.stroke(&line, stroke);
        frame.fill_text(canvas::Text {
            content: self.label.clone(),
            position: Point::new(bounds.width, y - LABEL_OFFSET),
            color: tokens.text_tertiary,
            size: Pixels(MICRO.size),
            line_height: MICRO.line_height,
            font: MICRO.font,
            align_x: Alignment::Right,
            align_y: Vertical::Top,
            ..canvas::Text::default()
        });
        vec![frame.into_geometry()]
    }
}
