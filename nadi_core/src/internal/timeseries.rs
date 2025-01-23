use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod timeseries {

    use crate::prelude::*;
    use abi_stable::std_types::RString;
    use nadi_plugin::{network_func, node_func};
    use std::collections::HashSet;

    /// Number of timeseries in the node
    #[node_func]
    fn ts_count(node: &mut NodeInner) -> usize {
        node.ts_map().len()
    }

    /// List all timeseries in the node
    #[node_func]
    fn ts_list(node: &mut NodeInner) -> Vec<String> {
        node.ts_map().keys().map(|s| s.to_string()).collect()
    }

    /// Type name of the timeseries
    #[node_func(safe = false)]
    fn ts_dtype(
        node: &mut NodeInner,
        /// Name of the timeseries
        name: &str,
        /// Do not error if timeseries does't exist
        safe: bool,
    ) -> Result<Option<String>, String> {
        match node.try_ts(name) {
            Ok(s) => Ok(Some(s.values_type().to_string())),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Length of the timeseries
    #[node_func(safe = false)]
    fn ts_len(
        node: &mut NodeInner,
        /// Name of the timeseries
        name: &str,
        /// Do not error if timeseries does't exist
        safe: bool,
    ) -> Result<Option<usize>, String> {
        match node.try_ts(name) {
            Ok(s) => Ok(Some(s.series().len())),
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /** Print the given timeseries values in csv format
    # TODO
    - save to file instead of showing with `outfile: Option<PathBuf>`
    */
    #[node_func(header = true)]
    fn ts_print(
        node: &mut NodeInner,
        /// name of the timeseries
        name: &String,
        /// show header
        header: bool,
        /// number of head rows to show (all by default)
        head: Option<i64>,
    ) -> Result<(), RString> {
        if let Some(ts) = node.ts(name) {
            let values = ts.values_as_attributes();
            if header {
                println!("time,{name}");
            }
            let head = head.map(|h| h as usize).unwrap_or_else(|| values.len());
            for (t, v) in ts
                .timeline()
                .lock()
                .str_values()
                .zip(values.iter())
                .take(head)
            {
                println!("{},{}", t, v.to_string());
            }
            println!();
        } else {
            return Err(format!(
                "Timeseries `{}` is not available in node `{}`",
                name,
                node.name()
            )
            .into());
        }
        Ok(())
    }

    /// Save timeseries from all nodes into a single csv file
    ///
    /// TODO: error/not on unqual length
    /// TODO: error/not on no timeseries, etc...
    /// TODO: output to `file: PathBuf`
    #[network_func]
    fn ts_print_csv(
        net: &mut Network,
        /// Name of the timeseries to save
        name: String,
        /// number of head rows to show (all by default)
        head: Option<usize>,
        /// Include only these nodes (all by default)
        nodes: Option<HashSet<String>>,
    ) -> anyhow::Result<()> {
        let mut ts_nodes = vec![];
        let mut values = vec![];
        let mut timeline = None;
        for node in net.nodes() {
            let node = node.lock();
            if let Some(ref node_list) = nodes {
                if !node_list.contains(node.name()) {
                    continue;
                }
            }
            // ignoring the nodes without the given timeseries
            if let Some(ts) = node.ts(&name) {
                if let Some(tl) = &timeline {
                    if !ts.is_timeline(tl) {
                        return Err(anyhow::Error::msg("Different Timelines"));
                    }
                } else {
                    timeline = Some(ts.timeline().clone());
                }
                ts_nodes.push(node.name().to_string());
                values.push(ts.values_as_attributes());
            }
        }
        // export to CSV
        if let Some(tl) = timeline {
            let tl = tl.lock();
            let head = head.unwrap_or(tl.str_values().count());
            println!("datetime,{}", ts_nodes.join(","));
            for (i, t) in tl.str_values().enumerate() {
                if i >= head {
                    break;
                }
                let row: Vec<String> = values.iter().map(|v| v[i].to_string()).collect();
                println!("{t},{}", row.join(","));
            }
        }
        Ok(())
    }
}
