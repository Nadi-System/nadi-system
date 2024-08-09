use crate::node::{new_node, Node};
use anyhow::Context;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

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
///
///     cannelton -> newburgh
///     newburgh -> evansville
///     evansville -> jt-myers
///     jt-myers -> old-shawneetown
///     old-shawneetown -> golconda
///     markland -> mcalpine
///     golconda -> smithland
///
/// The basic form of network file can contain a connection per
/// line. The program also supports the connection import from the
/// [DOT format (graphviz
/// package)](https://graphviz.org/doc/info/lang.html). Network file
/// without any connection format can be written as a node per line,
/// but those network can only call sequential functions, and not
/// input dependent ones.
///
/// Depending on the use cases, it can probably be applied to other
/// systems that are similar to a river system. Or even without the
/// connection information, the functions that are independent to each
/// other can be run in sequential order.
#[derive(Default, Clone)]
pub struct Network {
    /// List of [`Node`]s
    nodes: Vec<Node>,
    /// Map of node names to the [`Node`]
    nodes_map: HashMap<String, Node>,
    /// Output [`Node`] of the network if present
    output: Option<Node>,
}

#[derive(Debug, Clone)]
struct Attr {
    name: String,
    value: toml::Value,
}

fn parse_attr<'a>(attr: (&'a str, &'a str)) -> Option<Attr> {
    let (key, value) = attr;
    let value = match toml::Value::deserialize(toml::de::ValueDeserializer::new(value)) {
        Ok(v) => v,
        Err(_) => toml::Value::String(value.to_string()),
    };
    Some(Attr {
        name: key.to_string(),
        value,
    })
}

impl Network {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn nodes_count(&self) -> usize {
        self.nodes.len()
    }
    pub fn node(&self, index: usize) -> Option<&Node> {
        self.nodes.get(index)
    }
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }
    pub fn nodes_rev(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter().rev()
    }
    pub fn output(&self) -> Option<&Node> {
        self.output.as_ref()
    }
    pub fn contains(&self, node: &Node) -> bool {
        self.nodes_map.contains_key(node.borrow().name())
    }
    pub fn contains_name(&self, node: &str) -> bool {
        self.nodes_map.contains_key(node)
    }

    pub fn insert_node(&mut self, node: Node) {
        self.nodes_map
            .insert(node.borrow().name().to_string(), node.clone());
        self.nodes.push(node);
    }

    pub fn insert_node_by_name(&mut self, name: String) {
        let node = new_node(self.nodes_count(), name.clone());
        self.nodes_map.insert(name, node.clone());
        self.nodes.push(node);
    }

    fn remove_node_single(&mut self, node: &Node) {
        // extracted here so that the n.lock() doesn't hang
        // infinitely later.
        let nodeoutind = {
            let out = node.borrow().output();
            let name = node.borrow().name().to_string();

            out.map(|o| {
                o.borrow()
                    .inputs()
                    .position(|i| i.borrow().name() == &name)
                    .unwrap()
            })
        };
        let n = node.borrow();
        self.nodes.remove(n.index());
        self.nodes_map.remove(n.name());
        if let Some(out) = n.output() {
            for inp in n.inputs() {
                inp.borrow_mut().set_output(&out);
            }
            let mut out = out.borrow_mut();
            let inputs = out
                .inputs()
                .enumerate()
                .filter(|(i, _)| *i != nodeoutind.unwrap())
                .map(|(_, n)| n.clone())
                .collect();
            out.overwrite_input(inputs);
            for inp in n.inputs() {
                out.add_input(inp);
            }
        } else {
            for inp in n.inputs() {
                inp.borrow_mut().unset_output();
            }
            if n.inputs().count() > 1 {
                eprintln!("WARN: Node with multiple inputs and no output Removed");
            }
        }
        self.reindex();
    }

    pub fn remove_node(&mut self, node: &Node) {
        self.remove_node_single(node);
        self.reorder();
        self.set_levels();
    }

    pub fn subnetwork(&self, attr: &str) -> anyhow::Result<Self> {
        let mut net = self.clone();
        net.output = None;
        let mut remove_nodes = Vec::new();
        for n in net.nodes() {
            let n2 = n.borrow();
            let v = n2
                .attr(attr)
                .context(format!("Attribute Doesn't exist in node {:?}", n2.name()))?
                .as_bool()
                .context(format!("Attribute not bool in node {:?}", n2.name()))?;
            if !v {
                remove_nodes.push(n.clone());
            }
        }
        for n in remove_nodes {
            net.remove_node_single(&n);
        }
        let out_count: usize = net
            .nodes()
            .map(|n| n.borrow().output().is_none() as usize)
            .sum();
        if out_count > 1 {
            // IF there are multiple outputs collect all under a root node
            let root = new_node(0, "ROOT".to_string());
            for n in net.nodes() {
                let mut n2 = n.borrow_mut();
                if n2.output().is_none() {
                    n2.set_output(&root);
                    root.borrow_mut().add_input(n);
                }
            }
            net.insert_node(root);
        }
        net.reorder();
        net.set_levels();
        Ok(net)
    }

    pub fn node_by_name(&self, name: &str) -> Option<&Node> {
        self.nodes_map.get(name)
    }

    pub fn get_or_insert_node_by_name(&mut self, name: &str) -> &Node {
        if self.contains_name(name) {
            self.node_by_name(name).unwrap()
        } else {
            self.insert_node_by_name(name.to_string());
            self.node_by_name(name).unwrap()
        }
    }

    pub fn from_file<P: AsRef<Path>>(filename: P) -> anyhow::Result<Self> {
        let mut network = Self::new();
        let file = File::open(filename.as_ref())?;
        let reader = BufReader::new(file);
        for line in reader.lines() {
            let line = line.unwrap().trim().to_string();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            if let Some((inp, out)) = line.split_once("->") {
                let inp = inp.trim();
                let out = out.trim();
                if !network.contains_name(inp) {
                    network.insert_node_by_name(inp.to_string());
                }
                if !network.contains_name(out) {
                    network.insert_node_by_name(out.to_string());
                }
                let inp = network.node_by_name(inp.trim()).unwrap();
                let out = network.node_by_name(out.trim()).unwrap();
                {
                    inp.borrow_mut().set_output(out);
                    out.borrow_mut().add_input(inp);
                }
            } else {
                _ = network.get_or_insert_node_by_name(&line);
            }
        }
        network.reorder();
        network.set_levels();
        Ok(network)
    }

    pub fn load_attrs<P: AsRef<Path>>(&mut self, attr_dir: P) -> anyhow::Result<()> {
        self.nodes_map.iter().try_for_each(|(name, node)| {
            // ignore the error on attribute read
            let attr_file = attr_dir.as_ref().join(format!("{}.toml", name));
            if attr_file.exists() && attr_file.is_file() {
                node.borrow_mut().load_attr(&attr_file)
            } else {
                Ok(())
            }
        })?;
        Ok(())
    }

    pub fn from_dot<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let file_content = std::fs::read_to_string(path)?;
        let raw = dot_parser::ast::Graph::read_dot(&file_content)?;
        let mapped = raw.filter_map(&parse_attr);
        let graph = dot_parser::canonical::Graph::from(mapped);

        let nodes: Vec<Node> = graph
            .nodes
            .set
            .into_iter()
            .enumerate()
            .map(|(i, (n, nd))| {
                let node = new_node(i, n.to_string());
                for attr in nd.attr.elems {
                    node.borrow_mut().set_attr(attr.name, attr.value);
                }
                node
            })
            .collect();
        let nodes_map: HashMap<String, Node> = nodes
            .iter()
            .map(|n| (n.borrow().name().to_string(), n.clone()))
            .collect();

        for edge in graph.edges.set {
            let from = nodes_map.get(edge.from);
            let to = nodes_map.get(edge.to);
            if let (Some(f), Some(t)) = (from, to) {
                f.borrow_mut().set_output(t);
                t.borrow_mut().add_input(f);
            }
        }

        let mut net = Self {
            nodes,
            nodes_map,
            output: None,
        };
        net.reorder();
        net.set_levels();
        Ok(net)
    }

    pub fn reindex(&self) {
        for (i, n) in self.nodes().enumerate() {
            n.borrow_mut().set_index(i);
        }
    }

    pub fn reorder(&mut self) {
        self.calc_order();
        self.output = self.node(0).cloned().map(|n| {
            let mut child = n.clone();
            loop {
                let cc = child.borrow().output();
                match cc {
                    Some(c) => child = c,
                    None => break,
                }
            }
            child
        });
        let mut new_nodes: Vec<Node> = Vec::with_capacity(self.nodes.len());
        fn insert_node(nv: &mut Vec<Node>, n: Node) {
            nv.push(n.clone());
            let mut inps: Vec<Node> = n.borrow().inputs().cloned().collect();
            inps.sort_by(compare_node_order);
            for c in inps {
                insert_node(nv, c);
            }
        }
        if let Some(out) = &self.output {
            insert_node(&mut new_nodes, out.clone());
        }
        if new_nodes.len() < self.nodes.len() {
            // todo, make the nodes into different groups?
            eprintln!(
                "Reorder not done, the nodes are not connected: {} connected out of {}",
                new_nodes.len(),
                self.nodes.len()
            );
            return;
        }
        self.nodes = new_nodes;
        self.reindex();
    }

    /// Correct the network by providing a list of nodes with their outlet
    pub fn readjust(&mut self, corrections: &[(&str, &str)]) {
        for (node, outlet) in corrections {
            let node = self.get_or_insert_node_by_name(node).clone();
            let outlet = self.get_or_insert_node_by_name(outlet);
            node.borrow_mut().set_output(outlet);
        }
    }

    /// sets the levels for the nodes, 0 means it's the main branch and
    /// increasing number is for tributories level
    pub fn set_levels(&mut self) {
        fn recc_set(node: &Node, level: usize) {
            node.borrow_mut().set_level(level);
            node.borrow_mut().order_inputs();
            let node = node.borrow();
            let mut inps = node.inputs();
            if let Some(i) = inps.next() {
                recc_set(i, level);
            }
            for i in inps {
                recc_set(i, level + 1);
            }
        }
        if let Some(output) = &self.output {
            recc_set(output, 0);
        }
    }

    pub fn calc_order(&mut self) {
        let mut all_nodes: Vec<Node> = self.nodes.clone();
        let mut order_queue: Vec<Node> = Vec::with_capacity(self.nodes.len());
        loop {
            // very easy to get an infinite loop here, be careful about the logic
            if order_queue.is_empty() {
                if let Some(elem) = all_nodes.pop() {
                    if elem.borrow().order() > 0 {
                        continue;
                    }
                    order_queue.push(elem);
                } else {
                    break;
                }
            }

            let node = order_queue.pop().unwrap();
            if node.borrow().inputs().next().is_none() {
                node.borrow_mut().set_order(1);
            } else {
                let uncalc_inputs: Vec<Node> = node
                    .borrow()
                    .inputs()
                    .filter(|i| i.borrow().order() == 0)
                    .cloned()
                    .collect();
                if !uncalc_inputs.is_empty() {
                    order_queue.push(node);
                    uncalc_inputs.iter().for_each(|node| {
                        order_queue.push(node.clone());
                    });
                } else {
                    let ord: usize = node.borrow().inputs().map(|n| n.borrow().order()).sum();
                    node.borrow_mut().set_order(ord + 1);
                }
            }
        }
    }

    pub fn edges(&self) -> Vec<(String, String)> {
        let mut edges = Vec::new();
        for node in &self.nodes {
            let node = node.borrow();
            if let Some(out) = node.output() {
                edges.push((node.name().to_string(), out.borrow().name().to_string()));
            }
        }
        edges
    }

    pub fn print(&self) {
        for node in &self.nodes {
            let node = node.borrow();
            if let Some(out) = node.output() {
                println!("{} -> {}", node.name(), out.borrow().name());
            } else {
                println!("{}", node.name());
            }
        }
    }
}

fn compare_node_order(n1: &Node, n2: &Node) -> std::cmp::Ordering {
    n1.borrow()
        .order()
        .partial_cmp(&n2.borrow().order())
        .unwrap()
}
