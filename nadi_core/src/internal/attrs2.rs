use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod attributes {
    use crate::prelude::*;
    use abi_stable::std_types::Tuple2;
    use nadi_plugin::{env_func, network_func, node_func};
    use std::str::FromStr;
    use string_template_plus::Template;

    /// Set node attributes
    ///
    /// Use this function to set the node attributes of all nodes, or
    /// a select few nodes using the node selection methods (path or
    /// list of nodes)
    ///
    /// # Error
    /// The function should not error.
    ///
    /// # Example
    /// Following will set the attribute `a2d` to `true` for all nodes
    /// from `A` to `D`
    ///
    /// ```task
    /// network load_str("A -> B\n B -> D");
    /// node[A -> D] set_attrs(a2d = true)
    ///     ```
    /// This is equivalent to the following:
    /// ```task
    /// node[A->D].a2d = true;
    /// ```
    #[node_func]
    fn set_attrs(
        node: &mut NodeInner,
        /// Key value pairs of the attributes to set
        #[kwargs]
        attrs: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in attrs {
            node.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Retrive attribute
    ///
    /// ```task
    /// network load_str("A -> B\n B -> D");
    /// node assert_eq(get_attr("NAME"), NAME);
    /// ```
    #[node_func]
    fn get_attr(
        node: &NodeInner,
        /// Name of the attribute to get
        attr: &str,
        /// Default value if the attribute is not found
        default: Option<Attribute>,
    ) -> Option<Attribute> {
        node.attr(attr).cloned().or(default)
    }

    /// Check if the attribute is present
    ///
    /// ```task
    /// network load_str("A -> B\n B -> D");
    /// node.x = 90;
    /// node assert(has_attr("x"))
    /// node assert(!has_attr("y"))
    /// ```
    #[node_func]
    fn has_attr(
        node: &NodeInner,
        /// Name of the attribute to check
        attr: &str,
    ) -> bool {
        node.attr(attr).is_some()
    }

    /// Return the first Attribute that exists
    ///
    /// This is useful when you have a bunch of attributes that might
    /// be equivalent but are using different names. Normally due to
    /// them being combined from different datasets.
    ///
    /// ```task
    /// network load_str("A -> B\n B -> D");
    /// node.x = 90;
    /// node assert_eq(first_attr(["y", "x"]), 90)
    /// node assert_eq(first_attr(["x", "NAME"]), 90)
    /// ```
    #[node_func]
    fn first_attr(
        node: &NodeInner,
        /// attribute names
        attrs: &[String],
        /// Default value if not found
        default: Option<Attribute>,
    ) -> Option<Attribute> {
        for attr in attrs {
            if let Ok(Some(v)) = node.attr_dot(attr) {
                return Some(v.clone());
            }
        }
        default
    }

    /// map values from the attribute based on the given table
    ///
    /// ```task
    /// env.val = strmap("Joe", {Dave = 2, Joe = 20});
    /// env assert_eq(val, 20)
    /// env.val2 = strmap("Joe", {Dave=2}, default = 12);
    /// env assert_eq(val2, 12)
    /// ```
    #[env_func]
    fn strmap(
        /// Value to transform the attribute
        #[relaxed]
        attr: &str,
        /// Dictionary of key=value to map the data to
        attrmap: &AttrMap,
        /// Default value if key not found in `attrmap`
        default: Option<Attribute>,
    ) -> Option<Attribute> {
        attrmap.get(attr).cloned().or(default)
    }

    /// if else condition with multiple attributes
    ///
    /// ```task
    /// network load_str("a -> b");
    /// env.some_condition = true;
    /// node set_attrs_ifelse(
    ///     env.some_condition,
    ///     val1 = [1, 2],
    ///     val2 = ["a", "b"]
    /// );
    /// env assert_eq(nodes.val1, [1, 1])
    /// env assert_eq(nodes.val2, ["a", "a"])
    /// ```
    /// This is equivalent to using the if-else expression directly,
    ///
    /// ```task
    /// node.val1 = if (env.some_condition) {1} else {2};
    /// env assert_eq(nodes.val1, [1, 1])
    /// ```
    ///
    /// Furthermore if-else expression will give a lot more
    /// flexibility than this function in normal use cases. But this
    /// function is useful when you have to do something in a batch.
    #[node_func]
    fn set_attrs_ifelse(
        node: &mut NodeInner,
        /// Condition to check
        #[relaxed]
        cond: bool,
        /// key = [val1, val2] where key is set as first if `cond` is true else second
        #[kwargs]
        values: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in values {
            let (t, f) = FromAttribute::try_from_attr(v)?;
            let v = if cond { t } else { f };
            node.set_attr(k, v);
        }
        Ok(())
    }

    /// Set node attributes based on string templates
    ///
    /// This renders the template for each node, then it sets the
    /// values from the rendered results.
    ///
    /// ```task
    /// network load_str("a -> b");
    /// node set_attrs_render(val1 = "Node: {_NAME}");
    /// node[a] assert_eq(val1, "Node: a")
    /// ```
    #[node_func]
    fn set_attrs_render(
        node: &mut NodeInner,
        /// key value pair of attribute to set and the Template to render
        #[kwargs]
        kwargs: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = node.render(&templ).map_err(|e| e.to_string())?;
            node.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }

    /// Set node attributes by loading a toml from rendered template
    ///
    /// This function will render a string, and loads it as a toml
    /// string. This is useful when you need to make attributes based
    /// on some other variables that you can combine using the string
    /// template system.
    ///
    /// In most cases it is better to use the string manipulation
    /// functions and other environmental functions to get new
    /// attribute values to set.
    ///
    /// ```task
    /// network load_str("a -> b");
    /// node load_toml_render("label = \\\"Node: {_NAME}\\\"")
    /// node assert_eq(label, render("Node: {_NAME}"))
    /// ```
    #[node_func(echo = false)]
    fn load_toml_render(
        node: &mut NodeInner,
        /// String template to render and load as toml string
        toml: &Template,
        /// Print the rendered toml or not
        echo: bool,
    ) -> anyhow::Result<()> {
        let toml = format!("{}\n", node.render(toml)?);
        if echo {
            println!("{toml}");
        }
        let tokens = crate::parser::tokenizer::get_tokens(&toml);
        let attrs = crate::parser::attrs::parse(tokens)?;
        node.attr_map_mut().extend(attrs);
        Ok(())
    }

    /// Parse attribute from string
    ///
    /// ```task
    /// env assert_eq(parse_attr("true"), true)
    /// env assert_eq(parse_attr("123"), 123)
    /// env assert_eq(parse_attr("12.34"), 12.34)
    /// env assert_eq(parse_attr("\"my value\""), "my value")
    /// env assert_eq(parse_attr("1234-12-12"), 1234-12-12)
    /// ```
    #[env_func]
    fn parse_attr(
        /// String to parse into attribute
        toml: &str,
    ) -> Result<Attribute, String> {
        Attribute::from_str(toml).map_err(|e| e.to_string())
    }

    /// Parse attribute map from string
    ///
    /// ```task
    /// env assert_eq(parse_attrmap("y = true"), {y = true})
    /// env assert_eq(parse_attrmap(
    ///     "x = [1234-12-12, true]"),
    ///     {x = [1234-12-12, true]}
    /// )
    /// ```
    #[env_func]
    fn parse_attrmap(
        /// String to parse into attribute
        toml: String,
    ) -> Result<AttrMap, String> {
        let tokens = crate::parser::tokenizer::get_tokens(&toml);
        let attrs = crate::parser::attrs::parse(tokens).map_err(|e| e.to_string())?;
        Ok(attrs)
    }

    /// get the choosen attribute from Array or AttrMap
    ///
    /// ```task
    /// env.some_ar = ["this", 12, true];
    /// env.some_am = {x = "this", y = [12, true]};
    /// env assert_eq(get(some_ar, 0), "this")
    /// env assert_eq(get(some_ar, 2), true)
    /// env assert_eq(get(some_am, "x"), "this")
    /// env assert_eq(get(some_am, "y"), [12, true])
    /// ```
    #[env_func]
    fn get(
        /// Array or AttrMap Attribute to index
        parent: Attribute,
        /// Index value (Integer for Array, String for AttrMap)
        index: Attribute,
        /// Default value if the index is not present
        default: Option<Attribute>,
    ) -> Result<Attribute, String> {
        match (parent, index) {
            (Attribute::Array(ar), Attribute::Integer(ind)) => ar
                .get(ind as usize)
                .cloned()
                .or(default)
                .ok_or(format!("Index {ind} not found")),
            (Attribute::Table(am), Attribute::String(key)) => am
                .get(&key)
                .cloned()
                .or(default)
                .ok_or(format!("Index {key} not found")),
            (Attribute::Array(_), b) => Err(format!(
                "Array index should be Integer not {}",
                b.type_name()
            )),
            (Attribute::Table(_), b) => Err(format!(
                "AttrMap index should be String not {}",
                b.type_name()
            )),
            (a, _) => Err(format!("{} cannot be indexed", a.type_name())),
        }
    }

    /// Integer power
    ///
    /// ```task
    /// env assert_eq(powi(10.0, 2), 100.0)
    /// ```
    #[env_func]
    fn powi(
        /// base value
        #[relaxed]
        value: f64,
        power: i64,
    ) -> f64 {
        value.powi(power as i32)
    }

    /// Float power
    ///
    /// ```task
    /// env assert_eq(powf(100.0, 0.5), 10.0)
    /// ```
    #[env_func]
    fn powf(
        /// base value
        #[relaxed]
        value: f64,
        power: f64,
    ) -> f64 {
        value.powf(power)
    }

    /// Exponential
    ///
    /// ```task
    /// env assert_eq(log(exp(5.0)), 5.0)
    /// ```
    #[env_func]
    fn exp(#[relaxed] value: f64) -> f64 {
        value.exp()
    }

    /// Square Root
    /// ```task
    /// env assert_eq(sqrt(25.0), 5.0)
    /// ```
    #[env_func]
    fn sqrt(#[relaxed] value: f64) -> f64 {
        value.sqrt()
    }

    /// Logarithm of a value, natural if base not given
    ///
    /// ```task
    /// env assert_eq(log(exp(2.0)), 2.0)
    /// env assert_eq(log(2.0, 2.0), 1.0)
    /// ```
    #[env_func]
    fn log(#[relaxed] value: f64, base: Option<f64>) -> f64 {
        if let Some(b) = base {
            value.log(b)
        } else {
            value.ln()
        }
    }

    /// Float Division (same as / operator)
    ///
    /// ```task
    /// env assert_eq(float_div(10.0, 2), 10.0 / 2)
    /// ```
    #[env_func]
    fn float_div(
        /// numerator
        #[relaxed]
        value1: f64,
        /// denominator
        #[relaxed]
        value2: f64,
    ) -> f64 {
        value1 / value2
    }

    /// Float Multiplication (same as * operator)
    ///
    /// ```task
    /// env assert_eq(float_mult(5.0, 2), 5.0 * 2)
    /// ```
    #[env_func]
    fn float_mult(
        /// numerator
        #[relaxed]
        value1: f64,
        /// denominator
        #[relaxed]
        value2: f64,
    ) -> f64 {
        value1 * value2
    }

    /// Set network attributes
    ///
    /// # Arguments
    /// - `key=value` - Kwargs of attr = value
    ///
    /// ```task
    /// network set_attrs(val = 23.4)
    /// network assert_eq(val, 23.4)
    /// ```
    #[network_func]
    fn set_attrs(
        network: &mut Network,
        /// key value pair of attributes to set
        #[kwargs]
        attrs: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in attrs {
            network.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Set network attributes based on string templates
    ///
    /// It will set the attribute as a String
    ///
    /// ```task
    /// network.val = 23.4
    /// network set_attrs_render(val2 = "{val}05")
    /// network assert_eq(val2, "23.405")
    /// ```
    #[network_func]
    fn set_attrs_render(
        network: &mut Network,
        /// Kwargs of attr = String template to render
        #[kwargs]
        kwargs: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = network.render(&templ).map_err(|e| e.to_string())?;
            network.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }
}
