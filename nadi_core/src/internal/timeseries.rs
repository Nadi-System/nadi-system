use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod ts {

    use crate::prelude::*;
    use abi_stable::std_types::{
        ROption::{RNone, RSome},
        RString,
    };
    use nadi_plugin::{network_func, node_func};
    use std::collections::HashSet;
    use std::fs::File;
    use std::io::{BufWriter, Write};
    use std::path::Path;

    /// Number of timeseries in the node
    #[node_func]
    fn ts_count(node: &NodeInner) -> usize {
        node.ts_map().len()
    }

    /// List all timeseries in the node
    #[node_func]
    fn ts_list(node: &NodeInner) -> Vec<String> {
        node.ts_map().keys().map(|s| s.to_string()).collect()
    }

    #[node_func]
    /// Delete the timeseries with the given name
    fn ts_delete(
        node: &mut NodeInner,
        /// Name of the timeseries to delete from this node
        name: &str,
    ) -> bool {
        node.del_ts(name).is_some()
    }

    /// Type name of the timeseries
    #[node_func(safe = false)]
    fn ts_dtype(
        node: &NodeInner,
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
    #[node_func(safe = false, valid = false)]
    fn ts_len(
        node: &NodeInner,
        /// Name of the timeseries
        name: &str,
        /// Do not error if timeseries does't exist
        safe: bool,
        /// Only count valid data (skip nulls)
        valid: bool,
    ) -> Result<Option<usize>, String> {
        match node.try_ts(name) {
            Ok(s) => {
                if valid {
                    Ok(Some(s.series().len_valid()))
                } else {
                    Ok(Some(s.series().len()))
                }
            }
            Err(_) if safe => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Convert the timeseries to complete if it doesn't have gaps
    #[node_func]
    fn ts_complete(
        node: &mut NodeInner,
        /// Name of the timeseries
        name: &str,
    ) -> Result<(), String> {
        match node.ts_map_mut().remove(name) {
            RSome(s) => {
                node.ts_map_mut().insert(name.into(), s.maybe_complete());
                Ok(())
            }
            RNone => Err(format!("Timeseris {name} doesn't exist")),
        }
    }

    /** Print the given timeseries values in csv format
    # TODO
    - save to file instead of showing with `outfile: Option<PathBuf>`
    */
    #[node_func(header = true, missing = "")]
    fn ts_print(
        node: &NodeInner,
        /// name of the timeseries
        name: &str,
        /// show header
        header: bool,
        /// number of head rows to show (all by default)
        head: Option<i64>,
        /// Show missing values as this string
        missing: String,
    ) -> Result<(), RString> {
        if let Some(ts) = node.ts(name) {
            let values = ts.str_values(&missing);
            if header {
                println!("time,{name}");
            }
            let head = head.map(|h| h as usize).unwrap_or_else(|| ts.len());
            for (t, v) in ts.timeline().lock().str_values().zip(values).take(head) {
                println!("{t},{v}");
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
    #[network_func(missing = "")]
    fn ts_print_csv(
        net: &Network,
        /// Name of the timeseries to save
        name: &str,
        /// number of head rows to show (all by default)
        head: Option<usize>,
        /// Include only these nodes (all by default)
        nodes: Option<HashSet<String>>,
        /// Show missing values as this string
        missing: &str,
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
                values.push(ts.str_values(&missing).collect::<Vec<String>>());
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
                let row: Vec<&str> = values.iter().map(|v| v[i].as_str()).collect();
                println!("{t},{}", row.join(","));
            }
        }
        Ok(())
    }

    /// Write the given nodes to csv with given attributes and series
    #[network_func(missing = "")]
    fn series_csv(
        net: &Network,
        filter: Vec<bool>,
        /// Path to the output csv
        outfile: &Path,
        /// list of attributes to write
        attrs: Vec<String>,
        /// list of series to write
        series: Vec<String>,
        /// Show missing values as this string
        missing: &str,
    ) -> anyhow::Result<()> {
        let f = File::create(&outfile)?;
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
        for (node, _) in net.nodes().zip(filter).filter(|(_, f)| *f) {
            let node = node.lock();
            let attrs: Vec<String> = attrs
                .iter()
                .map(|a| node.attr(a).map(|a| a.to_string()).unwrap_or_default())
                .collect();
            let series: Vec<Vec<String>> = series
                .iter()
                .map(|a| {
                    node.series(a)
                        .map(|s| s.str_values(&missing).collect())
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
