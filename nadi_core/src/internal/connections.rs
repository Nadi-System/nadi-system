use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod conn {
    use crate::parser::tokenizer::valid_variable_name;
    use crate::prelude::*;
    use anyhow::Context;
    use nadi_plugin::{network_func, node_func};
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;
    use std::str::FromStr;

    /// Load the given file into the network
    ///
    /// This replaces the current network with the one loaded from the
    /// file.
    #[network_func(append = false, force = false)]
    fn load_file(
        net: &mut Network,
        /// File to load the network connections from
        file: PathBuf,
        /// Append the connections in the current network
        append: bool,
        /// Force overriding outputs if previous one is present, only valid for override
        force: bool,
    ) -> anyhow::Result<()> {
        if append {
            let contents =
                std::fs::read_to_string(&file).context("Error while accessing the network file")?;
            let tokens = crate::parser::tokenizer::get_tokens(&contents);
            let paths = crate::parser::network::parse(tokens)?;
            let edges: Vec<(&str, &str)> = paths
                .iter()
                .map(|p| (p.start.as_str(), p.end.as_str()))
                .collect();
            net.append_edges(&edges, force)
                .map_err(anyhow::Error::msg)?;
        } else if force {
            return Err(anyhow::Error::msg(
                "Parameter force not valid when append is false",
            ));
        } else {
            *net = Network::from_file(file)?;
        }
        Ok(())
    }

    /// Load network from the given string
    ///
    /// This replaces the current network with the one loaded from the
    /// string.
    ///
    /// ```task
    /// network load_str("a -> b");
    /// env assert_eq(nodes.NAME, ["b", "a"])
    /// ```
    #[network_func(append = false, force = false)]
    fn load_str(
        net: &mut Network,
        /// String containing Network connections
        contents: &str,
        /// Append the connections in the current network
        append: bool,
        /// Force overriding outputs if previous one is present, only valid for override
        force: bool,
    ) -> Result<(), String> {
        if append {
            let tokens = crate::parser::tokenizer::get_tokens(contents);
            let paths = crate::parser::network::parse(tokens).map_err(|e| e.to_string())?;
            let edges: Vec<(&str, &str)> = paths
                .iter()
                .map(|p| (p.start.as_str(), p.end.as_str()))
                .collect();
            net.append_edges(&edges, force)?;
        } else if force {
            return Err(String::from(
                "Parameter force not valid when append is false",
            ));
        } else {
            *net = Network::from_str(contents).map_err(|e| e.user_msg(None))?;
        }
        Ok(())
    }

    /// Load the given edges as a network
    ///
    /// This replaces the current network with the one loaded from the
    /// file.
    ///
    /// ```task
    /// network load_edges([["a", "b"], ["b", "c"]]);
    /// env assert_eq(nodes.NAME, ["c", "b", "a"])
    /// ```
    #[network_func(append = false, force = false)]
    fn load_edges(
        net: &mut Network,
        /// String containing Network connections
        edges: &[(String, String)],
        /// Append the connections in the current network
        append: bool,
        /// Force overriding outputs if previous one is present
        force: bool,
    ) -> Result<(), String> {
        let edges: Vec<(&str, &str)> = edges.iter().map(|p| (p.0.as_str(), p.1.as_str())).collect();
        if append {
            net.append_edges(&edges, force)?;
        } else {
            *net = Network::from_edges(&edges, force)?;
        }
        Ok(())
    }

    /// Take a subset of network by only including the selected nodes
    /// ```task
    /// network load_str("a -> b\n b->c");
    /// nodes[a->b].sth = true;
    /// node[c].sth = false;
    /// network subset(nodes.sth);
    /// env assert_eq(nodes.NAME, ["b", "a"])
    /// ```
    #[network_func(keep = true)]
    fn subset(
        net: &mut Network,
        filter: &[bool],
        /// Keep the selected nodes (false = removes the selected)
        keep: bool,
    ) -> Result<(), String> {
        net.subset(filter, keep)
    }

    /// Save the network into the given file
    ///
    /// For more control on graphviz file writing, use
    /// `save_graphviz` from `graphviz` plugin instead.
    #[network_func(quote_all = true, graphviz = false)]
    fn save_file(
        net: &Network,
        /// Path to the output file
        file: PathBuf,
        /// quote all node names; if false, doesn't quote valid identifier names
        quote_all: bool,
        /// wrap the network into a valid graphviz file
        graphviz: bool,
    ) -> anyhow::Result<()> {
        let file = File::create(file)?;
        let mut writer = BufWriter::new(file);
        if graphviz {
            writeln!(writer, "digraph network {{")?;
        }
        for (start, end) in net.edges_str() {
            if quote_all {
                writeln!(writer, "{:?} -> {:?}", start, end)?;
            } else {
                if valid_variable_name(start) {
                    write!(writer, "{}", start)?;
                } else {
                    write!(writer, "{:?}", start)?;
                }
                write!(writer, " -> ")?;
                if valid_variable_name(end) {
                    writeln!(writer, "{}", end)?;
                } else {
                    writeln!(writer, "{:?}", end)?;
                }
            }
        }
        if graphviz {
            writeln!(writer, "}}")?;
        }
        Ok(())
    }

    /// Take a subset of network by taking the given node as a new outlet
    ///
    /// ```task
    /// network load_str("a -> b\n b->c\n x -> y");
    /// network subset_from("b")
    /// env assert_eq(nodes.NAME, ["b", "a"])
    /// ```
    #[network_func]
    fn subset_from(net: &mut Network, new_outlet: &str) -> Result<(), String> {
        let node = net
            .node_by_name(new_outlet)
            .ok_or(format!("Node {new_outlet} not found in the network"))?
            .clone();
        net.new_outlet(node);
        Ok(())
    }

    /// Take a subset of network by only including the largest blob of connected nodes
    ///
    /// When you load a network that have disconnected nodes, this
    /// function allows you to filter out all the nodes except the one
    /// belonging to the largest connected network (number of
    /// nodes). Alternatively, you can also use ORDER and other logic
    /// in the task system to do that.
    ///
    /// If your network has a root node, and no parent node is given,
    /// then it'll just keep the network as it is.
    ///
    /// ```task
    /// network load_str("a -> b\n b->c\n x -> y");
    /// network subset_largest()
    /// env assert_eq(nodes.NAME, ["c", "b", "a"])
    /// ```
    #[network_func]
    fn subset_largest(net: &mut Network, parent: Option<String>) -> Result<(), String> {
        let node = match parent {
            Some(par) => {
                let par = net
                    .node_by_name(&par)
                    .ok_or(format!("Node {par} not found in the network"))?
                    .clone();

                let mut outlet: Option<Node> = None;
                for i in par.lock().inputs() {
                    let mut replace = outlet.is_none();
                    if let Some(ref o) = outlet {
                        if o.lock().order() < i.lock().order() {
                            replace = true;
                        }
                    }
                    if replace {
                        outlet = Some(i.clone());
                    }
                }
                outlet.unwrap_or(par)
            }
            None if net.outlets.len() > 1 => net.outlets().next().unwrap().clone(),
            // if one or 0 outlet nodes, then do nothing
            None => return Ok(()),
        };
        net.new_outlet(node);
        Ok(())
    }

    /// Move the node to the side so that its inputs go to the output
    #[node_func]
    fn move_aside(node: &mut NodeInner) {
        node.move_aside()
    }

    /// Move the node down so that it swaps the place with the output
    #[node_func]
    fn move_down(node: &mut NodeInner) {
        node.move_down()
    }
}
