use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod visuals {
    use anyhow::Context;
    use chrono::NaiveDate;
    use nadi_core::abi_stable::std_types::{RSome, RString, RVec};
    use nadi_core::attr_map;
    use nadi_core::attrs::Date;
    use nadi_core::functions::FunctionRet;
    use nadi_core::nadi_plugin::FromAttribute;
    use nadi_core::nadi_plugin::{env_func, network_func};
    use nadi_core::prelude::*;
    use nadi_core::timeseries::{HasTimeSeries, Series, TimeLine};
    use std::path::Path;
    use std::str::FromStr;
    use svg::node::element::*;
    use svg::Document;

    #[derive(FromAttribute, Debug)]
    struct Settings {
        top: f64,
        bottom: f64,
        right: f64,
        left: f64,
        deltax: f64,
        deltay: f64,
        fontsize: f64,
    }

    impl Default for Settings {
        fn default() -> Self {
            Self {
                top: 40.0,
                left: 10.0,
                right: 30.0,
                bottom: 10.0,
                deltax: 10.0,
                deltay: 10.0,
                fontsize: 8.0,
            }
        }
    }

    /// Generate the margins for the SVGs
    #[env_func(
        top = 40.0,
        left = 10.0,
        right = 30.0,
        bottom = 10.0,
        deltax = 10.0,
        deltay = 10.0,
        fontsize = 8.0
    )]
    fn svg_settings(
        top: f64,
        left: f64,
        right: f64,
        bottom: f64,
        deltax: f64,
        deltay: f64,
        fontsize: f64,
    ) -> AttrMap {
        attr_map!(top => top, left => left, right => right, bottom => bottom, deltax => deltax, deltay => deltay, fontsize => fontsize)
    }

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

    fn nodes_and_edges(net: &Network, label: &Template, set: &Settings) -> (Group, Group) {
        let count = net.nodes_count();
        let level = net
            .nodes()
            .map(|n| n.lock().level())
            .max()
            .unwrap_or_default();
        let mut nodes = Group::new();
        let mut edges = Group::new();
        for node in net.nodes() {
            let n = node.lock();
            let x = n.level() as f64 * set.deltax + set.left;
            let y = (count - 1 - n.index()) as f64 * set.deltay + set.top;
            let lab = label
                .render(&n)
                .unwrap_or_else(|_| label.original().to_string());
            nodes = nodes.add(n.node_point(x, y)).add(
                n.node_label(
                    set.deltax * (level + 1) as f64 + set.left,
                    y + set.fontsize / 2.0,
                    lab,
                )
                .set("font-size", set.fontsize),
            );
            if let RSome(out) = n.output() {
                let o = out.lock();
                let xo = o.level() as f64 * set.deltax + set.left;
                let yo = (count - 1 - o.index()) as f64 * set.deltay + set.top;
                edges = edges.add(n.node_line(x, y, xo, yo));
            }
        }
        (nodes, edges)
    }

    #[env_func]
    fn svg_open(path: RString) -> FunctionRet {
        FunctionRet::Image(path)
    }

    #[env_func]
    fn svg_open_multi(paths: RVec<RString>) -> FunctionRet {
        FunctionRet::Images(paths)
    }

    /// Exports the network as a svg
    #[network_func(
	label = Template::from_str("{NAME}").unwrap(),
	settings = Settings::default()
    )]
    fn svg_save(
        net: &mut Network,
        outfile: &Path,
        label: Template,
        width: Option<f64>,
        height: Option<f64>,
        bgcolor: Option<String>,
        settings: Settings,
    ) -> FunctionRet {
        let (nodes, edges) = nodes_and_edges(net, &label, &settings);

        let count = net.nodes_count();
        let level = net
            .nodes()
            .map(|n| n.lock().level())
            .max()
            .unwrap_or_default();
        let mut doc = Document::new().set(
            "viewBox",
            (
                0,
                0,
                width.unwrap_or(settings.left + settings.right + settings.deltax * level as f64),
                height.unwrap_or(
                    settings.top + settings.bottom + settings.deltay * (count - 2) as f64,
                ),
            ),
        );
        if let Some(col) = bgcolor {
            doc = doc.add(
                Rectangle::new()
                    .set("height", "100%")
                    .set("width", "100%")
                    .set("fill", col),
            );
        }
        match svg::save(outfile, &doc.add(edges).add(nodes)) {
            Ok(_) => FunctionRet::Image(outfile.to_string_lossy().into()),
            Err(e) => FunctionRet::Error(e.to_string().into()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    #[network_func(settings = Settings::default())]
    fn svg_ts_blocks(
        net: &mut Network,
        /// Output SVG file
        outfile: &Path,
        /// label to put for nodes
        label: Template,
        /// Name of the timeseries
        ts_name: String,
        /// Width of the SVG
        width: f64,
        /// width of the arrows
        arr_width: f64,
        /// Background color
        bgcolor: Option<String>,
        settings: Settings,
    ) -> anyhow::Result<()> {
        let count = net.nodes_count();

        let (nodes, edges) = nodes_and_edges(net, &label, &settings);
        let end = width - settings.right;
        let start = end - arr_width;

        let tl: TimeLine = match net
            .nodes()
            .filter_map(|n| n.lock().ts(&ts_name).map(|ts| ts.timeline()).cloned())
            .next()
        {
            Some(tl) => tl,
            None => return Err(anyhow::Error::msg("No TimeSeries")),
        };

        let (tl_start, tl_end, data_count) = {
            let tl = tl.lock();
            let tls = NaiveDate::parse_from_str(
                tl.str_values().next().context("No Timeline")?,
                tl.datetimefmt(),
            )?;
            let tle = NaiveDate::parse_from_str(
                tl.str_values().last().context("No Timeline")?,
                tl.datetimefmt(),
            )?;
            (tls, tle, tl.len() as f64)
        };

        let tls: Date = tl_start.into();
        let tle: Date = tl_end.into();
        let diff = (tl_end - tl_start).num_seconds() as f64;
        let y = tle.year - tls.year;
        let segments: Vec<(f64, u16)> = if y > 5 {
            (0..y)
                .map(|i| {
                    let y = tls.year + i + 1;
                    let d: NaiveDate = Date::new(y, 1, 1).into();
                    (
                        (d - tl_start).num_seconds() as f64 / diff * (end - start),
                        y,
                    )
                })
                .collect()
        } else if y > 2 {
            todo!()
        } else {
            todo!()
        };
        let axisy = settings.top - settings.deltay / 2.0;
        let points: Vec<String> = segments
            .iter()
            .map(|(x, _)| format!("{},{}", x + start, axisy))
            .collect();
        let mut labels = Group::new();
        let mut axis = Group::new();
        for (x, l) in segments {
            if l % 5 == 0 {
                labels = labels.add(
                    Text::new(l.to_string())
                        .set("x", 0)
                        .set("y", 0)
                        .set("text-anchor", "start")
                        .set("font-size", settings.fontsize * 0.8)
                        .set(
                            "transform",
                            format!(
                                "translate({}, {}) rotate(-30)",
                                start + x,
                                settings.top - settings.deltay
                            ),
                        ),
                );
                axis = axis.add(
                    Line::new()
                        .set("x1", start + x)
                        .set("y1", axisy - 3.0)
                        .set("x2", start + x)
                        .set("y2", axisy + 3.0)
                        .set("stroke", "#000000")
                        .set("stroke-width", 1),
                )
            } else {
                axis = axis.add(
                    Line::new()
                        .set("x1", start + x)
                        .set("y1", axisy - 1.5)
                        .set("x2", start + x)
                        .set("y2", axisy + 1.5)
                        .set("stroke", "#000000")
                        .set("stroke-width", 0.5),
                )
            }
        }
        axis = axis
            .add(
                Polyline::new()
                    .set(
                        "points",
                        format!("{start},{axisy} {} {end},{axisy}", points.join(" ")),
                    )
                    .set("fill", "none")
                    .set("stroke", "#000000")
                    .set("stroke-width", 0.5)
                    .set("marker-mid", "url(#mark1)"),
            )
            .add(labels);

        let mut lines = Group::new();
        for node in net.nodes() {
            let n = node.lock();
            if let Some(ts) = n.ts(&ts_name) {
                if !ts.is_timeline(&tl) {
                    return Err(anyhow::Error::msg("Not same timeline"));
                }

                let y = (count - n.index()) as f64 * settings.deltay + settings.top;
                let mut arrows = Group::new();

                match ts.series() {
                    Series::Complete(_) => {
                        let l = Line::new()
                            .set("x1", start)
                            .set("y1", y)
                            .set("x2", end)
                            .set("y2", y)
                            .set("stroke-width", 1.3)
                            .set("stroke", "#00aa00")
                            .set("marker-start", "url(#arr1)")
                            .set("marker-end", "url(#arr2)");
                        arrows = arrows.add(l).add(
                            Text::new("100%")
                                .set("x", end + 5.0)
                                .set("y", y + settings.fontsize / 2.0)
                                .set("text-anchor", "start")
                                .set("font-size", settings.fontsize * 0.8),
                        );
                    }
                    Series::Masked(ms, _) => {
                        for (pos, len) in ms.data_blocks(true) {
                            let lstart = start + (end - start) * pos as f64 / data_count;

                            let lend = start + (end - start) * (pos + len) as f64 / data_count;

                            let l = Line::new()
                                .set("x1", lstart)
                                .set("y1", y)
                                .set("x2", lend)
                                .set("y2", y)
                                .set("stroke-width", 1.3)
                                .set("stroke", "#0000aa")
                                .set("marker-start", "url(#arr1)")
                                .set("marker-end", "url(#arr2)");
                            arrows = arrows.add(l);
                        }
                        let vals = ms.get_valids();
                        let total = vals.len() as f64;
                        let count = vals.into_iter().filter(|v| *v).count() as f64;
                        arrows = arrows.add(
                            Text::new(format!("{:.2}", count / total))
                                .set("x", end + 5.0)
                                .set("y", y + 3.0)
                                .set("text-anchor", "start")
                                .set("font-size", settings.fontsize * 0.8),
                        );
                    }
                }
                lines = lines.add(arrows);
            }
        }
        let doc = Document::new().set(
            "viewBox",
            (
                0,
                0,
                width,
                settings.top + settings.deltay * count as f64 + settings.bottom,
            ),
        );
        let arr1 = Marker::new()
            .set("id", "arr1")
            .set("viewBox", (0, 0, 2, 2))
            .set("refY", 1)
            .add(
                Polyline::new()
                    .set("points", "2,2 0,1 2,0")
                    .set("fill", "none")
                    .set("stroke", "#0000aa")
                    .set("stroke-width", 0.5),
            );
        let arr2 = Marker::new()
            .set("id", "arr2")
            .set("viewBox", (0, 0, 2, 2))
            .set("refY", 1)
            .set("refX", 2)
            .add(
                Polyline::new()
                    .set("points", "0,0 2,1 0,2")
                    .set("fill", "none")
                    .set("stroke", "#0000aa")
                    .set("stroke-width", 0.5),
            );
        let mut doc = doc.add(Definitions::new().add(arr1).add(arr2));
        if let Some(col) = bgcolor {
            doc = doc.add(
                Rectangle::new()
                    .set("height", "100%")
                    .set("width", "100%")
                    .set("fill", col),
            );
        }
        let trans = format!("translate(0 {})", settings.deltay);
        svg::save(
            outfile,
            &doc.add(edges.set("transform", trans.clone()))
                .add(nodes.set("transform", trans))
                .add(lines)
                .add(axis),
        )?;
        Ok(())
    }
}
