use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod debug {

    use crate::{
        attrs::DateTime,
        timeseries::{TimeSeries, TimeSeriesValues},
        AttrMap, AttrSlice, Attribute, Network, NodeInner,
    };
    use abi_stable::std_types::Tuple2;
    use colored::Colorize;
    use nadi_plugin::{network_func, node_func};

    /// Print the args and kwargs on this function
    #[network_func]
    fn debug(net: &mut Network, #[args] args: AttrSlice, #[kwargs] kwargs: &AttrMap) {
        let mut args_str: Vec<String> = args
            .iter()
            .map(|a| Attribute::to_colored_string(a).to_string())
            .collect();
        let kwargs_str: Vec<String> = kwargs
            .iter()
            .map(|Tuple2(k, v)| format!("{}={}", k.to_string().blue(), v.to_colored_string()))
            .collect();
        args_str.extend(kwargs_str.into_iter());
        println!("Function Call: debug({})", args_str.join(", "));
        println!("Args: {args:?}");
        println!("KwArgs: {kwargs:?}");
    }
}
