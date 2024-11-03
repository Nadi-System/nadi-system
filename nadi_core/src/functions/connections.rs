use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod connections {
    use crate::parser::network::is_valid_node_name;
    use crate::prelude::*;
    use nadi_plugin::network_func;
    use std::path::PathBuf;

    use std::fs::File;
    use std::io::{BufWriter, Write};

    /// Load the given file into the network
    #[network_func]
    fn load_file(net: &mut Network, file: PathBuf) -> anyhow::Result<()> {
        *net = Network::from_file(file)?;
        Ok(())
    }

    /// Save the network into the given file
    ///
    /// # Arguments
    /// - `file`: Path to the output file
    /// - `quote_all` [default: true]: quote all node names.
    ///   if false, doesn't quote valid identifier names
    /// - `graphviz` [default: false]: wrap the network into
    ///   a valid graphviz file. For more control on file
    ///   `save_graphviz` from `graphviz` plugin instead.
    #[network_func(quote_all = true, graphviz = false)]
    fn save_file(
        net: &mut Network,
        file: PathBuf,
        quote_all: bool,
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
                if is_valid_node_name(start) {
                    write!(writer, "{}", start)?;
                } else {
                    write!(writer, "{:?}", start)?;
                }
                write!(writer, " -> ")?;
                if is_valid_node_name(end) {
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
