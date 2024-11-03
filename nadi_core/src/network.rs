use abi_stable::std_types::{RDuration, Tuple2};
use anyhow::Context;
use colored::Colorize;
use std::collections::HashMap;
use std::fmt::Debug;
use string_template_plus::{Render, RenderOptions, Template};

use std::path::Path;

use crate::attrs::{AttrMap, Attribute};
use crate::functions::Propagation;
use crate::node::{new_node, Node, NodeInner};
use crate::parser::parse_network;
use abi_stable::{
    std_types::{
        RHashMap,
        ROption::{self, RNone, RSome},
        RString, RVec,
    },
    StableAbi,
};

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
    nodes: RVec<RString>,
    /// Map of node names to the [`Node`]
    nodes_map: RHashMap<RString, Node>,
    /// Network Attributes
    attributes: AttrMap,
    /// Output [`Node`] of the network if present
    outlet: ROption<Node>,
    /// network is ordered based on input topology
    ordered: bool,
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

impl Network {
    // TODO import DOT format as well, or maybe make it work through plugin
    pub fn from_file<P: AsRef<Path>>(filename: P) -> anyhow::Result<Self> {
        let mut network = Self::default();
        let content =
            std::fs::read_to_string(filename).context("Error while accessing the network file")?;
        let (res, paths) = parse_network(&content)
            .map_err(|e| anyhow::Error::msg(e.to_string()))
            // .context("Error while parsing the network file")
	    ?;
        println!("{res}");
        for path in paths {
            if !network.nodes_map.contains_key(&path.start) {
                network.insert_node_by_name(&path.start);
            }
            if !network.nodes_map.contains_key(&path.end) {
                network.insert_node_by_name(&path.end);
            }

            let inp = network.node_by_name(&path.start).unwrap();
            let out = network.node_by_name(&path.end).unwrap();
            {
                inp.lock().set_output(out.clone());
                out.lock().add_input(inp.clone());
            }
        }
        network.reorder();
        network.set_levels();
        Ok(network)
    }

    pub fn load_attrs<P: AsRef<Path>>(&self, attr_dir: P) -> anyhow::Result<()> {
        self.nodes_map.iter().try_for_each(|Tuple2(name, node)| {
            // ignore the error on attribute read
            let attr_file = attr_dir.as_ref().join(format!("{}.toml", name));
            if attr_file.exists() && attr_file.is_file() {
                node.lock().load_attr(&attr_file)
            } else {
                Ok(())
            }
        })?;
        Ok(())
    }

    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().map(|n| &self.nodes_map[n])
    }

    pub fn edges(&self) -> impl Iterator<Item = (&Node, &Node)> + '_ {
        self.edges_ind().map(|(s, e)| {
            (
                &self.nodes_map[&self.nodes[s]],
                &self.nodes_map[&self.nodes[e]],
            )
        })
    }

    pub fn edges_str(&self) -> impl Iterator<Item = (&str, &str)> + '_ {
        self.edges_ind()
            .map(|(s, e)| (self.nodes[s].as_str(), self.nodes[e].as_str()))
    }

    pub fn edges_ind(&self) -> impl Iterator<Item = (usize, usize)> + '_ {
        self.nodes().filter_map(|n| {
            let n = n.lock();
            match n.output() {
                RSome(o) => Some((n.index(), o.lock().index())),
                RNone => None,
            }
        })
    }

    pub fn node_names(&self) -> impl Iterator<Item = &str> {
        self.nodes.iter().map(|n| n.as_str())
    }

    pub fn nodes_rev(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().rev().map(|n| &self.nodes_map[n])
    }

    pub fn nodes_count(&self) -> usize {
        self.nodes.len()
    }

    pub fn insert_node_by_name(&mut self, name: &str) {
        let node = new_node(self.nodes_count(), name);
        self.nodes_map.insert(name.into(), node);
        self.nodes.push(name.into());
    }

    pub fn node(&self, ind: usize) -> Option<&Node> {
        self.nodes.get(ind).map(|n| &self.nodes_map[n])
    }

    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes_map.get(name)
    }

    pub fn nodes_propagation(&self, prop: &Propagation) -> Vec<Node> {
        match prop {
            Propagation::Sequential | Propagation::InputsFirst => self.nodes().cloned().collect(),
            Propagation::Inverse | Propagation::OutputFirst => self.nodes_rev().cloned().collect(),
            // // since it is already ordered, we don't need to do this
            // Propagation::InputsFirst => {
            //     let mut all_nodes: Vec<&Node> = self.nodes().collect();
            //     let mut nodes = vec![];
            //     fn insert_node(n: &Node, nodes: &mut Vec<Node>) {
            //         let ni = n
            //             .try_lock_for(RDuration::from_secs(1))
            //             .expect("Lock failed for node, maybe branched network");
            //         if ni.inputs().is_empty() {
            //             nodes.push(n.clone());
            //         } else {
            //             for i in ni.inputs() {
            //                 insert_node(i, nodes);
            //             }
            //             nodes.push(n.clone());
            //         }
            //     }
            //     insert_node(self.outlet.as_ref().unwrap(), &mut nodes);
            //     nodes
            // }
            Propagation::List(n) => n.iter().map(|n| self.nodes_map[n].clone()).collect(),
            Propagation::Path(p) => self.nodes_path(p).unwrap_or_default(),
        }
    }

    pub fn nodes_path(&self, path: &StrPath) -> Option<Vec<Node>> {
        let start = self.node_by_name(path.start.as_str())?;
        let end = self.node_by_name(path.end.as_str())?;
        Some(self.nodes_path_safe(start.clone(), end.clone(), false))
    }

    /// Will return empty vec if the path doesn't exist
    pub fn nodes_path_safe(&self, start: Node, end: Node, strict: bool) -> Vec<Node> {
        // we'll assume the network is indexed based on order, small
        // indices are closer to outlet
        let (start, end) = if strict || (start.lock().index() > end.lock().index()) {
            (start, end)
        } else {
            (end, start)
        };
        let mut curr = start.clone();
        let mut path_nodes = vec![];
        let end_name = self.nodes[end.lock().index()].as_str();
        loop {
            path_nodes.push(curr.clone());
            if curr.lock().name() == end_name {
                break;
            }
            let tmp = if let RSome(o) = curr.lock().output() {
                o.clone()
            } else {
                // start doesn't reach end
                return vec![];
            };
            curr = tmp;
        }
        path_nodes
    }

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

    pub fn reorder(&mut self) {
        self.calc_order();
        self.outlet = self
            .node(0)
            .cloned()
            .map(|n| {
                let mut child = n.clone();
                loop {
                    let cc = child.lock().output().cloned();
                    match cc {
                        RSome(c) => child = c.clone(),
                        RNone => break,
                    }
                }
                child
            })
            .into();
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
            let inps = node.inputs();
            if !inps.is_empty() {
                recc_set(&inps[0], level);
            }
            if inps.len() > 1 {
                for i in &inps[1..] {
                    recc_set(i, level + 1);
                }
            }
        }
        if let RSome(output) = &self.outlet {
            recc_set(output, 0);
        }
    }

    /// move the network outlet to the given node, discard all nodes
    /// not leading to the outlet
    pub fn move_outlet(&mut self, name: &str) -> Result<(), String> {
        todo!()
    }

    pub fn set_attr(&mut self, name: &str, val: Attribute) {
        self.attributes.insert(name.into(), val);
    }

    pub fn attr(&self, name: &str) -> Option<&Attribute> {
        self.attributes.get(name)
    }

    pub fn attrs(&self) -> &AttrMap {
        &self.attributes
    }

    pub fn render(&self, template: &Template) -> anyhow::Result<String> {
        let mut op = RenderOptions::default();
        let used_vars = template.parts().iter().flat_map(|p| p.variables());
        for var in used_vars {
            if let Some(val) = self.attr(var) {
                op.variables.insert(var.to_string(), val.to_string());
            }
            if let Some(val) = var.strip_prefix('_') {
                if let Some(Attribute::String(s)) = self.attr(val) {
                    op.variables.insert(var.to_string(), s.to_string());
                }
            }
        }
        template.render(&op)
    }

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

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct StrPath {
    pub start: RString,
    pub end: RString,
    attributes: ROption<AttrMap>,
}

impl ToString for StrPath {
    fn to_string(&self) -> String {
        if let RSome(ref a) = &self.attributes {
            format!(
                "{} -> {} [{}]",
                self.start,
                self.end,
                a.iter()
                    .map(|Tuple2(k, v)| format!("{}={}", k, v.to_string()))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        } else {
            format!("{} -> {}", self.start, self.end)
        }
    }
}

impl StrPath {
    pub fn new(start: RString, end: RString) -> Self {
        Self {
            start,
            end,
            attributes: RNone,
        }
    }

    pub fn to_colored_string(&self) -> String {
        format!(
            "{} -> {}",
            self.start.to_string().green(),
            self.end.to_string().green()
        )
    }
}

fn compare_node_order(n1: &Node, n2: &Node) -> std::cmp::Ordering {
    n1.lock().order().partial_cmp(&n2.lock().order()).unwrap()
}
