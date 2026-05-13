use crate::network::Network;

/// Different ways to position nodes in a network
pub enum NetworkLayout {
    /// Create a tree (only valid if the network itself is a tree)
    Tree,
    /// Place all nodes in a circle
    Circle,
    /// Custom attribute
    Custom(String),
}

pub struct LayoutSpace {
    height: f32,
    width: f32,
    deltah: f32,
    deltav: f32,
}

impl NetworkLayout {
    pub fn apply(&self, net: &Network, space: &LayoutSpace) {
        match self {
            Self::Tree => tree_layout(net),
            Self::Circle => circle_layout(net),
            Self::Custom(attr) => custom_layout(net, &attr),
        }
    }
}

fn tree_layout(net: &Network) {
    todo!()
}

fn circle_layout(net: &Network, space: &LayoutSpace) {
    let nc = net.nodes().len();
    let angle = 2.0 * std::f32::consts::PI / nc;
    let (x, y) = (space.height / 2.0, space.width / 2.0);
    let r = (space.width / 2.0 - space.deltah).min(space.height / 2.0 - space.deltav);
    for (i, n) in net.nodes().enumerate() {
        let m = x + r * (angle * i).cos();
        let n = y + r * (angle * i).sin();
        n.lock().set_pos(m, n);
    }
}

fn custom_layout(net: &Network, attr: &str) {
    todo!()
}
