use crate::attrs::AttrMap;
use crate::node::NodeInner;
use abi_stable::{
    std_types::{RHashMap, RString},
    StableAbi,
};

/// Edge object to save attributes
#[repr(C)]
#[derive(Clone, StableAbi, Hash, Eq, PartialEq)]
pub struct Edge {
    from: RString,
    to: RString,
}

pub type EdgeAttrMap = RHashMap<Edge, AttrMap>;

impl NodeInner {
    // implement methods to list out edges, as well as get edge attr

    // We should have edge attribute return edge names by default. while edge.sth return edge attributes, currently it returns attributes of the node on the other side. So we could make a syntax that makes it return the attribute of the specific edge instead.

    // Personally I think we should replace the return of nodes' attribute the edge attribute, because we don't have a situation where we need edge nodes, if we do users can use inputs+outputs for that.

    // Also make inputs/outputs etc just return node names by default (or Node object, displays name), that way node is also a value that can be passed around to functions... In that case we need to make functions have node receivers. That could be powerful but hard to implement for now.

    // I also think we should remove node path as a way to select nodes. Or maybe keep it only for tree networks.
}
