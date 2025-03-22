use iced::Color;
use iced_graphics::geometry::Cache;
use nadi_core::graphics::node::{LINE_COLOR, NODE_COLOR, NODE_SIZE, TEXT_COLOR};
use nadi_core::prelude::*;
use nadi_core::string_template::Template;

pub struct NodeData {
    pub index: usize,
    pub name: String,
    pub size: f32,
    pub pos: (u64, usize),
    pub color: Option<Color>,
    pub linecolor: Option<Color>,
    pub textcolor: Option<Color>,
    pub label: String,
}

impl NodeData {
    fn new(node: &NodeInner, label: &Option<Template>) -> Self {
        let size = node
            .attr(nadi_core::graphics::node::NODE_SIZE.0)
            .and_then(f64::from_attr_relaxed)
            .unwrap_or(nadi_core::graphics::node::NODE_SIZE.1) as f32;
        let color = node_color(node, nadi_core::graphics::node::NODE_COLOR.0);
        let linecolor = node_color(node, nadi_core::graphics::node::LINE_COLOR.0);
        let textcolor = node_color(node, nadi_core::graphics::node::TEXT_COLOR.0);
        let label = label
            .as_ref()
            .map(|t| node.render(t).unwrap_or(t.original().to_string()))
            .unwrap_or_else(|| node.name().to_string());
        Self {
            index: node.index(),
            name: node.name().to_string(),
            size,
            pos: (node.level(), node.index()),
            color,
            linecolor,
            textcolor,
            label,
        }
    }
}

fn node_color(node: &NodeInner, attr: &str) -> Option<Color> {
    node.try_attr::<nadi_core::graphics::color::AttrColor>(attr)
        .unwrap_or_default()
        .color()
        .ok()
        .map(|c| Color::new(c.r as f32, c.g as f32, c.b as f32, 1.0))
}

pub struct NetworkData {
    pub nodes: Vec<NodeData>,
    pub edges: Vec<(usize, usize)>,
    pub label: Option<Template>,
    pub maxlevel: u64,
    pub deltax: f32,
    pub deltay: f32,
    pub offsetx: f32,
    pub offsety: f32,
    pub deltacol: f32,
    pub invert: bool,
    pub scale: f32,
    pub cache: Cache<iced::Renderer>,
}

impl Default for NetworkData {
    fn default() -> Self {
        Self {
            nodes: vec![],
            edges: vec![],
            label: None,
            maxlevel: 0,
            deltax: 20.0,
            deltay: 20.0,
            offsetx: 20.0,
            offsety: 20.0,
            deltacol: 20.0,
            invert: true,
            scale: 1.0,
            cache: Cache::<iced::Renderer>::new(),
        }
    }
}

impl NetworkData {
    pub fn new(net: &Network, label: Option<Template>) -> Self {
        let nodes = net
            .nodes()
            .map(|n| NodeData::new(&n.lock(), &label))
            .collect();
        let maxlevel = net.nodes().map(|n| n.lock().level()).max().unwrap_or(0);
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output().map(|o| (n.index(), o.lock().index())).into()
            })
            .collect();

        Self {
            nodes,
            edges,
            label,
            maxlevel,
            ..Default::default()
        }
    }

    pub fn update(&mut self, net: &Network, label: Option<Template>) {
        let nodes = net
            .nodes()
            .map(|n| NodeData::new(&n.lock(), &label))
            .collect();
        let maxlevel = net.nodes().map(|n| n.lock().level()).max().unwrap_or(0);
        let edges = net
            .nodes()
            .filter_map(|n| {
                let n = n.lock();
                n.output().map(|o| (n.index(), o.lock().index())).into()
            })
            .collect();
        self.label = label;
        self.nodes = nodes;
        self.edges = edges;
        self.maxlevel = maxlevel;
        self.cache.clear();
    }
}
