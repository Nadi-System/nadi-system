use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod timeseries {

    use crate::prelude::*;
    use abi_stable::std_types::RString;
    use nadi_plugin::{network_func, node_func};
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::path::PathBuf;

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

    /// Write the given nodes to csv with given attributes and series
    #[network_func]
    fn series_csv(
        net: &mut Network,
        #[prop] prop: &Propagation,
        /// Path to the output csv
        outfile: PathBuf,
        /// list of attributes to write
        attrs: Vec<String>,
        /// list of series to write
        series: Vec<String>,
    ) -> anyhow::Result<()> {
        let mut f = File::create(&outfile)?;
        let mut w = BufWriter::new(f);
        let middle = !attrs.is_empty() && !series.is_empty();
        // headers for the csv
        writeln!(
            w,
            "{}{}{}",
            attrs.join(","),
            if middle { "," } else { "" },
            series.join(",")
        )?;
        for node in net.nodes_propagation(prop).map_err(anyhow::Error::msg)? {
            let node = node.lock();
            let attrs: Vec<String> = attrs
                .iter()
                .map(|a| node.attr(a).map(|a| a.to_string()).unwrap_or_default())
                .collect();
            let series: Vec<Vec<String>> = series
                .iter()
                .map(|a| {
                    node.series(a)
                        .map(|s| {
                            s.clone()
                                .to_attributes()
                                .into_iter()
                                .map(|a| a.to_string())
                                .collect()
                        })
                        .unwrap_or_default()
                })
                .collect();
            let lengths: Vec<usize> = series.iter().map(|s| s.len()).collect();
            if lengths.is_empty() {
                writeln!(w, "{}", attrs.join(","))?;
                continue;
            } else if lengths.iter().any(|l| *l != lengths[0]) {
                return Err(anyhow::Error::msg(format!(
                    "Node {}: Series lengths don't match: {lengths:?}",
                    node.name()
                )));
            }
            for i in 0..lengths[0] {
                let values: Vec<&str> = series.iter().map(|s| s[i].as_str()).collect();
                writeln!(
                    w,
                    "{}{}{}",
                    attrs.join(","),
                    if middle { "," } else { "" },
                    values.join(",")
                )?;
            }
        }
        Ok(())
    }
}
