use crate::{
    attrs::{AttrMap, Attribute, HasAttributes},
    timeseries::{HasSeries, HasTimeSeries, SeriesMap, TsMap},
};
use abi_stable::{
    external_types::{parking_lot::mutex::RMutexGuard, RMutex},
    std_types::{
        RArc, RDuration,
        ROption::{self, RNone, RSome},
        RString, RVec,
    },
    StableAbi,
};

/// Wrapper around thread safe Mutex of [`NodeInner`]
#[repr(C)]
#[derive(Clone, StableAbi)]
pub struct Node {
    /// Name of the node, can not be changed
    name: RString,
    /// Actual node data
    inner: RArc<RMutex<NodeInner>>,
}

impl std::fmt::Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "<Node {:?}>", self.name)
    }
}

impl<'a> Node {
    pub fn new(index: usize, name: &str) -> Self {
        Self {
            name: name.to_string().into(),
            inner: RArc::new(RMutex::new(NodeInner::new(index, name))),
        }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn lock(&'a self) -> RMutexGuard<'a, NodeInner> {
        self.inner.lock()
    }

    pub fn try_lock(&'a self) -> Option<RMutexGuard<'a, NodeInner>> {
        self.inner.try_lock().into_option()
    }

    pub fn try_lock_for(&'a self, dur: RDuration) -> Option<RMutexGuard<'a, NodeInner>> {
        self.inner.try_lock_for(dur).into_option()
    }
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
    /// x position of the node for graphical reasons
    pub x_pos: f64,
    /// y position of the node for graphical reasons
    pub y_pos: f64,
    /// level represents the rank of the tributary, 0 for main branch
    /// and 1 for tributaries connected to main branch and so on
    pub(crate) level: u64,
    /// order of the current node
    pub(crate) order: u64,
    /// Number of nodes inputs to the current node
    pub(crate) weight: u64,
    /// Node attributes in a  Hashmap of [`RString`] to [`Attribute`]
    pub(crate) attributes: AttrMap,
    /// Hashmap of [`RString`] to [`Series`]
    pub(crate) series: SeriesMap,
    /// Hashmap of [`RString`] to [`TimeSeries`]
    pub(crate) timeseries: TsMap,
    /// List of names of immediate inputs
    pub(crate) input_names: RVec<RString>,
    /// List of immediate inputs
    pub(crate) inputs: RVec<Node>,
    /// List of names of immediate outputs
    pub(crate) output_names: RVec<RString>,
    /// Output of the node if present
    pub(crate) outputs: RVec<Node>,
}

impl HasAttributes for NodeInner {
    fn node_name(&self) -> Option<&str> {
        Some(self.name())
    }
    fn attr_map(&self) -> &AttrMap {
        &self.attributes
    }

    fn attr_map_mut(&mut self) -> &mut AttrMap {
        &mut self.attributes
    }
}

impl HasSeries for NodeInner {
    fn series_map(&self) -> &SeriesMap {
        &self.series
    }

    fn series_map_mut(&mut self) -> &mut SeriesMap {
        &mut self.series
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
    /// new node data
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

    /// name of the node
    pub fn name(&self) -> &str {
        &self.name
    }

    /// index of the node
    pub fn index(&self) -> usize {
        self.index
    }

    /// set index of the node
    pub fn set_index(&mut self, index: usize) {
        self.index = index;
        self.set_attr("INDEX", Attribute::Integer(index as i64));
    }

    /// pos of the node
    pub fn pos(&self) -> (f64, f64) {
        (self.x_pos, self.y_pos)
    }

    /// set pos of the node
    pub fn set_pos(&mut self, pos: (f64, f64)) {
        self.x_pos = pos.0;
        self.y_pos = pos.1;
        self.set_attr(
            "POSITION",
            Attribute::Array(vec![Attribute::Float(pos.0), Attribute::Float(pos.1)].into()),
        );
    }

    /// level of the node
    pub fn level(&self) -> u64 {
        self.level
    }

    /// order of the node
    pub fn order(&self) -> u64 {
        self.order
    }

    /// weight of the node
    pub fn weight(&self) -> u64 {
        self.weight
    }

    /// set level of the node
    pub fn set_level(&mut self, level: u64) {
        self.level = level;
        self.set_attr("LEVEL", Attribute::Integer(level as i64));
    }

    /// set order of the node
    pub fn set_order(&mut self, order: u64) {
        self.order = order;
        self.set_attr("ORDER", Attribute::Integer(order as i64));
    }

    /// set weight of the node
    pub fn set_weight(&mut self, weight: u64) {
        self.weight = weight;
        self.set_attr("WEIGHT", Attribute::Integer(weight as i64));
    }

    /// single input node of the node, None if no inputs or multiple inputs
    pub fn input(&self) -> ROption<&Node> {
        if let [inp] = self.inputs.as_slice() {
            RSome(inp)
        } else {
            RNone
        }
    }

    /// input nodes of the node
    pub fn inputs(&self) -> &[Node] {
        &self.inputs
    }

    /// input nodes as a mutable reference
    pub(crate) fn inputs_mut(&mut self) -> &mut RVec<Node> {
        &mut self.inputs
    }

    /// check if the node is a leaf node
    pub fn is_leaf(&self) -> bool {
        self.inputs.is_empty()
    }

    pub fn input_names(&self) -> &[RString] {
        self.input_names.as_slice()
    }

    pub fn output_names(&self) -> &[RString] {
        self.output_names.as_slice()
    }

    pub fn edge_names(&self) -> impl Iterator<Item = &RString> {
        self.input_names().iter().chain(self.output_names())
    }

    pub fn refresh_input_names(&mut self) {
        self.input_names = self
            .inputs()
            .iter()
            .map(|a| a.name().to_string().into())
            .collect::<Vec<RString>>()
            .into();
    }

    pub fn refresh_output_names(&mut self) {
        self.output_names = self
            .outputs()
            .iter()
            .map(|a| a.name().to_string().into())
            .collect::<Vec<RString>>()
            .into();
    }

    /// add a input node to the node
    pub fn add_input(&mut self, input: Node) {
        self.inputs.push(input);
        self.refresh_input_names();
    }

    /// remove the input nodes of the node
    pub fn unset_inputs(&mut self) -> Vec<Node> {
        self.input_names = RVec::new();
        self.inputs.drain(..).collect()
    }

    pub fn remove_input(&mut self, input: &str) {
        self.inputs = self
            .inputs
            .drain(..)
            // should make sure no input nodes are locked at this time
            .filter(|n| n.name() != input)
            .collect::<Vec<_>>()
            .into();
        self.refresh_input_names();
    }

    /// order the input nodes in the network
    pub fn order_inputs(&mut self) {
        self.inputs.sort_by(|a, b| {
            b.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .order
                .partial_cmp(
                    &a.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .order,
                )
                .unwrap()
        });
        self.refresh_input_names();
    }

    /// single output node of the node, None if no outputs or multiple outputs
    pub fn output(&self) -> ROption<&Node> {
        if let [out] = self.outputs.as_slice() {
            RSome(out)
        } else {
            RNone
        }
    }

    /// output nodes of the node
    pub fn outputs(&self) -> &[Node] {
        &self.outputs
    }

    /// output nodes as a mutable reference
    pub(crate) fn outputs_mut(&mut self) -> &mut RVec<Node> {
        &mut self.outputs
    }

    /// check if the node is a root node
    pub fn is_root(&self) -> bool {
        self.outputs.is_empty()
    }

    /// add a output node to the node
    pub fn add_output(&mut self, output: Node) {
        self.outputs.push(output);
        self.refresh_output_names();
    }

    /// remove the output nodes of the node
    pub fn unset_outputs(&mut self) -> Vec<Node> {
        self.output_names = RVec::new();
        self.outputs.drain(..).collect()
    }

    pub fn remove_output(&mut self, output: &str) {
        self.outputs = self
            .outputs
            .drain(..)
            // should make sure no output nodes are locked at this time
            .filter(|n| {
                n.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .name()
                    != output
            })
            .collect::<Vec<_>>()
            .into();
        self.refresh_output_names();
    }

    /// order the output nodes in the network
    pub fn order_outputs(&mut self) {
        self.outputs.sort_by(|a, b| {
            b.try_lock()
                .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                .order
                .partial_cmp(
                    &a.try_lock()
                        .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                        .order,
                )
                .unwrap()
        });
        self.refresh_output_names();
    }

    /// single edge node of the node, None if no edges or multiple edges
    pub fn edge(&self) -> Option<Node> {
        match self.edges().as_slice() {
            [n] => Some(n.clone()),
            _ => None,
        }
    }

    /// Edges of the node
    pub fn edges(&self) -> Vec<Node> {
        self.inputs()
            .iter()
            .chain(self.outputs())
            .cloned()
            .collect()
    }

    /// Move the node to the side (move the inputs to its output)
    pub fn move_aside(&mut self) -> Result<(), &'static str> {
        match self.outputs.as_slice() {
            [] => self.inputs().iter().for_each(|i| {
                i.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .unset_outputs();
            }),
            [o] => self.inputs().iter().for_each(|i| {
                o.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_input(i.clone());
                i.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .unset_outputs();
                i.try_lock()
                    .expect(&format!("mutex error: {:?} {}", file!(), line!()))
                    .add_output(o.clone());
            }),
            // if multiple outputs how do we move inputs and outputs?
            // => Need to add the outputs to each inputs, and also remove
            // current node as their output. then remove the inputs
            // from the current node.
            _outs => {
                return Err("move aside with multiple output nodes is not implemented");
            }
        }
        self.unset_inputs();
        Ok(())
    }

    // FIX: this might have to be removed, or added with EvalError for
    // when there are multiple output to the node
    // /// Move the network down one step, (swap places with its output)
    // pub fn move_down(&mut self) {
    //     let outs = self.unset_outputs();
    //     match outs.as_slice() {
    //         // no outputs means no moving down
    //         [] => (),
    //         [o] => {
    //             let i = o
    //                 .try_lock().expect("mutex error")
    //                 .inputs()
    //                 .iter()
    //                 // HACK current node will fail to lock
    //                 .position(|c| c.try_lock().is_none())
    //                 .unwrap();
    //             let new_out = o.try_lock().expect("mutex error").inputs.remove(i);
    //             self.output = o.try_lock().expect("mutex error").output().clone();
    //             o.try_lock().expect("mutex error").set_output(new_out);
    //             self.add_input(o.clone());
    //         }
    //         // if multiple outputs how do we move it down?
    //         outs => todo!(),
    //     }
    // }
}
