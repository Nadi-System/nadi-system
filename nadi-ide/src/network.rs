//! Network display in the form of table.
use iced::{Event, Renderer};
use iced_core::renderer::{self, Renderer as _};
use iced_core::widget::tree::{self, Tree};
use iced_core::{
    Clipboard, Color, Element, Layout, Length, Point, Rectangle, Shell, Size, Theme, Widget,
};
use iced_core::{event, layout, mouse};
use iced_graphics::geometry::{Path, Stroke};
use std::cell::RefCell;

mod dtypes;
pub use dtypes::NetworkData;

#[allow(missing_debug_implementations)]
pub struct NetworkTable<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    data: &'a NetworkData,
    on_press: Option<Box<dyn Fn(Option<String>) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> NetworkTable<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Creates a new [`NetworkTable`] with the provided [`Data`].
    pub fn new(data: &'a NetworkData) -> Self {
        Self {
            data,
            on_press: None,
            class: Theme::default(),
        }
    }

    /// Sets the style of the [`NetworkTable`].
    pub fn style(mut self, style: impl Fn(&Theme) -> Style + 'a) -> Self
    where
        Theme::Class<'a>: From<StyleFn<'a, Theme>>,
    {
        self.class = (Box::new(style) as StyleFn<'a, Theme>).into();
        self
    }

    /// Sets the style class of the [`NetworkTable`].
    pub fn class(mut self, class: impl Into<Theme::Class<'a>>) -> Self {
        self.class = class.into();
        self
    }
    pub fn on_press(mut self, on_press: impl Fn(Option<String>) -> Message + 'a) -> Self {
        self.on_press = Some(Box::new(on_press));
        self
    }
}

impl<Message, Theme> Widget<Message, Theme, Renderer> for NetworkTable<'_, Message, Theme>
where
    Theme: Catalog,
{
    fn tag(&self) -> tree::Tag {
        tree::Tag::of::<State>()
    }

    fn state(&self) -> tree::State {
        tree::State::new(State::default())
    }

    fn size(&self) -> Size<Length> {
        Size {
            width: Length::Shrink,
            height: Length::Shrink,
        }
    }

    fn layout(
        &self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        _limits: &layout::Limits,
    ) -> layout::Node {
        let mut x = (self.data.maxlevel + 2) as f32 * self.data.deltax + self.data.offsetx * 2.0;
        let num_chars = self
            .data
            .nodes
            .iter()
            .map(|n| n.label.len())
            .max()
            .unwrap_or_default() as f32;
        x += num_chars * 15.0; // TODO find text length required to draw it
        let y = (self.data.nodes.len() + 2) as f32 * self.data.deltay + self.data.offsety * 3.0;

        layout::Node::new(Size::new(x * self.data.scale, y * self.data.scale))
    }

    // might have to use this for hover effect
    fn on_event(
        &mut self,
        tree: &mut Tree,
        event: Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) -> event::Status {
        let state = tree.state.downcast_mut::<State>();
        let node = cursor.position_in(layout.bounds()).and_then(|pt| {
            let y = ((pt.y - self.data.offsety) / self.data.deltay).round() - 1.0;
            if y < 0.0 {
                None
            } else {
                self.data.nodes.get(y as usize).map(|n| n.name.to_string())
            }
        });
        if state.over_node != node {
            state.over_node = node;
            self.data.cache.clear();
        }
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event {
            if let Some(on_press) = &self.on_press {
                if let Some(node) = &state.over_node {
                    shell.publish(on_press(Some(node.to_string())));
                    return event::Status::Captured;
                } else if cursor.is_over(layout.bounds()) {
                    shell.publish(on_press(None));
                    return event::Status::Captured;
                }
            }
        }
        event::Status::Ignored
    }

    fn draw(
        &self,
        tree: &Tree,
        renderer: &mut Renderer,
        theme: &Theme,
        _style: &renderer::Style,
        layout: Layout<'_>,
        _cursor: mouse::Cursor,
        _viewport: &Rectangle,
    ) {
        let state = tree.state.downcast_ref::<State>();
        let bounds = layout.bounds();

        let style = theme.style(&self.class);
        let mut last_style = state.last_style.borrow_mut();

        if Some(style) != *last_style {
            self.data.cache.clear();

            *last_style = Some(style);
        }

        // Reuse cache if possible
        let geometry = self.data.cache.draw(renderer, bounds.size(), |frame| {
            frame.scale(self.data.scale);
            let coords: Vec<(f32, f32)> = self
                .data
                .nodes
                .iter()
                .map(|n| {
                    let (x, y) = n.pos;
                    (
                        (x + 1) as f32 * self.data.deltax + self.data.offsetx,
                        (y + 1) as f32 * self.data.deltay + self.data.offsety,
                    )
                })
                .collect();

            if let Some(name) = &state.over_node {
                if let Some(node) = self.data.nodes.iter().find(|n| &n.name == name) {
                    // highlight the row if it's selected
                    frame.fill_rectangle(
                        (
                            self.data.offsetx / 2.0,
                            coords[node.index].1 - self.data.deltay / 2.0,
                        )
                            .into(),
                        iced::Size::new(bounds.size().width - self.data.offsetx, self.data.deltay),
                        style.highlight,
                    );
                }
            }
            // Draw network lines
            for ((from, to), node) in self.data.edges.iter().zip(&self.data.nodes) {
                let line = Path::line(coords[*from].into(), coords[*to].into());
                frame.stroke(
                    &line,
                    Stroke::default()
                        .with_width(1.5)
                        .with_color(node.linecolor.unwrap_or(style.line)),
                );
            }

            for (node, pos) in self.data.nodes.iter().zip(coords) {
                let circle = Path::circle(pos.into(), node.size);
                frame.fill(&circle, node.color.unwrap_or(style.node));
                let mut txt = iced_graphics::geometry::Text::from(node.label.as_str());
                txt.position = (
                    self.data.offsetx + self.data.deltax * (self.data.maxlevel + 2) as f32,
                    pos.1,
                )
                    .into();
                txt.vertical_alignment = iced_core::alignment::Vertical::Center;
                txt.color = node.textcolor.unwrap_or(style.text);
                frame.fill_text(txt);
            }
        });

        renderer.with_translation(bounds.position() - Point::ORIGIN, |renderer| {
            use iced_graphics::geometry::Renderer as _;

            renderer.draw_geometry(geometry);
        });
    }
}

impl<'a, Message, Theme> From<NetworkTable<'a, Message, Theme>>
    for Element<'a, Message, Theme, Renderer>
where
    Message: 'a,
    Theme: Catalog + 'a,
{
    fn from(net_tbl: NetworkTable<'a, Message, Theme>) -> Self {
        Self::new(net_tbl)
    }
}

#[derive(Default)]
struct State {
    over_node: Option<String>,
    last_style: RefCell<Option<Style>>,
}

/// The appearance of a Network Table.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Style {
    /// The color of the Network Table nodes
    pub node: Color,
    /// The color of the Network Table lines
    pub line: Color,
    /// The color of the Network Table text
    pub text: Color,
    /// The color of the Highlighted row
    pub highlight: Color,
}

/// The theme catalog of a [`NetworkTable`].
pub trait Catalog {
    /// The item class of the [`Catalog`].
    type Class<'a>;

    /// The default class produced by the [`Catalog`].
    fn default<'a>() -> Self::Class<'a>;

    /// The [`Style`] of a class with the given status.
    fn style(&self, class: &Self::Class<'_>) -> Style;
}

/// A styling function for a [`NetworkTable`].
pub type StyleFn<'a, Theme> = Box<dyn Fn(&Theme) -> Style + 'a>;

impl Catalog for Theme {
    type Class<'a> = StyleFn<'a, Self>;

    fn default<'a>() -> Self::Class<'a> {
        Box::new(default)
    }

    fn style(&self, class: &Self::Class<'_>) -> Style {
        class(self)
    }
}

/// The default style of a [`NetworkTable`].
pub fn default(theme: &Theme) -> Style {
    let palette = theme.palette();

    Style {
        node: palette.primary,
        line: palette.danger,
        highlight: palette.background.scale_alpha(0.5),
        text: palette.text,
    }
}
