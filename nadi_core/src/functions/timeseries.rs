use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod timeseries {

    use crate::{
        attrs::{Date, DateTime, Time},
        timeseries::{TimeSeries, TimeSeriesValues},
        Attribute, NodeInner,
    };
    use abi_stable::std_types::{ROption, RString};

    use nadi_plugin::node_func;

    /// Print the list of available timeseries for the node
    /// # Arguments
    /// - `label` - Label the line with node name
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

    /// Print the given timeseries values in csv format
    /// # Arguments
    /// - `name` - name
    #[node_func(header = true)]
    fn show_ts(
        node: &mut NodeInner,
        name: String,
        _header: bool,
    ) -> Result<ROption<Attribute>, RString> {
        if let Some(ts) = node.ts(&name) {
            if let Some(values) = ts.values::<f64>() {
                let _start = ts.start();
                for v in values {
                    println!("{v}");
                }
                println!();
            } else {
                return Err(format!(
                    "Timeseries is `{}` not float in node `{}`",
                    ts.values_type(),
                    node.name()
                )
                .into());
            }
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

    // /// Print the given timeseries values in csv format
    // /// # Arguments
    // /// - `name` - name
    // /// - `header` - show the header row with node names
    // #[network_func(header = true)]
    // fn list_ts(net: &mut Network, name: String, header: bool) {
    //     if header {
    //         let columns: Vec<&str> = net.nodes().map(|n| n.lock().name()).collect();
    //         print!("{}: ", columns.join(","));
    //     }
    //     let values: Vec<Vec<String>> = net.nodes().map(|n| {
    //         n.lock()
    //             .ts(&name)
    //             .unwrap()
    //             .values::<f64>()
    //             .unwrap()
    //             .iter()
    //             .map(|v| v.to_string())
    //             .collect()
    //     });
    //     println!();
    // }

    /// set dummy timeseries by the given name and length and default value
    /// # Arguments
    /// - `name` - name
    /// - `start` - start datetime
    /// - `step` - time step in seconds
    /// - `length` - length
    /// - `value` - default value
    #[node_func(step = 24 * 60 * 60, length = 10u64)]
    fn dummy_ts(
        node: &mut NodeInner,
        name: String,
        #[relaxed] start: DateTime, // accept date only as well
        value: Attribute,
        step: i64,
        length: u64,
    ) {
        let vals = match value {
            Attribute::Bool(v) => {
                let vals: Vec<bool> = (0..length).map(|_| v).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::String(v) => {
                let vals: Vec<RString> = (0..length).map(|_| v.clone()).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::Integer(v) => {
                let vals: Vec<i64> = (0..length).map(|_| v).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::Float(v) => {
                let vals: Vec<f64> = (0..length).map(|_| v).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::Date(v) => {
                let vals: Vec<Date> = (0..length).map(|_| v.clone()).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::Time(v) => {
                let vals: Vec<Time> = (0..length).map(|_| v.clone()).collect();
                TimeSeriesValues::from(vals)
            }
            Attribute::DateTime(v) => {
                let vals: Vec<DateTime> = (0..length).map(|_| v.clone()).collect();
                TimeSeriesValues::from(vals)
            }
            v => {
                let vals: Vec<Attribute> = (0..length).map(|_| v.clone()).collect();
                TimeSeriesValues::from(vals)
            }
        };
        let ts = TimeSeries::new(start, step, vals);
        node.set_ts(&name, ts);
    }
}
