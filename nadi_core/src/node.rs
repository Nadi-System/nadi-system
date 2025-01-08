use crate::{
    attrs::{AttrMap, Attribute, HasAttributes},
    prelude::HasTimeSeries,
    timeseries::TsMap,
};
use abi_stable::{
    external_types::RMutex,
    std_types::{
        RArc,
        ROption::{self, RSome},
        RString, RVec,
    },
    StableAbi,
};

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
    pub(crate) timeseries: TsMap,
    /// List of immediate inputs
    pub(crate) inputs: RVec<Node>,
    /// Output of the node if present
    pub(crate) output: ROption<Node>,
}

impl HasAttributes for NodeInner {
    fn attr_map(&self) -> &AttrMap {
        &self.attributes
    }

    fn attr_map_mut(&mut self) -> &mut AttrMap {
        &mut self.attributes
    }
}

impl HasTimeSeries for NodeInner {
    fn ts_map(&self) -> &TsMap {
        &self.timeseries
    }

    fn ts_map_mut(&mut self) -> &mut TsMap {
        &mut self.timeseries
    }
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
}
