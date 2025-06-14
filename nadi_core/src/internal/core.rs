use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod core {
    use crate::prelude::*;
    use abi_stable::std_types::{RNone, RSome, RString, Tuple2};
    use nadi_plugin::{env_func, network_func, node_func};
    use std::collections::HashMap;

    /// Count the number of true values in the array
    #[env_func]
    fn count(vars: &[bool]) -> usize {
        vars.iter().filter(|a| **a).count()
    }

    /// Count the number of nodes in the network
    #[network_func]
    fn count(net: &Network, vars: Option<Vec<bool>>) -> usize {
        if let Some(v) = vars {
            v.iter().filter(|a| **a).count()
        } else {
            net.nodes().count()
        }
    }

    /// Get the name of the outlet node
    #[network_func]
    fn outlet(net: &Network) -> Option<String> {
        net.outlet().map(|o| o.lock().name().to_string())
    }

    /// Get the attr of the provided node
    #[network_func(attribute = "_")]
    fn node_attr(
        net: &Network,
        ///  name of the node
        name: String,
        /// attribute to get
        attribute: String,
    ) -> Option<Attribute> {
        net.node_by_name(&name)
            .and_then(|n| n.lock().attr_dot(&attribute).ok().flatten().cloned())
    }

    /// Count the number of input nodes in the node
    #[node_func]
    fn inputs_count(node: &NodeInner) -> usize {
        node.inputs().len()
    }

    /// Get attributes of the input nodes
    #[node_func(attr = "NAME")]
    fn inputs_attr(
        node: &NodeInner,
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
    fn has_outlet(node: &NodeInner) -> bool {
        node.output().is_some()
    }

    /// Get attributes of the output node
    #[node_func(attr = "NAME")]
    fn output_attr(
        node: &NodeInner,
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

    /// check if a float is nan
    #[env_func]
    fn isna(val: f64) -> bool {
        val.is_nan()
    }

    /// check if a float is +/- infinity
    #[env_func]
    fn isinf(val: f64) -> bool {
        val.is_infinite()
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

    /// format the attribute as a json string
    #[env_func]
    fn json(
        /// attribute to format
        value: Attribute,
    ) -> String {
        value.to_json()
    }

    /// append a value to an array
    #[env_func]
    fn append(
        /// List of attributes
        array: Vec<Attribute>,
        value: Attribute,
    ) -> Attribute {
        let mut a = array;
        a.push(value);
        Attribute::Array(a.into())
    }

    /// length of an array or hashmap
    #[env_func]
    fn length(
        /// Array or a HashMap
        value: &Attribute,
    ) -> Result<usize, String> {
        match value {
            Attribute::Array(a) => Ok(a.len()),
            Attribute::Table(t) => Ok(t.len()),
            _ => Err(format!(
                "Got {} instead of array/attrmap",
                value.type_name()
            )),
        }
    }

    /// year from date/datetime
    #[env_func]
    fn year(
        /// Date or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Date(d) => d.year,
            Attribute::DateTime(dt) => dt.date.year,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ))
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// month from date/datetime
    #[env_func]
    fn month(
        /// Date or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Date(d) => d.month,
            Attribute::DateTime(dt) => dt.date.month,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ))
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// day from date/datetime
    #[env_func]
    fn day(
        /// Date or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Date(d) => d.day,
            Attribute::DateTime(dt) => dt.date.day,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ))
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// Minimum of the variables
    ///
    /// Starts with integer for type purpose, MAX float is larger than
    /// max int, so it'll be incorrect for large numbers
    #[env_func(start=i64::MAX)]
    fn min_num(vars: Vec<Attribute>, start: Attribute) -> Attribute {
        let mut val = start;
        for r in vars {
            if r < val {
                val = r;
            }
        }
        val
    }

    /// Minimum of the variables
    ///
    /// Starts with integer for type purpose, MAX float is larger than
    /// max int, so it'll be incorrect for large numbers
    #[env_func(start=i64::MIN)]
    fn max_num(vars: Vec<Attribute>, start: Attribute) -> Attribute {
        let mut val = start;
        for r in vars {
            if r > val {
                val = r;
            }
        }
        val
    }

    /// Minimum of the variables
    #[env_func]
    fn min(vars: Vec<Attribute>, start: Attribute) -> Attribute {
        let mut val = start;
        for r in vars {
            if r < val {
                val = r;
            }
        }
        val
    }

    /// Minimum of the variables
    #[env_func]
    fn max(vars: Vec<Attribute>, start: Attribute) -> Attribute {
        let mut val = start;
        for r in vars {
            if r > val {
                val = r;
            }
        }
        val
    }

    /// Sum of the variables
    #[env_func(start = 0)]
    fn sum(vars: Vec<Attribute>, start: Attribute) -> Result<Attribute, EvalError> {
        let mut val = start;
        for r in vars {
            val = (val + r)?
        }
        Ok(val)
    }

    /// Product of the variables
    #[env_func(start = 1)]
    fn prod(vars: Vec<Attribute>, start: Attribute) -> Result<Attribute, EvalError> {
        let mut val = start;
        for r in vars {
            val = (val * r)?
        }
        Ok(val)
    }

    /// Get a list of unique string values
    #[env_func]
    fn unique_str(vars: Vec<String>) -> Vec<String> {
        vars.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get a count of unique string values
    #[env_func]
    fn count_str(vars: Vec<String>) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        vars.into_iter().for_each(|v| {
            let v = counts.entry(v).or_insert(0);
            *v += 1;
        });
        counts
    }

    /// Concat the strings
    #[env_func(join = "")]
    fn concat(#[args] vars: &[Attribute], join: &str) -> String {
        let reprs: Vec<String> = vars
            .iter()
            .map(|a| match a {
                Attribute::String(s) => s.to_string(),
                x => x.to_string(),
            })
            .collect();
        reprs.join(join)
    }

    /// Generate integer array
    #[env_func]
    fn range(start: i64, end: i64) -> Vec<i64> {
        (start..end).collect()
    }
}
