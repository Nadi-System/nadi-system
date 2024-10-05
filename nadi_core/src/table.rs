use std::{path::Path, str::FromStr};

use abi_stable::{
    std_types::{RString, RVec, Tuple2, Tuple4},
    StableAbi,
};
use cairo::Context;
use string_template_plus::Template;

use crate::{Network, Node};

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct NetworkPlot {
    nodes: RVec<NodePlot>,
    headers: RVec<RString>,
    edges: RVec<Tuple2<usize, usize>>,
    settings: NetworkPlotSettings,
}

impl NetworkPlot {
    pub fn get_width_height(&self, ctx: &Context) -> NetworkPlotArea {
        let max_lev = self.nodes.iter().map(|n| n.level).max().unwrap_or_default();
        let node_count = self.nodes.len();
        let height = (node_count - 1) as f64 * self.settings.dely + self.settings.offy * 2.0;
        let columns: Vec<f64> = self
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                let colwidth = match ctx.text_extents(h) {
                    Ok(e) => e.width(),
                    Err(_) => 0.0,
                };
                // get max width from all the text in the given columns
                self.nodes
                    .iter()
                    .filter_map(|n| ctx.text_extents(&n.labels[i]).ok())
                    .map(|e| e.width())
                    .fold(colwidth, f64::max)
            })
            .collect();
        let graph_width = (max_lev - 1) as f64 * self.settings.delx;
        let width = graph_width
            + columns.iter().sum::<f64>()
            + columns.len() as f64 * self.settings.colsep
            + self.settings.offy * 2.0;
        let stops: Vec<f64> = (0..columns.len())
            .map(|i| {
                graph_width
                    + self.settings.colsep * (i + 1) as f64
                    + columns[0..i].iter().sum::<f64>()
            })
            .collect();
        NetworkPlotArea {
            height,
            width,
            stops: stops.into(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct NetworkPlotArea {
    /// height of the area required to plot this
    height: f64,
    /// width of the area required to plot this
    width: f64,
    /// stops to start each column in the headers
    stops: RVec<f64>,
}

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct NetworkPlotSettings {
    delx: f64,
    dely: f64,
    offx: f64,
    offy: f64,
    colsep: f64,
}

impl Default for NetworkPlotSettings {
    fn default() -> Self {
        Self {
            delx: 20.0,
            dely: 20.0,
            offx: 10.0,
            offy: 10.0,
            colsep: 40.0,
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct NodePlot {
    index: usize,
    level: usize,
    /// rendered labels for each column
    labels: RVec<RString>,
    /// textwidth when the above labels are rendered
    colwidth: RVec<f64>,
    url: RString,
    size: u64,
    shape: NodeShape,
    color: Tuple4<u8, u8, u8, u8>,
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum NodeShape {
    #[default]
    Square,
    Rectangle(f64),
    Circle,
    Triangle,
    Ellipse(f64),
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum ColumnAlign {
    Left,
    Right,
    #[default]
    Center,
}

impl std::fmt::Display for ColumnAlign {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Left => '<',
                Self::Right => '>',
                Self::Center => '^',
            }
        )
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct Column {
    align: ColumnAlign,
    header: RString,
    template: RString,
}

impl Column {
    pub fn new(header: &str, template: &str, align: Option<ColumnAlign>) -> Self {
        Self {
            align: align.unwrap_or_default(),
            header: header.into(),
            template: template.into(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub struct Table {
    columns: RVec<Column>,
}

impl FromStr for Table {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cols = crate::parser::table::parse_table_complete(s).map_err(anyhow::Error::msg)?;
        Ok(Self {
            columns: cols.into(),
        })
    }
}

impl Table {
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_str(&contents)
    }

    pub fn render_contents(
        &self,
        net: &Network,
        conn: bool,
    ) -> Result<Vec<Vec<String>>, anyhow::Error> {
        let templates = self
            .columns
            .iter()
            .map(|c| Template::parse_template(&c.template))
            .collect::<Result<Vec<Template>, anyhow::Error>>()?;

        if conn {
            net.nodes()
                .zip(net.connections_utf8())
                .map(|(n, c)| {
                    let n = n.lock();
                    let mut row = templates
                        .iter()
                        .map(|t| n.render(t))
                        .collect::<Result<Vec<String>, anyhow::Error>>()?;
                    row.insert(0, c);
                    Ok(row)
                })
                .collect()
        } else {
            net.nodes()
                .map(|n| {
                    let n = n.lock();
                    let mut row = templates
                        .iter()
                        .map(|t| n.render(t))
                        .collect::<Result<Vec<String>, anyhow::Error>>()?;
                    Ok(row)
                })
                .collect()
        }
    }

    pub fn render_markdown(&self, net: &Network, conn: Option<String>) -> anyhow::Result<String> {
        let mut headers: Vec<&str> = self.columns.iter().map(|c| c.header.as_str()).collect();
        if let Some(c) = &conn {
            headers.insert(0, c);
        }
        let mut alignments: Vec<&ColumnAlign> = self.columns.iter().map(|c| &c.align).collect();
        if conn.is_some() {
            // conn needs to be left align for the ascii diagram to work
            alignments.insert(0, &ColumnAlign::Left);
        }
        let contents = self.render_contents(net, conn.is_some())?;
        let col_widths: Vec<usize> = headers
            .iter()
            .enumerate()
            .map(|(i, h)| {
                contents
                    .iter()
                    .map(|row| row[i].len())
                    .chain([h.len()])
                    .max()
                    .unwrap_or(1)
            })
            .collect();

        let mut table = String::new();
        table.push('|');
        for ((c, w), a) in headers.iter().zip(&col_widths).zip(&alignments) {
            table.push_str(&align_fmt_fn(c, a, w));
            table.push('|');
        }
        table.push('\n');
        table.push('|');
        for (w, a) in col_widths.iter().zip(&alignments) {
            let (pre, post) = match a {
                ColumnAlign::Left => (':', '-'),
                ColumnAlign::Right => ('-', ':'),
                ColumnAlign::Center => (':', ':'),
            };
            table.push_str(&format!("{pre}{:->1$}{post}|", "", w));
        }
        table.push('\n');
        for row in contents {
            table.push('|');
            for ((c, w), a) in row.iter().zip(&col_widths).zip(&alignments) {
                table.push_str(&align_fmt_fn(c, a, w));
                table.push('|');
            }
            table.push('\n');
        }
        Ok(table)
    }
}

fn align_fmt_fn(col: &str, align: &ColumnAlign, width: &usize) -> String {
    match align {
        ColumnAlign::Left => format!(" {:<1$} ", col, width),
        ColumnAlign::Right => format!(" {:>1$} ", col, width),
        ColumnAlign::Center => format!(" {:^1$} ", col, width),
    }
}
