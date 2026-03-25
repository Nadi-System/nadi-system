//! Network display in the form of table.
use iced::{Event, Renderer};
use iced_core::renderer::{self, Renderer as _};
use iced_core::widget::tree::{self, Tree};
use iced_core::{
    Clipboard, Color, Element, Layout, Length, Point, Rectangle, Shell, Size, Theme, Widget,
};
use iced_core::{layout, mouse};
use iced_graphics::geometry::{Frame, Path, Stroke};
use std::cell::RefCell;

mod dtypes;
pub use dtypes::{NetworkData, NetworkDataView, NetworkViewType};

#[allow(missing_debug_implementations)]
pub struct NetworkTable<'a, Message, Theme = iced::Theme>
where
    Theme: Catalog,
{
    data: &'a NetworkDataView,
    on_press: Option<Box<dyn Fn(Option<String>) -> Message + 'a>>,
    class: Theme::Class<'a>,
}

impl<'a, Message, Theme> NetworkTable<'a, Message, Theme>
where
    Theme: Catalog,
{
    /// Creates a new [`NetworkTable`] with the provided [`Data`].
    pub fn new(data: &'a NetworkDataView) -> Self {
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
        &mut self,
        _tree: &mut Tree,
        _renderer: &Renderer,
        limits: &layout::Limits,
    ) -> layout::Node {
        let conf = &self.data.config;

        let size = match self.data.network.ty {
            NetworkViewType::Flat => {
                let mut x =
                    (self.data.network.maxlevel + 2) as f32 * conf.deltax + conf.offsetx * 2.0;
                let num_chars = self
                    .data
                    .network
                    .nodes
                    .iter()
                    .map(|n| n.label.len())
                    .max()
                    .unwrap_or_default() as f32;
                x += num_chars * 15.0; // TODO find text length required to draw it
                let y =
                    (self.data.network.nodes.len() + 2) as f32 * conf.deltay + conf.offsety * 3.0;
                let xmax = limits.max().width;
                let xnew = x * self.data.scale;
                Size::new(xmax.max(xnew), y * self.data.scale)
            }
            NetworkViewType::Tree => {
                let x = (self.data.network.maxorder + 2) as f32 * conf.deltay + conf.offsety * 3.0;
                let y = (self.data.network.weight + 2) as f32 * conf.deltax + conf.offsetx * 2.0;
                Size::new(x * self.data.scale, y * self.data.scale)
            }
        };
        layout::Node::new(size)
    }

    // might have to use this for hover effect
    fn update(
        &mut self,
        tree: &mut Tree,
        event: &Event,
        layout: Layout<'_>,
        cursor: mouse::Cursor,
        _renderer: &Renderer,
        _clipboard: &mut dyn Clipboard,
        shell: &mut Shell<'_, Message>,
        _viewport: &Rectangle,
    ) {
        let conf = &self.data.config;

        let state = tree.state.downcast_mut::<State>();

        if state.last_scale != self.data.scale {
            self.data.cache.clear();
            state.last_scale = self.data.scale;
        }

        let node = cursor
            .position_in(layout.bounds())
            .and_then(|pt| match self.data.network.ty {
                NetworkViewType::Flat => {
                    let y = ((pt.y - conf.offsety * self.data.scale)
                        / (conf.deltay * self.data.scale)
                        - 1.0)
                        .round();
                    if y < 0.0 {
                        None
                    } else {
                        let index = y as usize;
                        self.data.network.nodes.get(index).map(|n| {
                            let y = (y + 0.5) * conf.deltay * self.data.scale
                                + conf.offsety * self.data.scale;
                            OverNode {
                                index,
                                name: n.name.to_string(),
                                pos: (conf.offsetx, y),
                                size: (
                                    layout.bounds().width - conf.offsetx,
                                    conf.deltay * self.data.scale,
                                ),
                            }
                        })
                    }
                }
                NetworkViewType::Tree => 'tree: {
                    let x = ((pt.x - conf.offsetx * self.data.scale)
                        / (conf.deltax * self.data.scale))
                        .round()
                        - 1.0;
                    if x < 0.0 || x > self.data.network.maxorder as f32 {
                        break 'tree None;
                    }
                    let y = (pt.y - conf.offsety * self.data.scale)
                        / (conf.deltay * self.data.scale)
                        - 0.5;
                    if y < 0.0 || y > self.data.network.weight as f32 {
                        break 'tree None;
                    }
                    self.data
                        .network
                        .nodes
                        .iter()
                        .find(|n| n.pos.0 == x && (n.pos.1 + 1.0) > y && n.pos.1 < y)
                        .map(|n| {
                            let y1 = conf.offsety * self.data.scale
                                + (n.pos.1 + 0.5) * conf.deltay * self.data.scale;
                            let x1 = conf.offsetx * self.data.scale
                                + (x + 0.5) * conf.deltax * self.data.scale;
                            OverNode {
                                index: n.index,
                                name: n.name.to_string(),
                                pos: (x1, y1),
                                size: (
                                    conf.deltax * self.data.scale,
                                    conf.deltay * self.data.scale,
                                ),
                            }
                        })
                }
            });
        if state.over_node != node {
            state.over_node = node;
        }
        if let Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) = event
            && let Some(on_press) = &self.on_press
        {
            if let Some(node) = &state.over_node {
                shell.publish(on_press(Some(node.name.to_string())));
            } else if cursor.is_over(layout.bounds()) {
                shell.publish(on_press(None));
            }
        }
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

        let mut frame = Frame::new(renderer, bounds.size());
        frame.scale(self.data.scale);

        let conf = &self.data.config;

        if let Some(overnode) = &state.over_node {
            // highlight the row if it's selected
            frame.fill_rectangle(
                (overnode.pos.0, overnode.pos.1).into(),
                iced::Size::new(overnode.size.0, overnode.size.1),
                style.highlight,
            );
        }
        let highlight = frame.into_geometry();
        // Reuse cache if possible
        let geometry = self.data.cache.draw(renderer, bounds.size(), |frame| {
            frame.scale(self.data.scale);
            let coords: Vec<(f32, f32)> = self
                .data
                .network
                .nodes
                .iter()
                .map(|n| {
                    let (x, y) = n.pos;
                    (
                        (x + 1.0) * conf.deltax + conf.offsetx,
                        (y + 1.0) * conf.deltay + conf.offsety,
                    )
                })
                .collect();
            // Draw network lines
            for (from, to, color, width) in &self.data.network.edges {
                let line = Path::line(coords[*from].into(), coords[*to].into());
                frame.stroke(
                    &line,
                    Stroke::default()
                        .with_width(*width)
                        .with_color(color.unwrap_or(style.line)),
                );
            }

            for (node, pos) in self.data.network.nodes.iter().zip(coords) {
                let npath = node.path(pos);
                frame.fill(&npath, node.color.unwrap_or(style.node));
                if !self.data.network.hide_labels {
                    let mut txt = iced_graphics::geometry::Text::from(node.label.as_str());
                    txt.position = (
                        conf.offsetx + conf.deltax * (self.data.network.maxlevel + 2) as f32,
                        pos.1,
                    )
                        .into();
                    txt.align_y = iced_core::alignment::Vertical::Center;
                    txt.color = node.textcolor.unwrap_or(style.text);
                    frame.fill_text(txt);
                }
            }
        });

        renderer.with_translation(bounds.position() - Point::ORIGIN, |renderer| {
            use iced_graphics::geometry::Renderer as _;

            renderer.draw_geometry(highlight);
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

#[derive(Clone, Default, PartialEq)]
struct OverNode {
    index: usize,
    name: String,
    pos: (f32, f32),
    size: (f32, f32),
}

#[derive(Default)]
struct State {
    over_node: Option<OverNode>,
    last_style: RefCell<Option<Style>>,
    last_scale: f32,
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
