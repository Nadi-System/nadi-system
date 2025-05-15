use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod visuals {
    use crate::graphics::node::NODE_SIZE;
    use crate::prelude::*;
    use nadi_plugin::network_func;

    /// Set the node size of the nodes based on the attribute value
    #[network_func(minsize = 4.0, maxsize = 12.0)]
    fn set_nodesize_attrs(
        net: &mut Network,
        /// Attribute values to use for size scaling
        #[relaxed]
        attrs: &[f64],
        /// minimum size of the node
        #[relaxed]
        minsize: f64,
        /// maximum size of the node
        #[relaxed]
        maxsize: f64,
    ) -> Result<Attribute, String> {
        let max = attrs.iter().fold(f64::MIN, |a, &b| f64::max(a, b));
        let min = attrs.iter().fold(f64::MAX, |a, &b| f64::min(a, b));
        let diff = max - min;
        let diffs = maxsize - minsize;
        attrs.iter().zip(net.nodes()).for_each(|(v, n)| {
            let s = (v - min) / diff * diffs + minsize;
            n.lock().set_attr(NODE_SIZE.0, s.into());
        });
        Ok(Attribute::Array(vec![max.into(), min.into()].into()))
    }
}
