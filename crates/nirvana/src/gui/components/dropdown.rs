use iced::advanced::layout::{self, Layout};
use iced::advanced::widget::{self, Tree, Widget};
use iced::advanced::{Clipboard, Shell, overlay, renderer};
use iced::{Element, Event, Length, Point, Rectangle, Size, Vector, keyboard, mouse};

/// Shows `menu` below `trigger` while `is_open`, at least as wide as the
/// trigger, above it when it does not fit below and shifted left when it does
/// not fit on the right. Publishes `on_dismiss` on
/// a click outside the menu or on Esc.
pub(in crate::gui) struct Dropdown<'a, Message> {
    trigger: Element<'a, Message>,
    menu: Element<'a, Message>,
    is_open: bool,
    gap: f32,
    on_dismiss: Message,
}

impl<'a, Message> Dropdown<'a, Message> {
    pub(in crate::gui) fn new(
        trigger: impl Into<Element<'a, Message>>,
        menu: impl Into<Element<'a, Message>>,
        is_open: bool,
        gap: f32,
        on_dismiss: Message,
    ) -> Self {
        Self {
            trigger: trigger.into(),
            menu: menu.into(),
            is_open,
            gap,
            on_dismiss,
        }
    }
}

impl<Message: Clone> Widget<Message, iced::Theme, iced::Renderer> for Dropdown<'_, Message> {
    fn children(&self) -> Vec<Tree> {
        vec![Tree::new(&self.trigger), Tree::new(&self.menu)]
    }

    fn diff(&self, tree: &mut Tree) {
        tree.diff_children(&[&self.trigger, &self.menu]);
    }

    fn size(&self) -> Size<Length> {
        self.trigger.as_widget().size()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &iced::Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        self.trigger
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
        self.trigger.as_widget_mut().update(
            &mut tree.children[0],
            event,
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
        self.trigger.as_widget().mouse_interaction(
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
        self.trigger.as_widget().draw(
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
        _renderer: &iced::Renderer,
        _viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'b, Message, iced::Theme, iced::Renderer>> {
        self.is_open.then(|| {
            overlay::Element::new(Box::new(Menu {
                menu: &mut self.menu,
                tree: &mut tree.children[1],
                trigger: layout.bounds() + translation,
                gap: self.gap,
                on_dismiss: self.on_dismiss.clone(),
            }))
        })
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &iced::Renderer,
        operation: &mut dyn widget::Operation,
    ) {
        self.trigger
            .as_widget_mut()
            .operate(&mut tree.children[0], layout, renderer, operation);
    }
}

impl<'a, Message: Clone + 'a> From<Dropdown<'a, Message>> for Element<'a, Message> {
    fn from(dropdown: Dropdown<'a, Message>) -> Self {
        Element::new(dropdown)
    }
}

struct Menu<'a, 'b, Message> {
    menu: &'b mut Element<'a, Message>,
    tree: &'b mut Tree,
    trigger: Rectangle,
    gap: f32,
    on_dismiss: Message,
}

impl<Message: Clone> overlay::Overlay<Message, iced::Theme, iced::Renderer>
    for Menu<'_, '_, Message>
{
    fn layout(&mut self, renderer: &iced::Renderer, bounds: Size) -> layout::Node {
        let limits = layout::Limits::new(Size::new(self.trigger.width, 0.0), bounds);
        let node = self
            .menu
            .as_widget_mut()
            .layout(self.tree, renderer, &limits);
        let below = self.trigger.y + self.trigger.height + self.gap;
        let above = self.trigger.y - self.gap - node.size().height;
        let y = if below + node.size().height > bounds.height && above >= 0.0 {
            above
        } else {
            below
        };
        let x = self
            .trigger
            .x
            .min(bounds.width - node.size().width)
            .max(0.0);
        node.move_to(Point::new(x, y))
    }

    fn draw(
        &self,
        renderer: &mut iced::Renderer,
        theme: &iced::Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
    ) {
        self.menu.as_widget().draw(
            self.tree,
            renderer,
            theme,
            style,
            layout,
            cursor,
            &layout.bounds(),
        );
    }

    fn update(
        &mut self,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
    ) {
        let dismissed = match event {
            Event::Mouse(mouse::Event::ButtonPressed(_)) => !cursor.is_over(layout.bounds()),
            Event::Keyboard(keyboard::Event::KeyPressed {
                key: keyboard::Key::Named(keyboard::key::Named::Escape),
                ..
            }) => true,
            _ => false,
        };
        if dismissed {
            shell.publish(self.on_dismiss.clone());
            shell.capture_event();
            return;
        }
        self.menu.as_widget_mut().update(
            self.tree,
            event,
            layout,
            cursor,
            renderer,
            clipboard,
            shell,
            &layout.bounds(),
        );
    }

    fn mouse_interaction(
        &self,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &iced::Renderer,
    ) -> mouse::Interaction {
        self.menu.as_widget().mouse_interaction(
            self.tree,
            layout,
            cursor,
            &layout.bounds(),
            renderer,
        )
    }
}
