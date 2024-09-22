use abi_stable::{
    std_types::{RString, RVec, Tuple2, Tuple4},
    StableAbi,
};
use cairo::Context;

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
