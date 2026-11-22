use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::keyboard::key::Named;
use iced::{Element, Event, Length, Rectangle, Size, Vector, keyboard, mouse};

type KeyHandler<'a, Message> = Box<dyn Fn(&keyboard::Event) -> Option<Message> + 'a>;

/// Listens to the key presses of `content`. `on_key` sees every key event
/// before `content` does, and the event is captured when it publishes.
/// `on_escape` is published when `content` captures an Esc press, like a
/// focused text input does to drop its focus.
pub(in crate::gui) struct KeyListener<'a, Message> {
    content: Element<'a, Message>,
    on_key: Option<KeyHandler<'a, Message>>,
    on_escape: Option<Message>,
}

impl<'a, Message> KeyListener<'a, Message> {
    pub(in crate::gui) fn new(content: impl Into<Element<'a, Message>>) -> Self {
        Self {
            content: content.into(),
            on_key: None,
            on_escape: None,
        }
    }

    pub(in crate::gui) fn on_key(
        mut self,
        on_key: impl Fn(&keyboard::Event) -> Option<Message> + 'a,
    ) -> Self {
        self.on_key = Some(Box::new(on_key));
        self
    }

    pub(in crate::gui) fn on_escape(mut self, message: Message) -> Self {
        self.on_escape = Some(message);
        self
    }
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for KeyListener<'_, Message> {
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
        if let (Some(on_key), Event::Keyboard(key_event)) = (&self.on_key, event)
            && let Some(message) = on_key(key_event)
        {
            shell.publish(message);
            shell.capture_event();
            return;
        }
        let was_captured = shell.is_event_captured();
        self.content.as_widget_mut().update(
            &mut tree.children[0],
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            viewport,
        );
        let is_escape = matches!(
            event,
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(Named::Escape),
                ..
            })
        );
        if let Some(on_escape) = &self.on_escape
            && is_escape
            && !was_captured
            && shell.is_event_captured()
        {
            shell.publish(on_escape.clone());
        }
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
        theme: &iced::Theme,
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
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
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

impl<'a, Message: Clone + 'a> From<KeyListener<'a, Message>> for Element<'a, Message> {
    fn from(listener: KeyListener<'a, Message>) -> Self {
        Element::new(listener)
    }
}
