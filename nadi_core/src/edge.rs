use crate::attrs::AttrMap;
use crate::network::Network;
use crate::node::NodeInner;
use abi_stable::{
    std_types::{RHashMap, RString},
    StableAbi,
};

/// Edge object to save attributes
#[repr(C)]
#[derive(Debug, Clone, StableAbi, Hash, Eq, PartialEq)]
pub struct Edge {
    pub(crate) from: RString,
    pub(crate) to: RString,
}

impl Edge {
    pub fn new(n1: &str, n2: &str) -> Self {
        Self {
            from: n1.to_string().into(),
            to: n2.to_string().into(),
        }
    }
}

pub type EdgeAttrMap = RHashMap<Edge, AttrMap>;
// pub type EdgeAttrMap = RHashMap<RString, RHashMap<RString, AttrMap>>;

impl NodeInner {
    // implement methods to list out edges, as well as get edge attr

    // We should have edge attribute return edge names by default. while edge.sth return edge attributes, currently it returns attributes of the node on the other side. So we could make a syntax that makes it return the attribute of the specific edge instead.

    // Personally I think we should replace the return of nodes' attribute the edge attribute, because we don't have a situation where we need edge nodes, if we do users can use inputs+outputs for that.

    // Also make inputs/outputs etc just return node names by default (or Node object, displays name), that way node is also a value that can be passed around to functions... In that case we need to make functions have node receivers. That could be powerful but hard to implement for now.

    // I also think we should remove node path as a way to select nodes. Or maybe keep it only for tree networks.
}

impl Network {
    // should we save all edges in network or per node? network makes sense, but then we don't have access to edge attributes from nodes... unless we use context as well, I guess we can do that.
}

// Syntax:

// edge[x -> y]
// edges[ -> y]
// edges[x -> ]
// edges[node -> ]
// edges[ -> node]
// edgesmap
// edges[*x -> *y]

// Basically: match pattern, empty means all, node name means that node, `node` keyword means current node in context, `*x` means the node in the variable x. IDK if we want list of nodes, we can prob do comma separated list of nodes. Except for *x type syntax. We can prob make *x be a list by default, yeah, that works.

// IT's a good design, i don't think we need edges keyword for node edges anyway, and this is gr
