use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod core {
    use crate::attrs::{Date, DateTime};
    use crate::prelude::*;
    use abi_stable::std_types::{RNone, RSome, RString, Tuple2};
    use nadi_plugin::{env_func, network_func, node_func};
    use std::collections::HashMap;

    /// Convert string to double quoted form
    ///
    /// ```task
    /// env assert_eq(str_quote("value"), "\"value\"")
    /// env assert_eq(str_quote(true), "\"true\"")
    /// env assert_eq(str_quote(12), "\"12\"")
    /// ```
    #[env_func(quote_char = "\"")]
    fn str_quote(#[relaxed] vars: String, quote_char: &str) -> Result<String, String> {
        if quote_char.len() == 1 {
            Ok(format!(
                "{quote_char}{}{quote_char}",
                vars.replace(&quote_char.to_string(), &format!("\\{quote_char}"))
            ))
        } else {
            Err(format!(
                "Quote Character should be a single character {}",
                quote_char
            ))
        }
    }

    /// Count the number of true values in the array
    ///
    /// ```task
    /// env assert_eq(count([true, false, true, false]), 2)
    /// ```
    #[env_func]
    fn count(vars: &[bool]) -> usize {
        vars.iter().filter(|a| **a).count()
    }

    /// Count the number of nodes in the network
    ///
    /// ```task
    /// network assert_eq(count(), 0)
    /// network load_str("a -> b")
    /// network assert_eq(count(), 2)
    /// node.sel = INDEX < 1
    /// network assert_eq(count(nodes.sel), 1)
    /// ```
    #[network_func]
    fn count(net: &Network, vars: Option<Vec<bool>>) -> usize {
        if let Some(v) = vars {
            v.iter().filter(|a| **a).count()
        } else {
            net.nodes().count()
        }
    }

    /// Get the name of the outlet node
    ///
    /// ```task
    /// network load_str("a -> b")
    /// network assert_eq(outlet(), "b")
    /// ```
    #[network_func]
    fn outlet(net: &Network) -> Option<String> {
        net.outlet().map(|o| o.lock().name().to_string())
    }

    /// Get the attr of the provided node
    ///
    /// ```task
    /// network load_str("a -> b")
    /// network assert_eq(node_attr("a", "NAME"), "a")
    /// ```
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

    /// Get a attrmap with node name and attributes
    ///
    /// ```task
    /// network load_str("a -> b")
    /// network assert_eq(node_map("NAME"), attrmap(a="a", b="b"))
    /// network assert_eq(node_map("INDEX"), attrmap(a=1, b=0))
    /// network assert_eq(node_map(), attrmap(a=1, b=0))
    /// ```
    #[network_func(invert = false)]
    fn node_map(
        net: &Network,
        /// attribute to get (defaults to INDEX)
        attribute: Option<String>,
        /// invert the map key and value
        invert: bool,
    ) -> Result<AttrMap, String> {
        let attr = attribute.unwrap_or("INDEX".into());
        Ok(net
            .nodes()
            .map(|n| -> Result<(RString, Attribute), String> {
                let n = n.lock();
                let key: RString = n.name().to_string().into();
                let val: Attribute = n
                    .attr_dot(&attr)?
                    .ok_or("Attribute Not Found".to_string())?
                    .clone();
                if invert {
                    Ok((RString::try_from_attr_relaxed(&val)?, key.into()))
                } else {
                    Ok((key, val))
                }
            })
            .collect::<Result<HashMap<RString, Attribute>, String>>()?
            .into())
    }

    /// Insert a key and value to a attrmap
    ///
    /// ```task
    /// env.x = attrmap(a=1, b=2)
    /// node assert_eq(insert(x, c, 3), attrmap(a=1, b=2, c=3))
    /// ```
    #[env_func]
    fn insert(mut map: AttrMap, key: RString, value: Attribute) -> AttrMap {
        map.insert(key, value);
        map
    }

    /// Count the number of input nodes in the node
    ///
    /// ```task
    /// network load_str("a -> b\n b -> d\n c -> d")
    /// node assert_eq(inputs_count(), length(inputs._))
    /// ```
    #[node_func]
    fn inputs_count(node: &NodeInner) -> usize {
        node.inputs().len()
    }

    /// Get attributes of the input nodes
    ///
    /// This is equivalent to using the `inputs` keyword
    /// ```task
    /// network load_str("a -> b\n b -> d\n c -> d")
    /// node assert_eq(inputs_attr("NAME"), inputs.NAME)
    /// ```
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

    /// Get attributes of the input nodes in map format
    ///
    /// ```task
    /// network load_str("a -> b\n b -> d\n c -> d")
    /// node assert_eq(node[b].inputs_map("NAME"), {a="a"})
    /// ```
    #[node_func(attr = "NAME")]
    fn inputs_map(
        node: &NodeInner,
        /// Attribute to get from inputs
        attr: String,
    ) -> Result<Attribute, String> {
        let attrs = node
            .inputs()
            .iter()
            .map(|n| {
                let n = n.lock();
                let name: RString = n.name().to_string().into();
                n.try_attr(&attr).map(|a| (name, a))
            })
            .collect::<Result<HashMap<RString, Attribute>, String>>()?;
        Ok(Attribute::Table(attrs.into()))
    }

    /// Node has an outlet or not
    ///
    /// This is equivalent to using `output._?`, as `_` is a dummy
    /// variable that will always be present in all cases, it being
    /// absent is because there is no output/outlet of that node.
    ///
    /// ```task
    /// network load_str("a -> b\n b -> d\n c -> d")
    /// node assert_eq(has_outlet(), output._?)
    /// ```
    #[node_func]
    fn has_outlet(node: &NodeInner) -> bool {
        node.output().is_some()
    }

    /// Get attributes of the output node
    ///
    /// This is equivalent to using the `output` keyword
    /// ```task
    /// network load_str("a -> b\n b -> d\n c -> d")
    /// node(output._?) assert_eq(output_attr("NAME"), output.NAME)
    /// ```
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
    ///
    /// ```task
    /// env assert_eq(type_name(true), "Bool")
    /// env assert_eq(type_name([true, 12]), "Array")
    /// env assert_eq(type_name([true, 12], recursive=true), ["Bool", "Integer"])
    /// env assert_eq(type_name("true"), "String")
    /// ```
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
    ///
    /// ```task
    /// env assert(isna(nan + 5))
    /// ```
    #[env_func]
    fn isna(val: f64) -> bool {
        val.is_nan()
    }

    /// check if a float is +/- infinity
    ///
    /// ```task
    /// env assert(isinf(12.0 / 0))
    /// ```
    #[env_func]
    fn isinf(val: f64) -> bool {
        val.is_infinite()
    }

    /// make a float from value
    ///
    /// ```task
    /// env assert_eq(float(5), 5.0)
    /// env assert_eq(float("5.0"), 5.0)
    /// ```
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
    ///
    /// ```task
    /// env assert_eq(str(nan + 5), "nan")
    /// env assert_eq(str(2 + 5), "7")
    /// env assert_eq(str(12.34), "12.34")
    /// env assert_eq(str("nan + 5"), "nan + 5")
    /// env assert_eq(str("true", quote=true), "\"true\"")
    /// ```
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
    ///
    /// ```task
    /// env assert_eq(int(5.0), 5)
    /// env assert_eq(int(5.1), 5)
    /// env assert_eq(int("45"), 45)
    /// env assert_eq(int("5.0", strfloat=true), 5)
    /// ```
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
    ///
    /// ```task
    /// env assert_eq(array(5, true), [5, true])
    /// ```
    #[env_func]
    fn array(
        /// List of attributes
        #[args]
        attributes: &[Attribute],
    ) -> Attribute {
        Attribute::Array(attributes.to_vec().into())
    }

    /// make an array combining the arrays from arguments
    ///
    /// ```task
    /// env assert_eq(array(5, true), [5, true])
    /// ```
    #[env_func(err_diff_len = true)]
    fn zip(
        /// List of attributes
        #[args]
        attributes: &[Attribute],
        /// error if different lengths
        err_diff_len: bool,
    ) -> Result<Attribute, String> {
        if attributes.is_empty() {
            return Ok(Attribute::Array(vec![].into()));
        }
        let mut attrs = Vec::with_capacity(attributes.len());
        let mut min_len = usize::MAX;
        for (i, attr) in attributes.iter().enumerate() {
            if let Attribute::Array(ar) = attr {
                if ar.len() < min_len {
                    min_len = ar.len();
                }
                attrs.push(ar);
            } else {
                return Err(format!(
                    "{i}th argument is {}, not an array",
                    attr.type_name()
                ));
            }
        }
        if err_diff_len {
            if attrs.iter().any(|a| a.len() != min_len) {
                return Err(format!("Lengths of the arguments are not the same"));
            }
        }
        let mut zipped = Vec::with_capacity(min_len);
        for j in 0..min_len {
            // we already made sure the min_len is valid, but using
            // filter map just in case
            let vals = attrs
                .iter()
                .filter_map(|a| a.get(j).cloned())
                .collect::<Vec<Attribute>>();
            zipped.push(Attribute::Array(vals.into()));
        }
        Ok(zipped.into())
    }

    /// make an attrmap from the arguments
    ///
    /// ```task
    /// env assert_eq(attrmap(val=5), {val=5})
    /// ```
    #[env_func]
    fn attrmap(
        /// name and values of attributes
        #[kwargs]
        attributes: &AttrMap,
    ) -> Attribute {
        Attribute::Table(attributes.clone())
    }

    /// make an attrmap from a list of (k, v)
    ///
    /// ```task
    /// env assert_eq(to_attrmap([["val", 5]]), {val=5})
    /// ```
    #[env_func]
    fn to_attrmap(
        /// name and values of attributes
        key_val: Vec<(RString, Attribute)>,
    ) -> AttrMap {
        key_val
            .into_iter()
            .collect::<HashMap<RString, Attribute>>()
            .into()
    }

    /// format the attribute as a json string
    ///
    /// ```task
    /// env assert_eq(json(5), "5")
    /// env assert_eq(json([5, true]), "[5, true]")
    /// env assert_eq(json({a=5}), "{\"a\": 5}")
    /// ```
    #[env_func]
    fn json(
        /// attribute to format
        value: Attribute,
    ) -> String {
        value.to_json()
    }

    /// append a value to an array
    ///
    /// ```task
    /// env assert_eq(append([4], 5), [4, 5])
    /// ```
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

    /// flatten the given list of arrays into a single one
    ///
    /// If any argument is not an array, then it will treat them as
    /// single element array
    ///
    /// ```task
    /// env assert_eq(flatten([[4], [5, 6]]), [4, 5, 6])
    /// ```
    #[env_func]
    fn flatten(
        /// List of attributes
        array: Vec<Attribute>,
    ) -> Attribute {
        let mut attr_list = Vec::new();
        for attr in array {
            if let Attribute::Array(a) = attr {
                a.into_iter().for_each(|v| {
                    attr_list.push(v);
                });
            } else {
                attr_list.push(attr);
            }
        }
        Attribute::Array(attr_list.into())
    }

    /// append a value to an array
    ///
    /// ```task
    /// env assert_eq(append([4], 5), [4, 5])
    /// ```
    #[env_func]
    fn drop_nan(
        /// List of attributes
        array: Vec<Attribute>,
    ) -> Attribute {
        Attribute::Array(
            array
                .into_iter()
                .filter(|f| !f.is_nan())
                .collect::<Vec<Attribute>>()
                .into(),
        )
    }

    /// length of an array or hashmap
    ///
    /// ```task
    /// env assert_eq(length([4, 5]), 2)
    /// env assert_eq(length({x=4, y=5}), 2)
    /// ```
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

    /// get the current date time
    #[env_func]
    fn now() -> DateTime {
        let now = chrono::Local::now();
        let date: Date = now.date_naive().into();
        date.with_time(now.time().into())
    }

    /// get the current date
    #[env_func]
    fn today() -> Date {
        chrono::Local::now().date_naive().into()
    }

    /// hour from time/datetime
    ///
    /// ```task
    /// env assert_eq(hour(12:14), 12)
    /// env assert_eq(hour(1223-12-14T15:19), 15)
    /// ```

    #[env_func]
    fn hour(
        /// Time or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Time(t) => t.hour,
            Attribute::DateTime(dt) => dt.time.hour,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// minute from time/datetime
    ///
    /// ```task
    /// env assert_eq(minute(12:14), 14)
    /// env assert_eq(minute(1223-12-14T15:19), 19)
    /// ```
    #[env_func]
    fn minute(
        /// Time or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Time(t) => t.min,
            Attribute::DateTime(dt) => dt.time.min,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// second from time/datetime
    ///
    /// ```task
    /// env assert_eq(second(12:14), 0)
    /// env assert_eq(second(1223-12-14T15:19:12), 12)
    /// ```
    #[env_func]
    fn second(
        /// Time or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Time(t) => t.sec,
            Attribute::DateTime(dt) => dt.time.sec,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// nanosecond from time/datetime
    ///
    /// ```task
    /// env assert_eq(nanosecond(12:14), 0)
    /// env assert_eq(nanosecond(1223-12-14T15:19), 0)
    /// ```
    #[env_func]
    fn nanosecond(
        /// Time or DateTime
        value: Attribute,
    ) -> Result<Attribute, String> {
        let val = match value {
            Attribute::Time(t) => t.nanosecond,
            Attribute::DateTime(dt) => dt.time.nanosecond,
            _ => {
                return Err(format!(
                    "Got {} instead of date/datetime",
                    value.type_name()
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// year from date/datetime
    ///
    /// ```task
    /// env assert_eq(year(1223-12-12), 1223)
    /// env assert_eq(year(1223-12-12T12:12), 1223)
    /// env assert_eq(year(1223-12-12 12:12:08), 1223)
    /// ```
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
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// month from date/datetime
    ///
    /// ```task
    /// env assert_eq(month(1223-12-14), 12)
    /// env assert_eq(month(1223-12-14T15:19), 12)
    /// ```
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
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// day from date/datetime
    ///
    /// ```task
    /// env assert_eq(day(1223-12-14), 14)
    /// env assert_eq(day(1223-12-14T15:19), 14)
    /// ```
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
                ));
            }
        };
        Ok(Attribute::Integer(val.into()))
    }

    /// Minimum of the variables
    ///
    /// ```task
    /// env assert_eq(min_num([1, 2, 3]), 1)
    /// env assert_eq(min_num([1.0, 2, 3]), 1.0)
    /// env assert_eq(min_num([1, 2, 3], start = 0), 0)
    /// ```
    #[env_func(start=f64::INFINITY)]
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
    /// ```task
    /// env assert_eq(max_num([1, 2, 3.0]), 3.0)
    /// env assert_eq(max_num([1.0, 2, 3]), 3)
    /// env assert_eq(max_num([1, inf, 3], 0), inf)
    /// ```
    #[env_func(start=-f64::INFINITY)]
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
    ///
    /// ```task
    /// env assert_eq(min([1, 2, 3], 100), 1)
    /// env assert_eq(min([1.0, 2, 3], 100), 1.0)
    /// env assert_eq(min([1, 2, 3], inf), 1)
    /// env assert_eq(min(["b", "a", "d"], "zzz"), "a")
    /// ```
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

    /// Maximum of the variables
    ///
    /// ```task
    /// env assert_eq(max([1, 2, 3], -1), 3)
    /// env assert_eq(max([1.0, 2, 3], -1), 3)
    /// env assert_eq(max([1, 2, 3], -inf), 3)
    /// env assert_eq(max(["b", "a", "d"], ""), "d")
    /// ```
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
    ///
    /// This function is for numeric attributes. You need to give the
    /// start attribute so that data type is valid.
    ///
    /// ```task
    /// env assert_eq(sum([2, 3, 4]), 9)
    /// env assert_eq(sum([2, 3, 4], start=0.0), 9.0)
    /// ```
    #[env_func(start = 0)]
    fn sum(vars: Vec<Attribute>, start: Attribute) -> Result<Attribute, EvalError> {
        let mut val = start;
        for r in vars {
            val = (val + r)?
        }
        Ok(val)
    }

    /// Product of the variables
    ///
    /// This function is for numerical values/attributes
    ///
    /// ```task
    /// env assert_eq(prod([1, 2, 3]), 6)
    /// env assert_eq(prod([1.0, 2, 3]), 6.0)
    /// ```
    #[env_func(start = 1)]
    fn prod(vars: Vec<Attribute>, start: Attribute) -> Result<Attribute, EvalError> {
        let mut val = start;
        for r in vars {
            val = (val * r)?
        }
        Ok(val)
    }

    /// Get a list of unique string values
    ///
    /// The order of the strings returned is not guaranteed
    ///
    /// ```task
    /// env.uniq = unique_str(["hi", "me", "hi", "you"]);
    /// env assert_eq(length(uniq), 3)
    /// ```
    #[env_func]
    fn unique_str(#[relaxed] vars: Vec<String>) -> Vec<String> {
        vars.into_iter()
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect()
    }

    /// Get a count of unique string values
    ///
    /// ```task
    /// env assert_eq(
    ///     count_str(["Hi", "there", "Deliah", "Hi"]),
    ///     {Hi = 2, there = 1, Deliah=1}
    /// )
    /// ```
    #[env_func]
    fn count_str(#[relaxed] vars: Vec<String>) -> HashMap<String, usize> {
        let mut counts: HashMap<String, usize> = HashMap::new();
        vars.into_iter().for_each(|v| {
            let v = counts.entry(v).or_insert(0);
            *v += 1;
        });
        counts
    }

    /// Concat the strings
    ///
    /// ```task
    /// env assert_eq(concat("Hello", "World", join=" "), "Hello World")
    /// ```
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

    /// Generate integer array, end is not included
    ///
    /// ```task
    /// env assert_eq(range(1, 5), [1, 2, 3, 4])
    /// ```
    #[env_func]
    fn range(start: i64, end: i64) -> Vec<i64> {
        (start..end).collect()
    }

    /// Assert the condition is true
    ///
    /// Use `assert_eq`/`assert_neq` if you are testing equality for
    /// better error message.
    ///
    /// ```task
    /// env assert(true)
    /// ```
    #[env_func(note = "Condition False")]
    fn assert(condition: bool, note: String) -> Result<(), String> {
        if condition {
            Ok(())
        } else {
            Err(format!("Assert Failed: {note}"))
        }
    }

    /// Assert the two values are equal
    ///
    /// This function is for testing the code, as well as for
    /// terminating the execution when certain values are not equal
    ///
    /// ```task
    /// env assert_eq(1, 1)
    /// env assert_eq(true, 1 > 0)
    /// env assert_eq("string val", concat("string", " ", "val"))
    /// ```
    #[env_func]
    fn assert_eq(left: Attribute, right: Attribute) -> Result<(), String> {
        if left == right {
            Ok(())
        } else {
            Err(format!("Assert Failed: {left:?} ≠ {right:?}"))
        }
    }

    /// Assert the two values are not equal
    ///
    /// This function is for testing the code, as well as for
    /// terminating the execution when certain values are not equal
    ///
    /// ```task
    /// env assert_neq(1, 1.0)
    /// env assert_neq(true, 1 < 0)
    /// env assert_neq("string val", concat("string", "val"))
    /// ```
    #[env_func]
    fn assert_neq(left: Attribute, right: Attribute) -> Result<(), String> {
        if left != right {
            Ok(())
        } else {
            Err(format!("Assert Failed: Both side is {left:?}"))
        }
    }
}
