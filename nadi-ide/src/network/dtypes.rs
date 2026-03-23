use iced::Color;
use iced_graphics::geometry::Cache;
use iced_graphics::geometry::Path;
use nadi_core::graphics::color::Color as NadiColor;
use nadi_core::graphics::node::NodeShape;
use nadi_core::prelude::*;
use nadi_core::template::Template;

pub struct NodeData {
    pub index: usize,
    pub weight: u64,
    pub order: u64,
    pub name: String,
    pub size: f32,
    pub shape: NodeShape,
    pub pos: (f32, f32),
    pub color: Option<Color>,
    pub textcolor: Option<Color>,
    pub label: String,
}

pub(super) fn iced_color(c: NadiColor) -> Color {
    Color::from_rgb(c.r as f32 / 255.0, c.g as f32 / 255.0, c.b as f32 / 255.0)
}

impl NodeData {
    fn new(node: &NodeInner, label: &Option<Template>) -> Self {
        let size = node.node_size() as f32;
        let shape = node.node_shape();
        let color = node.maybe_node_color().map(iced_color);
        let textcolor = node.maybe_text_color().map(iced_color);
        // TODO load node.visual.nodelabel if not use network label provided
        let label = label
            .as_ref()
            .map(|t| t.render(node).unwrap_or(t.original().to_string()))
            .unwrap_or_else(|| node.name().to_string());
        Self {
            index: node.index(),
            order: node.order(),
            weight: node.weight(),
            name: node.name().to_string(),
            size,
            shape,
            pos: (node.level() as f32, node.index() as f32),
            color,
            textcolor,
            label,
        }
    }

    pub fn path(&self, pos: (f32, f32)) -> Path {
        let size = self.size;
        match self.shape {
            NodeShape::Square => {
                let x = pos.0 - size;
                let y = pos.1 - size;
                Path::rectangle((x, y).into(), (2.0 * size, 2.0 * size).into())
            }
            NodeShape::Rectangle(r) => {
                let r = r.abs() as f32;
                let (sizex, sizey) = if r > 1.0 {
                    (size / r, size)
                } else {
                    (size, size * r)
                };
                let x = pos.0 - sizex;
                let y = pos.1 - sizey;
                Path::rectangle((x, y).into(), (2.0 * sizex, 2.0 * sizey).into())
            }
            NodeShape::Circle => Path::circle(pos.into(), size),
            NodeShape::Ellipse(r) => {
                let r = r.abs() as f32;
                let (sizex, sizey) = if r > 1.0 {
                    (size / r, size)
                } else {
                    (size, size * r)
                };
                Path::new(|b| {
                    b.ellipse(iced::widget::canvas::path::arc::Elliptical {
                        center: pos.into(),
                        radii: [sizex, sizey].into(),
                        rotation: 0.into(),
                        start_angle: 0.into(),
                        end_angle: std::f32::consts::TAU.into(),
                    });
                    b.close();
                })
            }
            NodeShape::Triangle => {
                let ht = 2.0 * 0.8660 * size;
                let dx = size;
                let points = [
                    (pos.0 - dx, pos.1 + ht / 3.0),
                    (pos.0, pos.1 - 2.0 * ht / 3.0),
                    (pos.0 + dx, pos.1 + ht / 3.0),
                ];
                Path::new(|b| {
                    b.move_to(points[0].into());
                    b.line_to(points[1].into());
                    b.line_to(points[2].into());
                    b.close();
                })
            }
            NodeShape::IsoTriangle(r) => {
                let ht = 2.0 * 0.8660 * size;
                let dx = size;
                let r = r.abs() as f32;
                let (ht, dx) = if r > 1.0 { (ht / r, dx) } else { (ht, dx * r) };
                let points = [
                    (pos.0 - dx, pos.1 + ht / 3.0),
                    (pos.0, pos.1 - 2.0 * ht / 3.0),
                    (pos.0 + dx, pos.1 + ht / 3.0),
                ];
                Path::new(|b| {
                    b.move_to(points[0].into());
                    b.line_to(points[1].into());
                    b.line_to(points[2].into());
                    b.close();
                })
            }
        }
    }
}

#[derive(Default)]
pub struct NetworkData {
    pub nodes: Vec<NodeData>,
    pub edges: Vec<(usize, usize, Option<Color>, f32)>,
    pub label: Option<Template>,
    pub hide_labels: bool,
    pub maxlevel: u64,
    pub weight: u64,
    pub maxorder: u64,
    pub ty: NetworkViewType,
}

pub struct NetworkDataView {
    pub network: NetworkData,
    pub config: TreeConfig,
    pub invert: bool,
    pub scale: f32,
    pub cache: Cache<iced::Renderer>,
}

impl Default for NetworkDataView {
    fn default() -> Self {
        Self {
            network: NetworkData::default(),
            config: TreeConfig::default(),
            invert: true,
            scale: 1.0,
            cache: Cache::<iced::Renderer>::new(),
        }
    }
}

#[derive(Default)]
pub enum NetworkViewType {
    #[default]
    Flat,
    Tree,
}

pub struct TreeConfig {
    pub deltax: f32,
    pub deltay: f32,
    pub offsetx: f32,
    pub offsety: f32,
    pub deltacol: f32,
}

impl Default for TreeConfig {
    fn default() -> Self {
        Self {
            deltax: 20.0,
            deltay: 20.0,
            offsetx: 20.0,
            offsety: 20.0,
            deltacol: 20.0,
        }
    }
}

fn position_nodes(nds: Vec<Node>, x: f32, mut y: f32, pos: &mut Vec<NodeData>) {
    for n in nds {
        let n = n.lock();
        let ind = n.index();
        let nd = &mut pos[ind];
        let wt = n.weight();
        nd.pos = (x, y + wt as f32 / 2.0);
        position_nodes(n.inputs().iter().cloned().collect(), x + 1.0, y, pos);
        y += wt as f32;
    }
}

impl NetworkData {
    pub fn new(net: &Network, ty: NetworkViewType) -> Self {
        // TODO read network.visual.nodelabel here
        let label: Option<Template> = None;
        let mut nodes = net
            .nodes()
            .map(|n| NodeData::new(&n.lock(), &label))
            .collect();

        let mut hide_labels = false;
        if let NetworkViewType::Tree = ty {
            position_nodes(net.outlets().cloned().collect(), 0.0, 0.0, &mut nodes);
            hide_labels = true;
        }
        let maxlevel = net.nodes().map(|n| n.lock().level()).max().unwrap_or(0);
        let weight = net.outlets().map(|n| n.lock().weight()).sum();
        let maxorder = net.nodes().next().map(|n| n.lock().order()).unwrap_or(0);
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output()
                    .map(|o| {
                        (
                            n.index(),
                            o.lock().index(),
                            n.maybe_line_color().map(iced_color),
                            n.line_width() as f32,
                        )
                    })
                    .into()
            })
            .collect();

        Self {
            nodes,
            edges,
            label,
            maxlevel,
            weight,
            maxorder,
            hide_labels,
            ty,
        }
    }

    pub fn update(&mut self, net: &Network, ty: NetworkViewType) {
        let label: Option<Template> = None;
        let mut nodes = net
            .nodes()
            .map(|n| NodeData::new(&n.lock(), &label))
            .collect();
        let maxlevel = net.nodes().map(|n| n.lock().level()).max().unwrap_or(0);
        let weight = net.outlets().map(|n| n.lock().weight()).sum();
        let maxorder = net.nodes().next().map(|n| n.lock().order()).unwrap_or(0);
        if let NetworkViewType::Tree = ty {
            position_nodes(net.outlets().cloned().collect(), 0.0, 0.0, &mut nodes);
            self.hide_labels = true;
        } else {
            self.hide_labels = false;
        }
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output()
                    .map(|o| {
                        (
                            n.index(),
                            o.lock().index(),
                            n.maybe_line_color().map(iced_color),
                            n.line_width() as f32,
                        )
                    })
                    .into()
            })
            .collect();
        self.label = label;
        self.nodes = nodes;
        self.edges = edges;
        self.maxlevel = maxlevel;
        self.weight = weight;
        self.maxorder = maxorder;
        self.ty = ty;
    }
}

impl NetworkDataView {
    pub fn update(&mut self, netdata: NetworkData) {
        self.network = netdata;
        self.cache.clear();
    }
}
