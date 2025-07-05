use crate::attrs::{AttrMap, HasAttributes};
use crate::expressions::{EvalError, Expression};
use crate::node::{new_node, Node, NodeInner};
use crate::timeseries::{HasSeries, HasTimeSeries, SeriesMap, TsMap};
use abi_stable::std_types::RDuration;
use abi_stable::{
    std_types::{
        RHashMap,
        ROption::{self, RNone, RSome},
        RString, RVec,
    },
    StableAbi,
};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

pub const ROOT_NODE_NAME: &str = "*ROOT*";

/// Network is a collection of Nodes, with Connection information. The
/// connection information is saved in the nodes itself (`inputs` and
/// `output` variables), but they are assigned from the network.
///
/// The nadi system (lit, river system), is designed for the
/// connections between points along a river. Out of different types
/// of river networks possible, it can only handle non-branching
/// tributaries system, where each point can have zero to multiple
/// inputs, but can only have one output. Overall the system should
/// have a single output point. There can be branches in the river
/// itself in the physical sense as long as they converse before the
/// next point of interests. There cannot be node points that have
/// more than one path to reach another node in the representative
/// system.
///
/// Here is an example network file,
/// ```network
///     cannelton -> newburgh
///     newburgh -> evansville
///     evansville -> "jt-myers"
///     "jt-myers" -> "old-shawneetown"
///     "old-shawneetown" -> golconda
///     markland -> mcalpine
///     golconda -> smithland
/// ```
/// The basic form of network file can contain a connection per line,
/// the node names can either be identifier (alphanumeric+_) or a
/// quoted string (similar to [DOT format (graphviz
/// package)](https://graphviz.org/doc/info/lang.html)). Network file
/// without any connection format can be written as a node per line,
/// but those network can only call sequential functions, and not
/// input dependent ones.
///
/// Depending on the use cases, it can probably be applied to other
/// systems that are similar to a river system. Or even without the
/// connection information, the functions that are independent to each
/// other can be run in sequential order.
#[repr(C)]
#[derive(StableAbi, Default, Clone)]
pub struct Network {
    /// List of [`Node`]s
    pub(crate) nodes: RVec<RString>,
    /// Map of node names to the [`Node`]
    pub(crate) nodes_map: RHashMap<RString, Node>,
    /// Network Attributes
    pub(crate) attributes: AttrMap,
    /// Network Series
    pub(crate) series: SeriesMap,
    /// Network TimeSeries
    pub(crate) timeseries: TsMap,
    /// Output [`Node`] of the network if present
    pub(crate) outlet: ROption<Node>,
    /// network is ordered based on input topology
    pub(crate) ordered: bool,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Network")
            .field("nodes", &self.nodes)
            .field("attributes", &self.attributes)
            .field(
                "outlet",
                &self.outlet.clone().map(|o| o.lock().name().to_string()),
            )
            .field("ordered", &self.ordered)
            .finish()
    }
}

impl HasAttributes for Network {
    fn attr_map(&self) -> &AttrMap {
        &self.attributes
    }

    fn attr_map_mut(&mut self) -> &mut AttrMap {
        &mut self.attributes
    }
}

impl HasSeries for Network {
    fn series_map(&self) -> &SeriesMap {
        &self.series
    }

    fn series_map_mut(&mut self) -> &mut SeriesMap {
        &mut self.series
    }
}

impl HasTimeSeries for Network {
    fn ts_map(&self) -> &TsMap {
        &self.timeseries
    }

    fn ts_map_mut(&mut self) -> &mut TsMap {
        &mut self.timeseries
    }
}

impl Network {
    /// Iterator for the nodes in the network
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().map(|n| &self.nodes_map[n])
    }

    /// Iterator for the edges of the network
    pub fn edges(&self) -> impl Iterator<Item = (&Node, &Node)> + '_ {
        self.edges_ind().map(|(s, e)| {
            (
                &self.nodes_map[&self.nodes[s]],
                &self.nodes_map[&self.nodes[e]],
            )
        })
    }

    /// Outlet of the network (node with no output)
    pub fn outlet(&self) -> Option<&Node> {
        self.outlet.as_ref().into()
    }

    /// Append the edges from the list, making new nodes if necessary
    pub fn append_edges(&mut self, edges: &[(&str, &str)]) -> Result<(), String> {
        for (start, end) in edges {
            if !self.nodes_map.contains_key(*start) {
                self.insert_node_by_name(start);
            }
            if !self.nodes_map.contains_key(*end) {
                self.insert_node_by_name(end);
            }
            let inp = self.node_by_name(start).unwrap();
            let out = self.node_by_name(end).unwrap();
            {
                if let RSome(n) = inp.lock().set_output(out.clone()) {
                    return Err(format!(
                        "Node {:?} already has {:?} as output (new: {:?})",
                        start,
                        n.lock().name(),
                        end
                    ));
                }
                out.lock().add_input(inp.clone());
            }
        }
        self.reorder();
        self.set_levels();
        Ok(())
    }

    /// Create a network with given edges
    pub fn from_edges(edges: &[(&str, &str)]) -> Result<Self, String> {
        let mut network = Self::default();
        network.append_edges(edges)?;
        Ok(network)
    }

    /// Iterator of the edges with nodes' names
    pub fn edges_str(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.edges_ind()
            .map(|(s, e)| (self.nodes[s].as_str(), self.nodes[e].as_str()))
    }

    /// Iterator of the edges with node index
    pub fn edges_ind(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.nodes().filter_map(|n| {
            let n = n.lock();
            match n.output() {
                RSome(o) => Some((n.index(), o.lock().index())),
                RNone => None,
            }
        })
    }

    /// Iterator of node names
    pub fn node_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|n| n.as_str())
    }

    /// Nodes iterator in reverse order
    pub fn nodes_rev(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().rev().map(|n| &self.nodes_map[n])
    }

    /// Number of nodes in the network
    pub fn nodes_count(&self) -> usize {
        self.nodes.len()
    }

    /// Insert a new node by its name
    pub fn insert_node_by_name(&mut self, name: &str) {
        if self.nodes_map.contains_key(name) {
            return;
        }
        let node = new_node(self.nodes_count(), name);
        self.nodes_map.insert(name.into(), node);
        self.nodes.push(name.into());
    }

    /// Get a node by index
    pub fn node(&self, ind: usize) -> Option<&Node> {
        self.nodes.get(ind).map(|n| &self.nodes_map[n])
    }

    /// Get a node by name
    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes_map.get(name)
    }

    /// Get a node by name (with error msg on failure)
    pub fn try_node_by_name(&self, name: &str) -> Result<&Node, EvalError> {
        self.nodes_map
            .get(name)
            .ok_or_else(|| EvalError::NodeNotFound(name.to_string()))
    }

    /// Get nodes in the given order
    pub fn nodes_order(&self, prop: &PropOrder) -> Vec<Node> {
        match prop {
            PropOrder::Auto | PropOrder::Sequential | PropOrder::OutputFirst => {
                self.nodes().cloned().collect()
            }
            PropOrder::Inverse | PropOrder::InputsFirst => self.nodes_rev().cloned().collect(),
        }
    }

    /// Get nodes in the given order and selection
    pub fn nodes_select(
        &self,
        order: &PropOrder,
        prop: &PropNodes,
    ) -> Result<Vec<Node>, EvalError> {
        match prop {
            PropNodes::All => Ok(self.nodes_order(order)),
            PropNodes::List(lst) => {
                let mut sel_lst: HashSet<&str> = lst.iter().map(|n| n.as_str()).collect();
                let res = self
                    .nodes_order(order)
                    .into_iter()
                    .filter(|n| sel_lst.remove(n.lock().name()))
                    .collect();
                if sel_lst.is_empty() {
                    Ok(res)
                } else {
                    Err(EvalError::NodeNotFound(
                        sel_lst.into_iter().collect::<Vec<&str>>().join(", "),
                    ))
                }
            }
            PropNodes::Path(p) => self.nodes_path(order, p),
        }
    }

    /// Get a list of nodes in the given path
    pub fn nodes_path(&self, order: &PropOrder, path: &StrPath) -> Result<Vec<Node>, EvalError> {
        let start = self.try_node_by_name(path.start.as_str())?;
        let end = self.try_node_by_name(path.end.as_str())?;
        // we'll assume the network is indexed based on order, small
        // indices are closer to outlet; and resuffle the nodes
        let (start, end, flipped) = if start.lock().index() > end.lock().index() {
            (start, end, false)
        } else {
            (end, start, true)
        };
        let mut curr = start.clone();
        let mut path_nodes = vec![];
        let start_name = self.nodes[start.lock().index()].as_str();
        let end_name = self.nodes[end.lock().index()].as_str();
        loop {
            path_nodes.push(curr.clone());
            if curr.lock().name() == end_name {
                break;
            }
            let tmp = if let RSome(o) = curr.lock().output() {
                o.clone()
            } else {
                return Err(EvalError::PathNotFound(
                    start_name.to_string(),
                    curr.lock().name().to_string(),
                    end_name.to_string(),
                ));
            };
            curr = tmp;
        }
        match order {
            PropOrder::Auto if flipped => Ok(path_nodes.into_iter().rev().collect()),
            PropOrder::Auto => Ok(path_nodes),
            PropOrder::Sequential | PropOrder::OutputFirst => Ok(path_nodes),
            PropOrder::Inverse | PropOrder::InputsFirst => {
                Ok(path_nodes.into_iter().rev().collect())
            }
        }
    }

    /// Calculate the order of all nodes
    ///
    /// Value of order signifies the number of all nodes (recursively)
    /// that are on the input side of the node
    pub fn calc_order(&mut self) {
        let _all_nodes: Vec<RString> = self.nodes.to_vec();
        let _order_queue: Vec<RString> = Vec::with_capacity(self.nodes.len());

        let mut orders = HashMap::<String, u64>::with_capacity(self.nodes.len());

        fn get_set_ord(node: &NodeInner, orders: &mut HashMap<String, u64>) -> u64 {
            orders.get(node.name()).copied().unwrap_or_else(|| {
                let mut ord = 1;
                for i in node.inputs() {
                    ord += get_set_ord(
                        &i.try_lock_for(RDuration::from_secs(1))
                            .expect("Lock failed for node, maybe branched network"),
                        orders,
                    );
                }
                orders.insert(node.name().to_string(), ord);
                ord
            })
        }

        for node in self.nodes() {
            let mut ni = node
                .try_lock_for(RDuration::from_secs(1))
                .expect("Lock failed for node, maybe branched network");
            let ord = get_set_ord(&ni, &mut orders);
            ni.set_order(ord);
        }
    }

    /// Reorder the nodes in the network
    pub fn reorder(&mut self) {
        self.calc_order();
        self.outlet = {
            let outlets: Vec<Node> = self
                .nodes()
                .filter(|n| n.lock().output().is_none())
                .map(|o| o.clone())
                .collect();
            match &outlets[..] {
                [] => RNone,
                [o] => RSome(o.clone()),
                outs => {
                    eprintln!("Multiple Outlet Nodes found; adding a *ROOT* node");
                    self.insert_node_by_name(ROOT_NODE_NAME);
                    let outlet = self.node_by_name(ROOT_NODE_NAME).expect("Just inserted");
                    for o in outs {
                        o.lock().set_output(outlet.clone());
                        outlet.lock().add_input(o.clone());
                    }
                    RSome(outlet.clone())
                }
            }
        };
        let mut new_nodes: Vec<Node> = Vec::with_capacity(self.nodes.len());
        fn insert_node(nv: &mut Vec<Node>, n: Node) {
            nv.push(n.clone());
            let mut inps: Vec<Node> = n.lock().inputs().to_vec();
            inps.sort_by(compare_node_order);
            for c in inps {
                insert_node(nv, c);
            }
        }
        if let RSome(out) = &self.outlet {
            insert_node(&mut new_nodes, out.clone());
        }
        if new_nodes.len() < self.nodes.len() {
            // todo, make the nodes into different groups?
            eprintln!(
                "Reorder not done, the nodes are not connected: {} connected out of {}",
                new_nodes.len(),
                self.nodes.len()
            );
            self.ordered = false;
            return;
        }
        self.nodes = new_nodes
            .iter()
            .map(|n| n.lock().name().into())
            .collect::<Vec<RString>>()
            .into();
        self.reindex();
        self.ordered = true;
    }

    /// reindex the nodes in the network
    pub fn reindex(&self) {
        for (i, n) in self.nodes().enumerate() {
            n.lock().set_index(i);
        }
    }

    /// sets the levels for the nodes, 0 means it's the main branch and
    /// increasing number is for tributories level
    pub fn set_levels(&mut self) {
        fn recc_set(node: &Node, level: u64) {
            node.lock().set_level(level);
            node.lock().order_inputs();
            let node = node.lock();
            let mut inps = node.inputs().iter();
            if let Some(i) = inps.next() {
                recc_set(i, level);
            }
            for i in inps {
                recc_set(i, level + 1);
            }
        }
        if let RSome(output) = &self.outlet {
            recc_set(output, 0);
        }
    }

    /// Remove a single node from the network
    ///
    /// This will remove the node, while making all the input nodes
    /// now goto the output node of the removed node. If it doesn't
    /// have a output node, and there is more than one input nodes,
    /// then the resulting network is no longer a directed tree, the
    /// function will print a warning.
    fn remove_node_single(&mut self, node: &Node) {
        let (ind, out) = {
            let n = node.try_lock().expect("mutex problem 1");
            let ind = n.index();
            self.nodes.remove(ind);
            self.nodes_map.remove(n.name());
            // make sure the block below doesn't hang for long
            (ind, n.output().map(|o| o.clone()))
        };
        if let RSome(out) = out {
            let pos = out
                .try_lock()
                .expect("mutex problem 2")
                .inputs()
                .iter()
                .position(|i| i.try_lock().expect("mutex problem 3").index() == ind)
                .expect("Node should be in input list of output");
            out.try_lock()
                .expect("mutex problem 4")
                .inputs_mut()
                .remove(pos);
            for inp in node.lock().inputs() {
                inp.try_lock()
                    .expect("mutex problem 5")
                    .set_output(out.clone());
                out.try_lock()
                    .expect("mutex problem 6")
                    .add_input(inp.clone());
            }
        } else {
            for inp in node.lock().inputs() {
                inp.try_lock().expect("mutex problem 7").unset_output();
            }
            if node.lock().inputs().len() > 1 {
                eprintln!("WARN: Node with multiple inputs and no output Removed");
            }
        }
        self.reindex();
    }

    /// Remove a single node from the network
    ///
    /// This will remove the node, while making all the input nodes
    /// now goto the output node of the removed node. If it doesn't
    /// have a output node, and there is more than one input nodes,
    /// then the resulting network is no longer a directed tree, the
    /// function will print a warning.
    pub fn remove_node(&mut self, node: &Node) {
        self.remove_node_single(node);
        self.reorder();
        self.set_levels();
    }

    /// Subset the network into a new network by removing a bunch of nodes
    pub fn subset(&mut self, filter: &[bool], keep: bool) -> Result<(), String> {
        let remove_nodes: Vec<Node> = self
            .nodes()
            .zip(filter)
            .filter(|(_, &f)| f ^ keep)
            .map(|n| n.0.clone())
            .collect();
        for node in remove_nodes {
            self.remove_node_single(&node);
        }
        self.reorder();
        self.set_levels();
        Ok(())
    }

    pub fn new_outlet(&mut self, node: Node) {
        let mut nodes = Vec::with_capacity(self.nodes.len());
        let mut nodes_map = HashMap::with_capacity(self.nodes.len());
        fn register(n: &Node, nds: &mut Vec<RString>, nmp: &mut HashMap<RString, Node>) {
            let nm: RString = n.lock().name().to_string().into();
            nds.push(nm.clone());
            nmp.insert(nm, n.clone());
            for i in n.lock().inputs() {
                register(i, nds, nmp)
            }
        }
        register(&node, &mut nodes, &mut nodes_map);
        self.nodes = nodes.into();
        self.nodes_map = nodes_map.into();
        self.outlet = RSome(node);
        self.reorder();
        self.set_levels();
    }

    /// get the connections in utf8 string to print in terminal
    pub fn connections_utf8(&self) -> Vec<String> {
        self.nodes()
            .map(|node| {
                let node = node.lock();
                let level = node.level();
                let par_level = node.output().map(|n| n.lock().level()).unwrap_or(level);
                let _merge = level != par_level;

                let mut line = String::new();
                for _ in 0..level {
                    line.push_str("  │");
                }
                if level != par_level {
                    line.pop();
                    if node.inputs().is_empty() {
                        line.push_str("├──");
                    } else {
                        line.push_str("├──┐");
                    }
                } else if node.inputs().is_empty() {
                    line.push_str("  ╵");
                } else if node.output().is_none() {
                    line.push_str("  ╷");
                } else {
                    line.push_str("  │");
                }
                line
            })
            .collect()
    }

    /// get the connections in ascii string to print in terminal
    pub fn connections_ascii(&self) -> Vec<String> {
        self.nodes()
            .map(|node| {
                let node = node.lock();
                let level = node.level();
                let par_level = node.output().map(|n| n.lock().level()).unwrap_or(level);
                let _merge = level != par_level;

                let mut line = String::new();
                for _ in 0..level {
                    line.push_str("  |");
                }
                if level != par_level {
                    line.pop();
                    line.push_str("|--*");
                // this is never needed as the first child is put in the same level
                // line.push_str("`--*");
                } else {
                    line.push_str("  *");
                }
                line
            })
            .collect()
    }
}

/// Path with start and end node
#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct StrPath {
    pub start: RString,
    pub end: RString,
}

impl std::fmt::Display for StrPath {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "{} -> {}", self.start, self.end)
    }
}

impl StrPath {
    /// new path
    pub fn new(start: RString, end: RString) -> Self {
        Self { start, end }
    }

    /// Make a string with colors to print in terminal
    pub fn to_colored_string(&self) -> String {
        format!(
            "{} -> {}",
            self.start.to_string().green(),
            self.end.to_string().green()
        )
    }
}

/// Propagation of the nodes in a network
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Propagation {
    /// order of the nodes
    pub order: PropOrder,
    /// List or path of nodes
    pub nodes: PropNodes,
    /// Condition to evaluate for selection of nodes
    pub condition: PropCondition,
}

impl std::fmt::Display for Propagation {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(fmt, "{}{}{}", self.order, self.nodes, self.condition)
    }
}

/// Propagation order for nodes in a network
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PropOrder {
    /// Automatically based on context
    #[default]
    Auto,
    /// Sequential order (index: 0,1,2,...)
    Sequential,
    /// Inverse of the sequential
    Inverse,
    /// Input nodes before the output node
    InputsFirst,
    /// output node before the inputs node
    OutputFirst,
}

impl std::fmt::Display for PropOrder {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::Auto => Ok(()),
            Self::Sequential => write!(fmt, "<sequential>"),
            Self::Inverse => write!(fmt, "<inverse>"),
            Self::InputsFirst => write!(fmt, "<inputsfirst>"),
            Self::OutputFirst => write!(fmt, "<outputfirst>"),
        }
    }
}

/// List of nodes in a network
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PropNodes {
    /// No selection (all nodes)
    #[default]
    All,
    /// List of nodes by their name
    List(RVec<RString>),
    /// Path between two nodes by their name
    Path(StrPath),
}

impl std::fmt::Display for PropNodes {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::All => Ok(()),
            Self::List(v) => write!(
                fmt,
                "[{}]",
                v.iter()
                    .map(|a| a.as_str())
                    .collect::<Vec<&str>>()
                    .join(", ")
            ),
            Self::Path(p) => write!(fmt, "[{}]", p),
        }
    }
}

/// Propagation condition for the nodes
#[derive(Debug, Default, Clone, PartialEq)]
pub enum PropCondition {
    /// No condition (all nodes)
    #[default]
    All,
    /// Expression to evaluate into a bool to check
    Expr(Expression),
    // TODO
    // Head(usize),
    // Tail(usize),
}

impl std::fmt::Display for PropCondition {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::All => Ok(()),
            Self::Expr(expr) => write!(fmt, "({})", expr.to_string()),
        }
    }
}

fn compare_node_order(n1: &Node, n2: &Node) -> std::cmp::Ordering {
    n1.lock().order().partial_cmp(&n2.lock().order()).unwrap()
}

/// Take any [`Node`] and create [`Network`] with it as the outlet.
impl From<Node> for Network {
    fn from(node: Node) -> Self {
        let mut net = Self::default();

        let mut nodes = vec![];
        fn insert_node(n: &Node, nodes: &mut Vec<Node>) {
            let ni = n
                .try_lock_for(RDuration::from_secs(1))
                .expect("Lock failed for node, maybe branched network");
            if ni.inputs().is_empty() {
                nodes.push(n.clone());
            } else {
                for i in ni.inputs() {
                    insert_node(i, nodes);
                }
                nodes.push(n.clone());
            }
        }
        insert_node(&node, &mut nodes);
        net.nodes_map = nodes
            .into_iter()
            .map(|n| {
                let name = RString::from(n.lock().name());
                (name, n)
            })
            .collect::<HashMap<RString, Node>>()
            .into();
        net.nodes = net.nodes_map.keys().cloned().collect::<Vec<_>>().into();
        net.outlet = RSome(node);
        net.reorder();
        net.set_levels();
        net
    }
}
