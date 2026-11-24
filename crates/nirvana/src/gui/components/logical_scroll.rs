use std::borrow::Cow;

use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Theme, Vector, mouse};

/// Passes trackpad scroll deltas to `content` in logical pixels. iced_winit
/// 0.14 sends them in physical pixels, which scrolls `scale_factor` times
/// too far.
pub(in crate::gui) struct LogicalScroll<'a, Message> {
    content: Element<'a, Message>,
    scale_factor: f32,
}

impl<'a, Message> LogicalScroll<'a, Message> {
    pub(in crate::gui) fn new(content: impl Into<Element<'a, Message>>, scale_factor: f32) -> Self {
        Self {
            content: content.into(),
            scale_factor,
        }
    }
}

impl<Message> Widget<Message, Theme, iced::Renderer> for LogicalScroll<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.content)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.content]);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.content
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            &logical(event, self.scale_factor),
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.content.as_widget().mouse_interaction(
            &tree.children[0],
            layout,
            cursor,
            viewport,
            renderer,
        )
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            style,
            layout,
            cursor,
            viewport,
        );
    }

    fn overlay<'b>(
        &'b mut self,
        tree: &'b mut Tree,
        layout: Layout<'b>,
        renderer: &iced::Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, Theme, iced::Renderer>> {
        self.content.as_widget_mut().overlay(
            &mut tree.children[0],
            layout,
            renderer,
            viewport,
            translation,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
}

impl<'a, Message: 'a> From<LogicalScroll<'a, Message>> for Element<'a, Message> {
    fn from(scroll: LogicalScroll<'a, Message>) -> Self {
        Element::new(scroll)
    }
}

/// Returns `event` with a pixel scroll delta divided by `scale_factor`.
fn logical(event: &Event, scale_factor: f32) -> Cow<'_, Event> {
    match event {
        Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels { x, y },
        }) => Cow::Owned(Event::Mouse(mouse::Event::WheelScrolled {
            delta: mouse::ScrollDelta::Pixels {
                x: x / scale_factor,
                y: y / scale_factor,
            },
        })),
        _ => Cow::Borrowed(event),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scrolled(delta: mouse::ScrollDelta) -> Event {
        Event::Mouse(mouse::Event::WheelScrolled { delta })
    }

    #[test]
    fn logical_divides_pixel_deltas_by_the_scale_factor() {
        let event = scrolled(mouse::ScrollDelta::Pixels { x: 4.0, y: -10.0 });
        assert_eq!(
            *logical(&event, 2.0),
            scrolled(mouse::ScrollDelta::Pixels { x: 2.0, y: -5.0 })
        );
    }

    #[test]
    fn logical_keeps_line_deltas() {
        let event = scrolled(mouse::ScrollDelta::Lines { x: 0.0, y: 3.0 });
        assert_eq!(*logical(&event, 2.0), event);
    }
}
