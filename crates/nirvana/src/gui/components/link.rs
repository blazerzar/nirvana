use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{Tree, Widget};
use iced::advanced::{Clipboard, Renderer as _, Shell, renderer};
use iced::widget::Text;
use iced::{Element, Event, Length, Rectangle, Size, mouse, window};

use crate::gui::design_system::tokens;

const UNDERLINE_WIDTH: f32 = 1.0;

/// An underlined text link that turns primary on hover. Without `on_press`
/// it is accent text without an underline.
pub(in crate::gui) fn link<'a, Message: Clone + 'a>(
    label: Text<'a>,
    on_press: Option<Message>,
) -> Element<'a, Message> {
    Element::new(Link {
        label: label.into(),
        on_press,
        is_hovered: None,
    })
}

struct Link<'a, Message> {
    label: Element<'a, Message>,
    on_press: Option<Message>,
    /// Whether the link was hovered when it was last drawn.
    is_hovered: Option<bool>,
}

impl<Message> Link<'_, Message> {
    fn is_hovered(&self, layout: Layout<'_>, cursor: mouse::Cursor) -> bool {
        self.on_press.is_some() && cursor.is_over(layout.bounds())
    }
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for Link<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.label)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.label]);
    }

    fn size(&self) -> Size<Length> {
        self.label.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.label
            .as_widget_mut()
            .layout(&mut tree.children[0], renderer, limits)
    }

    fn update(
        &mut self,
        _tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &iced::Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let is_hovered = self.is_hovered(layout, cursor);
        match event {
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) if is_hovered => {
                if let Some(on_press) = &self.on_press {
                    shell.publish(on_press.clone());
                    shell.capture_event();
                }
            }
            Event::Window(window::Event::RedrawRequested(_)) => {
                self.is_hovered = Some(is_hovered);
            }
            _ if self.is_hovered.is_some_and(|was| was != is_hovered) => {
                shell.request_redraw();
            }
            _ => {}
        }
    }

    fn mouse_interaction(
        &self,
        _tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _viewport: &Rectangle,
        _renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        if self.is_hovered(layout, cursor) {
            mouse::Interaction::Pointer
        } else {
            mouse::Interaction::None
        }
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        let tokens = tokens(theme);
        let is_hovered = self.is_hovered(layout, cursor);
        let (text_color, underline) = if is_hovered {
            (tokens.text_primary, tokens.accent)
        } else {
            (tokens.text_accent, tokens.border_accent)
        };
        self.label.as_widget().draw(
            &tree.children[0],
            renderer,
            theme,
            &renderer::Style { text_color },
            layout,
            cursor,
            viewport,
        );
        if self.on_press.is_some() {
            let bounds = layout.bounds();
            renderer.fill_quad(
                renderer::Quad {
                    bounds: Rectangle {
                        y: bounds.y + bounds.height - UNDERLINE_WIDTH,
                        height: UNDERLINE_WIDTH,
                        ..bounds
                    },
                    ..renderer::Quad::default()
                },
                underline,
            );
        }
    }
}
