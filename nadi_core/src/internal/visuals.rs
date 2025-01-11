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
        attr: String,
        #[relaxed] default: Option<f64>,
        #[relaxed] minsize: f64,
        #[relaxed] maxsize: f64,
    ) -> Result<Attribute, String> {
        let values = if let Some(v) = default {
            net.nodes()
                .map(|n| n.lock().try_attr_relaxed::<f64>(&attr).unwrap_or(v))
                .collect::<Vec<f64>>()
        } else {
            net.nodes()
                .map(|n| n.lock().try_attr_relaxed::<f64>(&attr))
                .collect::<Result<Vec<f64>, String>>()?
        };
        let max = values.iter().fold(f64::MIN, |a, &b| f64::max(a, b));
        let min = values.iter().fold(f64::MAX, |a, &b| f64::min(a, b));
        let diff = max - min;
        let diffs = maxsize - minsize;
        values.into_iter().zip(net.nodes()).for_each(|(v, n)| {
            let s = (v - min) / diff * diffs + minsize;
            n.lock().set_attr(NODE_SIZE.0, s.into());
        });
        Ok(Attribute::Array(vec![max.into(), min.into()].into()))
    }
}
