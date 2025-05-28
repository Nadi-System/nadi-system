use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod visuals {
    use crate::prelude::*;
    use crate::string_template::Template;
    use nadi_core::abi_stable::std_types::RSome;
    use nadi_plugin::network_func;
    use std::path::Path;
    use svg::node::element::*;
    use svg::Document;

    /// Set the node size of the nodes based on the attribute value
    #[network_func(minsize = 4.0, maxsize = 12.0)]
    fn set_nodesize_attrs(
        net: &Network,
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
            n.lock().set_node_size(s);
        });
        Ok(Attribute::Array(vec![max.into(), min.into()].into()))
    }

    /// Exports the network as a svg
    #[network_func(
	label = Template::parse_template("{_NAME}").unwrap(),
        x_spacing = 25u64,
        y_spacing = 25u64,
        offset = 10u64,
	twidth = 9.0,
        width = 500u64,
        height = 240u64,
    )]
    #[allow(clippy::too_many_arguments)]
    fn svg_save(
        net: &mut Network,
        outfile: &Path,
        label: Template,
        x_spacing: u64,
        y_spacing: u64,
        offset: u64,
        /// in average how many units each text character takes
        ///
        /// For auto calculating width of the page since we don't have Cairo
        twidth: f64,
        width: u64,
        height: u64,
        bgcolor: Option<String>,
        page_width: Option<u64>,
        page_height: Option<u64>,
    ) -> anyhow::Result<()> {
        let count = net.nodes_count() as u64;
        let level = net
            .nodes()
            .map(|n| n.lock().level())
            .max()
            .unwrap_or_default();
        let mut max_textlen: usize = 0;

        let mut nodes = Group::new();
        let mut edges = Group::new();
        for node in net.nodes() {
            let n = node.lock();
            let x = n.level() * x_spacing + offset;
            let y = (count - n.index() as u64) * y_spacing + offset;
            let lab = n
                .render(&label)
                .unwrap_or_else(|_| label.original().to_string());
            if lab.len() > max_textlen {
                max_textlen = lab.len();
            }
            nodes =
                nodes
                    .add(n.node_point(x, y))
                    .add(n.node_label((level + 2) * x_spacing, y, lab));
            if let RSome(out) = n.output() {
                let o = out.lock();
                let xo = o.level() * x_spacing + offset;
                let yo = (count - o.index() as u64) * y_spacing + offset;
                edges = edges.add(n.node_line(x, y, xo, yo));
            }
        }
        let mut doc = Document::new()
            .set(
                "viewBox",
                (
                    0,
                    0,
                    page_width.unwrap_or(
                        x_spacing * (level + 2)
                            + offset
                            + (twidth * max_textlen as f64).ceil() as u64,
                    ),
                    page_height.unwrap_or(2 * offset + y_spacing * count),
                ),
            )
            .set("height", height)
            .set("width", width);
        if let Some(col) = bgcolor {
            doc = doc.add(
                Rectangle::new()
                    .set("height", "100%")
                    .set("width", "100%")
                    .set("fill", col),
            );
        }
        svg::save(outfile, &doc.add(edges).add(nodes))?;
        Ok(())
    }
}
