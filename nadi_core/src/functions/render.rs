use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod render {
    use crate::{Attribute, FromAttribute, NodeInner};
    use nadi_plugin::node_func;
    use string_template_plus::Template;

    #[derive(Debug)]
    struct TestVal(i64);

    // If you need custom values, you can implement FromAttribute, or
    // FromAttributeRelaxed for your type
    impl FromAttribute for TestVal {
        fn from_attr(value: &Attribute) -> Option<Self> {
            match value {
                Attribute::Float(v) => Some(v.floor() as i64),
                Attribute::Integer(v) => Some(*v),
                Attribute::Bool(v) => Some(*v as i64),
                Attribute::String(s) => s.parse::<i64>().ok(),
                _ => None,
            }
            .map(TestVal)
        }
    }

    /// Render the template based on the node attributes
    ///
    /// # Arguments
    /// - `template` - String template to render
    #[node_func(safe = false)]
    fn render(
        node: &mut NodeInner,
        template: Template,
        safe: bool,
        order: Option<TestVal>,
        #[relaxed] another: Option<f64>,
    ) -> Result<String, String> {
        let text = node.render(template).map_err(|e| e.to_string())?;
        println!("render={text:?} safe={safe:?} another={another:?} order={order:?}");
        Ok(text)
    }
}
