use iced::advanced::widget::{Operation, Tree};
use iced::advanced::{Clipboard, Layout, Shell, Widget, layout, mouse, overlay, renderer};
use iced::{Element, Event, Length, Rectangle, Size, Vector};

/// Wraps visual content and moves its completed layout horizontally.
///
/// Unlike `Pin`, the translation does not reduce or expand the space offered to
/// the child, so a fill-sized image keeps identical bounds throughout a pager
/// transition.
struct TranslateX<'a, Message, Theme, Renderer> {
    content: Element<'a, Message, Theme, Renderer>,
    x: f32,
}

impl<Message, Theme, Renderer> Widget<Message, Theme, Renderer>
    for TranslateX<'_, Message, Theme, Renderer>
where
    Renderer: iced::advanced::Renderer,
{
    fn tag(&self) -> iced::advanced::widget::tree::Tag {
        self.content.as_widget().tag()
    }

    fn state(&self) -> iced::advanced::widget::tree::State {
        self.content.as_widget().state()
    }

    fn children(&self) -> Vec<Tree> {
        self.content.as_widget().children()
    }

    fn diff(&self, tree: &mut Tree) {
        self.content.as_widget().diff(tree);
    }

    fn size(&self) -> Size<Length> {
        self.content.as_widget().size()
    }

    fn size_hint(&self) -> Size<Length> {
        self.content.as_widget().size_hint()
    }

    fn layout(
        &mut self,
        tree: &mut Tree,
        renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        translate_node(
            self.content.as_widget_mut().layout(tree, renderer, limits),
            self.x,
        )
    }

    fn operate(
        &mut self,
        tree: &mut Tree,
        layout: Layout<'_>,
        renderer: &Renderer,
        operation: &mut dyn Operation,
    ) {
        self.content
            .as_widget_mut()
            .operate(tree, layout, renderer, operation);
    }

    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        renderer: &Renderer,
        clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        viewport: &Rectangle,
    ) {
        self.content.as_widget_mut().update(
            tree, event, layout, cursor, renderer, clipboard, shell, viewport,
        );
    }

    fn mouse_interaction(
        &self,
        tree: &Tree,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
        renderer: &Renderer,
    ) -> mouse::Interaction {
        self.content
            .as_widget()
            .mouse_interaction(tree, layout, cursor, viewport, renderer)
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        style: &renderer::Style,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        viewport: &Rectangle,
    ) {
        self.content
            .as_widget()
            .draw(tree, renderer, theme, style, layout, cursor, viewport);
    }

    fn overlay<'a>(
        &'a mut self,
        tree: &'a mut Tree,
        layout: Layout<'a>,
        renderer: &Renderer,
        viewport: &Rectangle,
        translation: Vector,
    ) -> Option<overlay::Element<'a, Message, Theme, Renderer>> {
        self.content
            .as_widget_mut()
            .overlay(tree, layout, renderer, viewport, translation)
    }
}

pub(super) fn translate_x<'a, Message, Theme, Renderer>(
    content: impl Into<Element<'a, Message, Theme, Renderer>>,
    x: f32,
) -> Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: 'a,
    Renderer: iced::advanced::Renderer + 'a,
{
    Element::new(TranslateX {
        content: content.into(),
        x,
    })
}

fn translate_node(node: layout::Node, x: f32) -> layout::Node {
    node.translate(Vector::new(x, 0.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn translation_preserves_page_size_in_both_directions() {
        let size = Size::new(1280.0, 720.0);

        let left = translate_node(layout::Node::new(size), -320.0);
        let right = translate_node(layout::Node::new(size), 320.0);

        assert_eq!(left.size(), size);
        assert_eq!(right.size(), size);
        assert_eq!(left.bounds().x, -320.0);
        assert_eq!(right.bounds().x, 320.0);
    }
}
