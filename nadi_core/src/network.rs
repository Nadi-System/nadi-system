use crate::attrs::{AttrMap, HasAttributes};
use crate::eval::EvalErrorType;
use crate::expressions::RawExpr;
use crate::node::{new_node, Node};
use crate::timeseries::{HasSeries, HasTimeSeries, SeriesMap, TsMap};
use abi_stable::std_types::{RDuration, Tuple2};
use abi_stable::{
    std_types::{
        RHashMap,
        ROption::{RNone, RSome},
        RString, RVec,
    },
    StableAbi,
};
use colored::Colorize;
use std::collections::{HashMap, HashSet};
use std::fmt::Debug;

#[derive(Default, Clone, PartialEq)]
enum NetworkType {
    DAG,
    InTree,
    OutTree,
    Undirected,
    #[default]
    Directed,
}

impl std::fmt::Display for NetworkType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::DAG => write!(fmt, "DAG"),
            Self::InTree => write!(fmt, "InTree"),
            Self::OutTree => write!(fmt, "OutTree"),
            Self::Undirected => write!(fmt, "Undirected"),
            Self::Directed => write!(fmt, "Directed"),
        }
    }
}

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
    /// Outlet [`Node`s] of the network
    pub(crate) outlets: RVec<Node>,
    /// network is ordered based on input topology
    pub(crate) ordered: bool,
}

impl std::fmt::Debug for Network {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Network")
            .field("nodes", &self.nodes)
            .field("attributes", &self.attributes)
            .field("outlets", &self.outlets.len())
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

    /// Leaf of the network (single node with no inputs)
    pub fn leaf(&self) -> Option<&Node> {
        let mut leaves = self.leaves();
        let lf = leaves.next()?;
        if leaves.next().is_some() {
            None
        } else {
            Some(lf)
        }
    }

    /// Iterator for the leaf nodes in the network
    pub fn leaves(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .iter()
            .filter(|n| {
                self.nodes_map[n.as_str()]
                    .try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .is_leaf()
            })
            .map(|n| &self.nodes_map[n])
    }

    // Return Result<&Node, bool>, the bool is true if there are multiple roots, false if there are none, then make an EvalErrorType that takes VarType and this bool to generate better error message.
    /// Root of the network (single node with no output)
    pub fn root(&self) -> Option<&Node> {
        let mut roots = self.roots();
        let rt = roots.next()?;
        if roots.next().is_some() {
            None
        } else {
            Some(rt)
        }
    }

    /// Iterator for the root nodes in the network
    pub fn roots(&self) -> impl Iterator<Item = &Node> {
        self.nodes
            .iter()
            .filter(|n| {
                self.nodes_map[n.as_str()]
                    .try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .is_root()
            })
            .map(|n| &self.nodes_map[n])
    }

    /// Append the edges from the list, making new nodes if necessary
    pub fn append_edges(&mut self, edges: &[(&str, &str)], force: bool) -> Result<(), String> {
        for (start, end) in edges {
            if start == end {
                // Maybe fix it after adding assertions
                // return Err(format!("Node {:?} has itself as the output", start));
                if !self.nodes_map.contains_key(*start) {
                    self.insert_node_by_name(start);
                }
                let inp = self.node_by_name(start).expect("Input just inserted");
                let out = inp.clone();
                inp.try_lock()
                    .into_option()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_output(out.clone());
                inp.try_lock()
                    .into_option()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_input(out.clone());
            } else {
                if !self.nodes_map.contains_key(*start) {
                    self.insert_node_by_name(start);
                }
                if !self.nodes_map.contains_key(*end) {
                    self.insert_node_by_name(end);
                }
                let inp = self.node_by_name(start).expect("Input just inserted");
                let out = self.node_by_name(end).expect("Output just inserted");
                inp.try_lock()
                    .into_option()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_output(out.clone());
                out.try_lock()
                    .into_option()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_input(inp.clone());
            }
        }
        self.flatten();
        Ok(())
    }

    pub fn position_nodes(&mut self) {
        todo!()
    }

    pub fn flatten(&mut self) {
        self.reorder();
        self.set_levels();
        self.nodes_map.values().for_each(|n| {
            let mut n = n
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()));
            let pos = (n.level() as f64, n.index() as f64);
            n.set_pos(pos)
        })
    }

    /// Create a network with given edges
    pub fn from_edges(edges: &[(&str, &str)], force: bool) -> Result<Self, String> {
        let mut network = Self::default();
        network.append_edges(edges, force)?;
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
            let n = n
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()));
            match n.output() {
                RSome(o) => Some((
                    n.index(),
                    o.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .index(),
                )),
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
    pub fn try_node_by_name(&self, name: &str) -> Result<&Node, EvalErrorType> {
        self.nodes_map
            .get(name)
            .ok_or_else(|| EvalErrorType::NodeNotFound(name.to_string()))
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
    ) -> Result<Vec<Node>, EvalErrorType> {
        match (prop, order) {
            (PropNodes::All, o) => Ok(self.nodes_order(o)),
            // preserve the list order if order not given explicitly
            (PropNodes::List(lst), PropOrder::Auto) => {
                let nodes: Vec<_> = lst.iter().map(|n| self.node_by_name(n.as_str())).collect();
                if nodes.iter().any(Option::is_none) {
                    let not_found: Vec<&str> = lst
                        .iter()
                        .zip(nodes)
                        .filter(|(_, o)| o.is_none())
                        .map(|(n, _)| n.as_str())
                        .collect();
                    Err(EvalErrorType::NodeNotFound(not_found.join(", ")))
                } else {
                    Ok(nodes.into_iter().filter_map(|o| o.cloned()).collect())
                }
            }
            (PropNodes::List(lst), o) => {
                let mut sel_lst: HashSet<&str> = lst.iter().map(|n| n.as_str()).collect();
                let res = self
                    .nodes_order(o)
                    .into_iter()
                    .filter(|n| {
                        sel_lst.remove(
                            n.try_lock()
                                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                                .name(),
                        )
                    })
                    .collect();
                if sel_lst.is_empty() {
                    Ok(res)
                } else {
                    Err(EvalErrorType::NodeNotFound(
                        sel_lst.into_iter().collect::<Vec<&str>>().join(", "),
                    ))
                }
            }
            (PropNodes::Path(p), o) => self.nodes_path(o, p),
        }
    }

    /// Get a list of nodes in the given path
    pub fn nodes_path(
        &self,
        order: &PropOrder,
        path: &StrPath,
    ) -> Result<Vec<Node>, EvalErrorType> {
        let start = self.try_node_by_name(path.start.as_str())?;
        let end = self.try_node_by_name(path.end.as_str())?;
        // we'll assume the network is indexed based on order, small
        // indices are closer to outlet; and resuffle the nodes
        let (start, end, flipped) = if start
            .try_lock()
            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
            .index()
            > end.lock().index()
        {
            (start, end, false)
        } else {
            (end, start, true)
        };
        let mut curr = start.clone();
        let mut path_nodes = vec![];
        let start_name = self.nodes[start
            .try_lock()
            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
            .index()]
        .as_str();
        let end_name = self.nodes[end
            .try_lock()
            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
            .index()]
        .as_str();
        loop {
            path_nodes.push(curr.clone());
            if curr
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .name()
                == end_name
            {
                break;
            }
            let tmp = if let RSome(o) = curr
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .output()
            {
                o.clone()
            } else {
                // TODO: work with multiple output
                return Err(EvalErrorType::PathNotFound(
                    start_name.to_string(),
                    curr.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .name()
                        .to_string(),
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

    pub fn leaf_nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes_map.iter().map(|n| n.1).filter(|n| {
            n.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .is_empty()
        })
    }

    /// Calculate the weight of all nodes
    ///
    /// Value of weight signifies the number of all nodes (recursively)
    /// that are on the input side of the node
    pub fn calc_weights(&mut self) {
        let mut weights = HashMap::<RString, u64>::with_capacity(self.nodes.len());

        // ran into stackoverflow with the recursive pattern of calculation
        self.nodes_map.iter().for_each(|n| {
            if n.1
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .is_empty()
            {
                weights.insert(n.0.clone(), 1);
            }
            let mut visited = HashSet::new();
            visited.insert(
                n.1.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .index(),
            );
            let mut n =
                n.1.try_lock_for(RDuration::from_secs(1))
                    .expect("Lock failed for node, maybe branched network")
                    .output()
                    .cloned();
            while let RSome(out) = n {
                let o = out
                    .try_lock_for(RDuration::from_secs(1))
                    .expect("Lock failed for node, maybe branched network");
                if visited.contains(&o.index()) {
                    // looped back to the already visited node
                    // this needs to be here to prevent infinite loop
                    break;
                }
                visited.insert(o.index());
                *weights.entry(o.name().into()).or_insert(1) += 1;
                n = o.output().cloned();
            }
        });

        for (node, ord) in weights {
            self.nodes_map[&node]
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .set_weight(ord);
        }
    }

    /// Calculate the order of all nodes; needs to be called after reorder
    ///
    /// Value of order signifies the number of all nodes (recursively)
    /// that are on the input side of the node
    pub fn calc_order(&mut self) {
        self.nodes_map.values().for_each(|n| {
            n.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .set_order(0)
        });
        self.nodes.iter().rev().for_each(|n| {
            let nobj = &self.nodes_map[n];
            let inputs: Vec<Node> = nobj
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .iter()
                .cloned()
                .collect();
            let ord = inputs
                .iter()
                // nodes that have some node loop back to them will have zero  (unset) value as order from them
                .map(|i| {
                    i.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .order()
                })
                .max()
                .unwrap_or(0);
            nobj.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .set_order(ord + 1);
        });
    }

    /// Reorder the nodes in the network
    pub fn reorder(&mut self) {
        self.calc_weights();
        let weights: HashMap<_, _> = self
            .nodes_map
            .iter()
            .map(|n| {
                (
                    n.0.as_str(),
                    n.1.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .weight(),
                )
            })
            .collect();
        self.outlets = {
            let mut outlets: Vec<&str> = self
                .nodes_map
                .iter()
                .filter(|Tuple2(_, n)| {
                    n.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .is_root()
                })
                .map(|Tuple2(n, _)| n.as_str())
                .collect();
            outlets.sort_by(|a, b| weights[b].cmp(&weights[a]));
            outlets
                .into_iter()
                .map(|o| self.nodes_map[o].clone())
                .collect()
        };

        let mut visited: HashSet<String> = HashSet::new();
        let mut nodes_queue: Vec<String> = Vec::with_capacity(self.nodes.len());
        let mut new_nodes: Vec<String> = Vec::with_capacity(self.nodes.len());
        for o in self.outlets.iter().rev() {
            nodes_queue.push(
                o.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .name()
                    .to_string(),
            );
        }

        while let Some(curr) = nodes_queue.pop() {
            if visited.contains(&curr) {
                // basically to stop infinite loop, need to look
                // further how it impacts the values
                continue;
            }
            visited.insert(curr.clone());
            let inputs: Vec<Node> = self.nodes_map[curr.as_str()]
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .iter()
                .cloned()
                .collect();
            let mut inps: Vec<String> = inputs
                .iter()
                .map(|i| {
                    i.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .name()
                        .to_string()
                })
                .collect();

            inps.sort_by(|n1, n2| {
                weights[n2.as_str()]
                    .partial_cmp(&weights[n1.as_str()])
                    .unwrap()
            });
            // this just reorders the inputs, we don't change the input nodes
            self.nodes_map[curr.as_str()]
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs = inps
                .iter()
                .map(|i| self.nodes_map[i.as_str()].clone())
                .collect();
            for c in inps {
                nodes_queue.push(c.clone());
            }
            new_nodes.push(curr);
        }
        let new_nodes: Vec<Node> = new_nodes
            .iter()
            .map(|n| self.nodes_map[n.as_str()].clone())
            .collect();
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
            .map(|n| {
                n.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .name()
                    .into()
            })
            .collect::<Vec<RString>>()
            .into();
        self.reindex();
        self.calc_order();
        self.ordered = true;
        // eprintln!("Exit Re Order");
    }

    /// reindex the nodes in the network
    pub fn reindex(&self) {
        for (i, n) in self.nodes().enumerate() {
            n.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .set_index(i);
        }
    }

    /// sets the levels for the nodes, 0 means it's the main branch and
    /// increasing number is for tributories level
    pub fn set_levels(&mut self) {
        fn recc_set(node: &Node, level: u64, visited: &mut HashSet<usize>) {
            let ind = node
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .index();
            if visited.contains(&ind) {
                // this will protect from infinite loop; not sure if
                // it is correct to skip it, or change the level to
                // something reasonable
                return;
            }
            visited.insert(ind);
            node.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .set_level(level);
            let inputs = node
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .to_vec();
            match &inputs[..] {
                [] => (),
                [first, rest @ ..] => {
                    recc_set(first, level, visited);
                    for i in rest {
                        recc_set(i, level + 1, visited);
                    }
                }
            }
        }

        let mut visited: HashSet<usize> = HashSet::new();
        for o in &self.outlets {
            recc_set(o, 0, &mut visited);
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
        // remove node from the network
        let (ind, outputs): (usize, Vec<Node>) = {
            let n = node
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()));
            let ind = n.index();
            self.nodes.remove(ind);
            self.nodes_map.remove(n.name());
            // make sure the block below doesn't hang for long
            (
                ind,
                n.outputs().iter().map(|o| o.clone()).collect::<Vec<Node>>(),
            )
        };

        // remove node from its output
        for out in &outputs {
            let pos = out
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
                .iter()
                .position(|i| {
                    i.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .index()
                        == ind
                })
                .expect("Node should be in input list of output");
            out.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs_mut()
                .remove(pos);
        }
        let inputs: Vec<Node> = node
            .try_lock()
            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
            .inputs()
            .iter()
            .cloned()
            .collect();

        // remove node from its inputs
        for inp in &inputs {
            let pos = inp
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .outputs()
                .iter()
                .position(|i| {
                    i.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .index()
                        == ind
                })
                .expect("Node should be in input list of output");
            inp.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .outputs_mut()
                .remove(pos);
        }

        // add new connections
        for inp in inputs {
            for out in &outputs {
                inp.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_output(out.clone());
                out.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_input(inp.clone());
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
        // self.reorder();
        // self.set_levels();
    }

    // TODO: rewrite to make sure subset works properly
    /// Subset the network into a new network by removing a bunch of nodes
    pub fn subset(&mut self, filter: &[bool], keep: bool) -> Result<(), String> {
        let include_nodes: HashMap<String, Node> = self
            .nodes()
            .zip(self.node_names())
            .zip(filter)
            .filter(|(_, &f)| !(f ^ keep))
            .map(|((n, name), _)| (name.to_string(), n.clone()))
            .collect();
        include_nodes.values().for_each(|n| {
            n.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .unset_inputs();
        });
        for node in include_nodes.values() {
            let mut start = node.clone();
            loop {
                let out = start
                    .try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .output()
                    .cloned();
                match out {
                    RNone => {
                        node.try_lock()
                            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                            .unset_outputs();
                        break;
                    }
                    RSome(o) => {
                        let mut op =
                            o.try_lock()
                                .expect(&format!("mutex error: {:?} {}", file!(), line!()));
                        if include_nodes.contains_key(op.name()) {
                            op.add_input(node.clone());
                            node.try_lock()
                                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                                .add_output(o.clone());
                            break;
                        } else {
                            start = o.clone();
                        }
                    }
                }
            }
        }
        self.nodes = include_nodes
            .iter()
            .map(|(n, _)| n.to_string().into())
            .collect();
        self.nodes_map = include_nodes
            .into_iter()
            .map(|(n, o)| (n.into(), o))
            .collect();
        self.reorder();
        self.set_levels();
        Ok(())
    }

    pub fn new_root(&mut self, node: Node) {
        node.try_lock()
            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
            .unset_outputs();
        let mut nodes = Vec::with_capacity(self.nodes.len());
        let mut nodes_map = HashMap::with_capacity(self.nodes.len());
        fn register(n: &Node, nds: &mut Vec<RString>, nmp: &mut HashMap<RString, Node>) {
            let nm: RString = n
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .name()
                .to_string()
                .into();
            nds.push(nm.clone());
            nmp.insert(nm, n.clone());
            for i in n
                .try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .inputs()
            {
                register(i, nds, nmp)
            }
        }
        register(&node, &mut nodes, &mut nodes_map);
        self.nodes = nodes.into();
        self.nodes_map = nodes_map.into();
        self.outlets = vec![node].into();
        // self.reorder();
        // self.set_levels();
    }

    /// get the connections in utf8 string to print in terminal
    pub fn connections_utf8(&self) -> Vec<String> {
        self.nodes()
            .map(|node| {
                let node =
                    node.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()));
                let level = node.level();
                let par_level = node
                    .output()
                    .map(|n| {
                        n.try_lock()
                            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                            .level()
                    })
                    .unwrap_or(level);
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
                let node =
                    node.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()));
                let level = node.level();
                let par_level = node
                    .output()
                    .map(|n| {
                        n.try_lock()
                            .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                            .level()
                    })
                    .unwrap_or(level);
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
#[derive(StableAbi, Debug, Hash, Default, Clone, Eq, PartialEq)]
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
    /// start position of the propagation
    pub start: (usize, usize),
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
    Expr(RawExpr),
    // TODO
    // Head(usize),
    // Tail(usize),
}

impl std::fmt::Display for PropCondition {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Self::All => Ok(()),
            Self::Expr(expr) => write!(fmt, "({})", expr),
        }
    }
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
                let name = RString::from(
                    n.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .name(),
                );
                (name, n)
            })
            .collect::<HashMap<RString, Node>>()
            .into();
        net.nodes = net.nodes_map.keys().cloned().collect::<Vec<_>>().into();
        net.outlets = vec![node].into();
        // net.reorder();
        // net.set_levels();
        net
    }
}
