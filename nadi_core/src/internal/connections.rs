use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod connections {
    use crate::parser::tokenizer::valid_variable_name;
    use crate::prelude::*;
    use nadi_plugin::network_func;
    use std::path::PathBuf;

    use std::fs::File;
    use std::io::{BufWriter, Write};

    /// Load the given file into the network
    ///
    /// This replaces the current network with the one loaded from the
    /// file.
    #[network_func(append = false)]
    fn load_file(
        net: &mut Network,
        /// File to load the network connections from
        file: PathBuf,
        /// Append the connections in the current network
        append: bool,
    ) -> anyhow::Result<()> {
        if append {
            todo!()
        } else {
            *net = Network::from_file(file)?;
        }
        Ok(())
    }

    /// Take a subset of network by only including the selected nodes
    #[network_func(keep = true)]
    fn subset(
        net: &mut Network,
        #[prop] prop: &Propagation,
        /// Keep the selected nodes (false = removes the selected)
        keep: bool,
    ) -> Result<(), String> {
        net.subset(prop, keep)
    }

    /// Save the network into the given file
    ///
    /// For more control on graphviz file writing use
    /// `save_graphviz` from `graphviz` plugin instead.
    #[network_func(quote_all = true, graphviz = false)]
    fn save_file(
        net: &mut Network,
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
}
