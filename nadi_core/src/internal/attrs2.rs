use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod attrs {
    use crate::prelude::*;
    use abi_stable::std_types::Tuple2;
    use nadi_plugin::{network_func, node_func};

    use string_template_plus::Template;

    /// Set node attributes
    ///
    /// Use this function to set the node attributes of all nodes, or
    /// a select few nodes using the node selection methods (path or
    /// list of nodes)
    ///
    /// # Arguments
    /// - `key=value` - Kwargs of attr = value
    ///
    /// # Error
    /// The function should not error.
    ///
    /// # Example
    /// Following will set the attribute `a2d` to `true` for all nodes
    /// from `A` to `D`
    ///
    /// ```task
    /// node[A -> D] set_attrs(a2d = true)
    /// ```
    #[node_func]
    fn set_attrs(node: &mut NodeInner, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            node.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Retrive attribute
    #[node_func]
    fn get_attr(node: &mut NodeInner, attr: &str, default: Option<Attribute>) -> Option<Attribute> {
        node.attr(attr).cloned().or(default)
    }

    /// Check if the attribute is present
    #[node_func]
    fn has_attr(node: &mut NodeInner, attr: &str) -> bool {
        node.attr(attr).is_some()
    }

    /// simple if else condition
    #[node_func]
    fn ifelse(
        _node: &mut NodeInner,
        #[relaxed] cond: bool,
        iftrue: Attribute,
        iffalse: Attribute,
    ) -> Result<Attribute, String> {
        let v = if cond { iftrue } else { iffalse };
        Ok(v)
    }

    /// boolean and
    #[node_func]
    fn and(_node: &mut NodeInner, #[args] conds: &[Attribute]) -> bool {
        let mut ans = true;
        for c in conds {
            ans = ans && bool::from_attr_relaxed(c).unwrap();
        }
        ans
    }

    /// boolean or
    #[node_func]
    fn or(_node: &mut NodeInner, #[args] conds: &[Attribute]) -> bool {
        let mut ans = false;
        for c in conds {
            ans = ans || bool::from_attr_relaxed(c).unwrap();
        }
        ans
    }

    /// map values from the attribute based on the given table
    #[node_func]
    fn strmap(
        node: &mut NodeInner,
        attr: &str,
        attrmap: &AttrMap,
        default: Option<Attribute>,
    ) -> Option<Attribute> {
        let attr = String::from_attr_relaxed(node.attr(attr)?)?;
        attrmap.get(attr.as_str()).cloned().or(default)
    }

    /// if else condition with multiple attributes
    #[node_func]
    fn set_attrs_ifelse(
        node: &mut NodeInner,
        #[relaxed] cond: bool,
        #[kwargs] kwargs: &AttrMap,
    ) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let (t, f) = FromAttribute::try_from_attr(v)?;
            let v = if cond { t } else { f };
            node.set_attr(k, v);
        }
        Ok(())
    }

    /// Set node attributes based on string templates
    ///
    /// # Arguments
    /// - `attr=template` - Kwargs of attr = String template to render
    #[node_func]
    fn set_attrs_render(node: &mut NodeInner, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = node.render(&templ).map_err(|e| e.to_string())?;
            node.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }

    /// Set node attributes based on string templates
    ///
    /// # Arguments
    /// - toml - String template to render and load as TOML string
    #[node_func(echo = false)]
    fn load_toml_render(node: &mut NodeInner, toml: &Template, echo: bool) -> anyhow::Result<()> {
        let toml = format!("{}\n", node.render(toml)?);
        if echo {
            println!("{toml}");
        }
        let tokens = crate::parser::tokenizer::get_tokens(&toml)?;
        let attrs = crate::parser::attrs::parse(tokens)?;
        node.attr_map_mut().extend(attrs);
        Ok(())
    }

    /// map values from the attribute based on the given table
    #[node_func]
    fn float_transform(
        _node: &mut NodeInner,
        #[relaxed] value: f64,
        transformation: &str,
    ) -> Result<Attribute, String> {
        let value = if value == 0.0 { value + 0.1 } else { value };
        // let attr = String::from_attr_relaxed(node.attr(attr)?)?;
        Ok(Attribute::Float(match transformation {
            "log" => value.ln(),
            "log10" => value.log10(),
            "sqrt" => value.sqrt(),
            t => return Err(format!("Unknown Transformation: {t}")),
        }))
    }

    /// Set network attributes
    ///
    /// # Arguments
    /// - `key=value` - Kwargs of attr = value
    #[network_func]
    fn set_attrs(network: &mut Network, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            network.set_attr(k.as_str(), v.clone());
        }
        Ok(())
    }

    /// Set network attributes based on string templates
    ///
    /// # Arguments
    /// - `attr=template` - Kwargs of attr = String template to render
    #[network_func]
    fn set_attrs_render(network: &mut Network, #[kwargs] kwargs: &AttrMap) -> Result<(), String> {
        for Tuple2(k, v) in kwargs {
            let templ: Template = Template::try_from_attr(v)?;
            let text = network.render(&templ).map_err(|e| e.to_string())?;
            network.set_attr(k.as_str(), text.into());
        }
        Ok(())
    }
}
