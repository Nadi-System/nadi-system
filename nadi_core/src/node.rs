use crate::{
    attrs::{AttrMap, Attribute, FromAttribute},
    timeseries::TimeSeries,
};
use abi_stable::{
    external_types::RMutex,
    std_types::{
        RArc, RHashMap,
        ROption::{self, RSome},
        RString, RVec,
    },
    StableAbi,
};
use string_template_plus::{Render, RenderOptions, Template};

pub type Node = RArc<RMutex<NodeInner>>;

/// Create a new [`Node`]
pub fn new_node(index: usize, name: &str) -> Node {
    RArc::new(RMutex::new(NodeInner::new(index, name)))
}

/// Represents points with attributes and timeseries. These can be any
/// point as long as they'll be on the network and connection to each
/// other.
///
/// The attributes format is [`Attribute`], which has
/// [`Attribute::Array`] and [`Attribute::Table`] which means users
/// are free to make their own attributes with custom combinations and
/// plugins + functions that can work with those attributes.
///
/// Since attributes are loaded using TOML file, simple attributes can
/// be stored and parsed from strings, and complex ones can be saved in
/// different files and their path can be stored as node attributes.
///
/// Here is an example node attribute file. Here we have string,
/// float, int and boolean values.
/// ```toml
///     stn="smithland"
///     nat_7q10=12335.94850131619
///     orsanco_7q10=16900
///     lock=true
///     ...
/// ```
///    
#[repr(C)]
#[derive(StableAbi, Default, Clone)]
pub struct NodeInner {
    /// index of the current node in the [`crate::Network`]
    pub(crate) index: usize,
    /// name of the node
    pub(crate) name: RString,
    /// level represents the rank of the tributary, 0 for main branch
    /// and 1 for tributaries connected to main branch and so on
    pub(crate) level: u64,
    /// Number of inputs connected to the current node
    pub(crate) order: u64,
    /// Node attributes in a  Hashmap of [`RString`] to [`Attribute`]
    pub(crate) attributes: AttrMap,
    /// Hashmap of [`RString`] to [`TimeSeries`]
    pub(crate) timeseries: RHashMap<RString, TimeSeries>,
    /// List of immediate inputs
    pub(crate) inputs: RVec<Node>,
    /// Output of the node if present
    pub(crate) output: ROption<Node>,
}

impl NodeInner {
    pub fn new(index: usize, name: &str) -> Self {
        let mut node = Self {
            index,
            name: name.into(),
            ..Default::default()
        };
        node.set_attr("NAME", Attribute::String(name.into()));
        node.set_attr("INDEX", Attribute::Integer(index as i64));
        node
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn set_index(&mut self, index: usize) {
        self.index = index;
        self.set_attr("INDEX", Attribute::Integer(index as i64));
    }

    pub fn level(&self) -> u64 {
        self.level
    }

    pub fn order(&self) -> u64 {
        self.order
    }

    pub fn set_level(&mut self, level: u64) {
        self.level = level;
        self.set_attr("LEVEL", Attribute::Integer(level as i64));
    }

    pub fn set_order(&mut self, order: u64) {
        self.order = order;
        self.set_attr("ORDER", Attribute::Integer(order as i64));
    }

    // pub fn extend_attr(&mut self, attrs: &AttrMap) {
    //     self.attributes.extend(attrs);
    // }

    pub fn del_attr(&mut self, name: &str) -> bool {
        self.attributes.remove(name.into()).is_some()
    }

    pub fn set_attr(&mut self, name: &str, val: Attribute) {
        self.attributes.insert(name.into(), val);
    }

    pub fn attr(&self, name: &str) -> Option<&Attribute> {
        self.attributes.get(name)
    }

    pub fn try_attr<T: FromAttribute>(&self, name: &str) -> Result<T, String> {
        match self.attr(name) {
            Some(v) => FromAttribute::try_from_attr(v),
            None => Err(format!(
                "Attribute Error: Attribute {name} not found in Node"
            )),
        }
    }

    pub fn attrs(&self) -> &AttrMap {
        &self.attributes
    }

    pub fn attrs_mut(&mut self) -> &mut AttrMap {
        &mut self.attributes
    }

    pub fn set_ts(&mut self, name: &str, val: TimeSeries) {
        self.timeseries.insert(name.into(), val);
    }

    pub fn ts(&self, name: &str) -> Option<&TimeSeries> {
        self.timeseries.get(name)
    }

    pub fn try_ts(&self, name: &str) -> Result<&TimeSeries, String> {
        self.timeseries.get(name).ok_or(format!(
            "Node `{}` does not have timeseries `{name}`",
            self.name
        ))
    }

    pub fn ts_all(&self) -> &RHashMap<RString, TimeSeries> {
        &self.timeseries
    }

    pub fn inputs(&self) -> &[Node] {
        &self.inputs
    }

    pub(crate) fn inputs_mut(&mut self) -> &mut RVec<Node> {
        &mut self.inputs
    }

    pub fn add_input(&mut self, input: Node) {
        self.inputs.push(input);
    }

    pub fn unset_inputs(&mut self) {
        self.inputs = RVec::new();
    }

    pub fn order_inputs(&mut self) {
        self.inputs
            .sort_by(|a, b| b.lock().order.partial_cmp(&a.lock().order).unwrap());
    }

    pub fn output(&self) -> ROption<&Node> {
        self.output.as_ref()
    }

    pub fn set_output(&mut self, output: Node) {
        self.output = RSome(output);
    }

    pub fn unset_output(&mut self) -> ROption<Node> {
        self.output.take()
    }

    /// Move the node to the side (move the inputs to its output)
    pub fn move_aside(&mut self) {
        if let RSome(o) = self.output() {
            self.inputs().iter().for_each(|i| {
                o.lock().add_input(i.clone());
                i.lock().set_output(o.clone())
            });
        } else {
            self.inputs().iter().for_each(|i| {
                i.lock().unset_output();
            });
        }
        self.unset_inputs();
    }

    /// Move the network down one step, (swap places with its output)
    pub fn move_down(&mut self) {
        if let RSome(out) = self.unset_output() {
            let i = out
                .lock()
                .inputs()
                .iter()
                // HACK current node will fail to lock
                .position(|c| c.try_lock().is_none())
                .unwrap();
            let o = out.lock().inputs.remove(i);
            self.output = out.lock().output.clone();
            out.lock().set_output(o);
            self.add_input(out.clone());
        }
    }

    pub fn render(&self, template: &Template) -> anyhow::Result<String> {
        let mut op = RenderOptions::default();
        let used_vars = template.parts().iter().flat_map(|p| p.variables());
        for var in used_vars {
            if let Some(val) = self.attr(var) {
                op.variables.insert(var.to_string(), val.to_string());
            }
            if let Some(val) = var.strip_prefix('_') {
                if let Some(Attribute::String(s)) = self.attr(val) {
                    op.variables.insert(var.to_string(), s.to_string());
                }
            }
        }
        template.render(&op)
    }
}
