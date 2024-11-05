use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod timeseries {

    use crate::prelude::*;
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

    /** Print the given timeseries values in csv format

    # Arguments
    - `name` - name
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
}
