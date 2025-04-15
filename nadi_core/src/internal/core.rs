use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod core {
    use crate::prelude::*;
    use abi_stable::std_types::{RNone, RSome, RString, Tuple2};
    use nadi_plugin::{env_func, network_func, node_func};
    use std::collections::HashMap;

    /// Count the number of nodes in the network
    ///
    /// If propagation is present, only counts those nodes
    #[network_func]
    fn count(net: &mut Network, #[prop] prop: &Propagation) -> Result<usize, String> {
        net.nodes_propagation(prop).map(|v| v.len())
    }

    /// Count the number of input nodes in the node
    #[node_func]
    fn inputs_len(node: &mut NodeInner) -> usize {
        node.inputs().len()
    }

    /// Get attributes of the input nodes
    #[node_func(attr = "NAME")]
    fn inputs(
        node: &mut NodeInner,
        /// Attribute to get from inputs
        attr: String,
    ) -> Result<Attribute, String> {
        let attrs: Vec<Attribute> = node
            .inputs()
            .iter()
            .map(|n| n.lock().try_attr(&attr))
            .collect::<Result<Vec<Attribute>, String>>()?;
        Ok(Attribute::Array(attrs.into()))
    }

    /// Node has an outlet or not
    #[node_func]
    fn has_outlet(node: &mut NodeInner) -> bool {
        node.output().is_some()
    }

    /// Get attributes of the output node
    #[node_func(attr = "NAME")]
    fn output(
        node: &mut NodeInner,
        /// Attribute to get from inputs
        attr: String,
    ) -> Result<Attribute, String> {
        match node.output() {
            RSome(n) => n.lock().try_attr(&attr),
            RNone => Err(String::from("Output doesn't exist for the node")),
        }
    }

    fn get_type_recur(attr: &Attribute) -> Attribute {
        match attr {
            Attribute::Array(a) => Attribute::Array(
                a.iter()
                    .map(get_type_recur)
                    .collect::<Vec<Attribute>>()
                    .into(),
            ),
            Attribute::Table(a) => Attribute::Table(
                a.iter()
                    .map(|Tuple2(k, v)| (k.clone(), get_type_recur(v)))
                    .collect::<HashMap<RString, Attribute>>()
                    .into(),
            ),
            a => Attribute::String(a.type_name().into()),
        }
    }

    /// Type name of the arguments
    #[env_func(recursive = false)]
    fn type_name(
        /// Argument to get type
        value: Attribute,
        /// Recursively check types for array and table
        recursive: bool,
    ) -> Attribute {
        if recursive {
            get_type_recur(&value)
        } else {
            Attribute::String(RString::from(value.type_name()))
        }
    }

    /// make a float from value
    #[env_func(parse = true)]
    fn float(
        /// Argument to convert to float
        value: Attribute,
        /// parse string to float
        parse: bool,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::String(s) if parse => s.parse::<f64>().map_err(|e| e.to_string())?,
            _ => f64::try_from_attr_relaxed(&value)?,
        };
        Ok(Attribute::Float(val))
    }

    /// make a string from value
    #[env_func(quote = false)]
    fn str(
        /// Argument to convert to float
        value: Attribute,
        /// quote it if it's literal string
        quote: bool,
    ) -> Result<Attribute, String> {
        let val = if quote {
            value.to_string()
        } else {
            String::try_from_attr_relaxed(&value)?
        };
        Ok(Attribute::String(val.into()))
    }

    /// make an int from the value
    #[env_func(parse = true, round = true, strfloat = false)]
    fn int(
        /// Argument to convert to int
        value: Attribute,
        /// parse string to int
        parse: bool,
        /// round float into integer
        round: bool,
        /// parse string first as float before converting to int
        strfloat: bool,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::String(s) if strfloat => {
                s.parse::<f64>().map_err(|e| e.to_string())?.round() as i64
            }
            Attribute::String(s) if parse => s.parse::<i64>().map_err(|e| e.to_string())?,
            Attribute::Float(f) if round => f.round() as i64,
            ref v => i64::try_from_attr_relaxed(v)?,
        };
        Ok(Attribute::Integer(val))
    }

    /// make an array from the arguments
    #[env_func]
    fn array(
        /// List of attributes
        #[args]
        attributes: &[Attribute],
    ) -> Attribute {
        Attribute::Array(attributes.to_vec().into())
    }

    /// make an array from the arguments
    #[env_func]
    fn attrmap(
        /// name and values of attributes
        #[kwargs]
        attributes: &AttrMap,
    ) -> Attribute {
        Attribute::Table(attributes.clone())
    }
}
