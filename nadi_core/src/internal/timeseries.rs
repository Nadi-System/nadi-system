use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod timeseries {

    use crate::prelude::*;
    use abi_stable::std_types::{ROption, RString};
    use nadi_plugin::{node_func, network_func};
    use crate::timeseries::TimeSeries;
    use std::collections::{HashMap, HashSet};

    /// Print the list of available timeseries for the node
    ///
    /// # Arguments
    /// - `label`: Label the line with node name
    #[node_func(label = true)]
    fn list_ts(node: &mut NodeInner, label: bool) {
        if label {
            print!("{}: ", node.name());
        }
        for ts in node.ts_all() {
            print!("{}", ts.0);
        }
        println!();
    }

    /** Print the given timeseries values in csv format

    # Arguments
    - `name`: name of the timeseries
    - `header`: whether to show header or not
    - `head`: number of head rows to show (all by default)

    # TODO
    - save to file instead of showing with `outfile: Option<PathBuf>`
    */
    #[node_func(header = true)]
    fn show_ts(
        node: &mut NodeInner,
        name: &String,
        header: bool,
        head: Option<i64>,
    ) -> Result<ROption<Attribute>, RString> {
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
        Ok(None.into())
    }

    
    /// Save timeseries from all nodes into a single csv file
    ///
    /// TODO: error/not on unqual length
    /// TODO: error/not on no timeseries, etc...
    /// TODO: output to `file: PathBuf`
    #[network_func]
    fn show_ts_csv(net: &mut Network, name: String, head: Option<usize>, nodes: Option<HashSet<String>>) -> anyhow::Result<()> {
	let mut ts_nodes = vec![];
	let mut values = vec![];
	let mut timeline = None;
	for node in net.nodes(){
	    let node = node.lock();
	    if let Some(ref node_list) = nodes {
		if !node_list.contains(node.name()){
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
