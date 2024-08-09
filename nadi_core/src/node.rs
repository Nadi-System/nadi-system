use anyhow::Context;
use serde_derive::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::Path;
use std::rc::Rc;
use string_template_plus::{Render, RenderOptions, Template};
use toml::{Table, Value};

use crate::timeseries::{LoadedTimeSeries, TimeSeriesDefinition};

/// Reference counted node data to use in [`crate::Network`]
pub type Node = Rc<RefCell<NodeInner>>;

/// Represents points with attributes and timeseries. These can be any
/// point as long as they'll be on the network and connection to each
/// other.
///
/// The attributes can be any format. There is a special type of
/// attribute timeseries to deal with timeseries data that has been
/// provided by the system. But users are free to make their own
/// attributes and plugins + functions that can work with those
/// attributes.
///
/// Since attributes are loaded using TOML file, simple attributes can
/// be stored and parse from strings, and complex ones can be saved in
/// different files and their path can be stored as node attributes.
///
/// Here is an example node attribute file. Here we have string,
/// float, int and boolean values, as well as a timeseries definition.
///
///     [attrs]
///     stn="smithland"
///     nat_7q10=12335.94850131619
///     orsanco_7q10=16900
///     lock=true
///     ...
///    
///     [timeseries]
///     natural={path="../test.csv", dtype="float", ...}
#[derive(Clone)]
pub struct NodeInner {
    /// index of the current node in the [`crate::Network`]
    index: usize,
    /// name of the node
    name: String,
    /// level represents the rank of the tributary, 0 for main branch
    /// and 1 for tributaries connected to main branch and so on
    level: usize,
    /// Number of inputs connected to the current node
    order: usize,
    /// List of immediate inputs
    inputs: Vec<Node>,
    /// Output of the node if present
    output: Option<Node>,
    /// Node attributes in a  Hashmap of [`String`] to [`toml::Value`]
    attrs: Table,
    /// Lazy timeseries containing [`TimeSeriesDefinition`]
    lazy_timeseries: HashMap<String, TimeSeriesDefinition>,
    /// Hashmap of [`LoadedTimeSeries`]
    timeseries: HashMap<String, LoadedTimeSeries>,
}

#[derive(Deserialize, Serialize, Debug, Clone, Default)]
pub struct NodeAttrs {
    attrs: Table,
    timeseries: Option<HashMap<String, TimeSeriesDefinition>>,
}

impl NodeAttrs {
    pub fn extend(&mut self, attrs: Table) {
        self.attrs.extend(attrs);
    }
    pub fn attr(&self, key: &str) -> Option<Value> {
        self.attrs.get(key).cloned()
    }
    pub fn has_attr(&self, key: &str) -> bool {
        self.attrs.contains_key(key)
    }
    pub fn set_attr<T: ToString>(&mut self, key: T, value: Value) {
        self.attrs.insert(key.to_string(), value);
    }
}

/// Create a new [`Node`]
pub fn new_node(index: usize, name: String) -> Node {
    let mut ni = NodeInner {
        index,
        name: name.clone(),
        level: 0,
        order: 0,
        inputs: vec![],
        output: None,
        attrs: Table::new(),
        lazy_timeseries: HashMap::new(),
        timeseries: HashMap::new(),
    };
    ni.set_attr("INDEX", Value::Integer(index as i64));
    ni.set_attr("NAME", Value::String(name));
    ni.set_attr("ORDER", Value::Integer(0));
    ni.set_attr("LEVEL", Value::Integer(0));

    Rc::new(RefCell::new(ni))
}

impl NodeInner {
    pub fn index(&self) -> usize {
        self.index
    }
    pub fn set_index(&mut self, ind: usize) {
        self.set_attr("INDEX", Value::Integer(ind as i64));
        self.index = ind;
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn set_name(&mut self, name: &str) {
        self.set_attr("NAME", Value::String(name.to_string()));
        self.name = name.to_string();
    }
    pub fn order(&self) -> usize {
        self.order
    }

    /// Move the node down (swap place with output) TODO untested
    pub fn move_down(&mut self) {
        if self.output.is_none() {
            return;
        }

        let out = self.output().unwrap();
        let i = out
            .borrow()
            .inputs()
            .position(|c| c.borrow().name() == self.name())
            .unwrap();
        out.borrow_mut()
            .set_output(out.borrow().inputs().nth(i).unwrap());
        out.borrow_mut().inputs.remove(i);
        self.add_input(&out);
        self.output = out.borrow().output.clone();
    }

    /// Move the node to the side (move the inputs to its output) TODO untested
    pub fn move_aside(&mut self) {
        if let Some(o) = &self.output {
            self.inputs().for_each(|i| {
                o.borrow_mut().add_input(i);
                i.borrow_mut().set_output(o)
            });
        } else {
            self.inputs().for_each(|i| i.borrow_mut().unset_output());
        }
        self.overwrite_input(vec![]);
    }

    pub fn set_order(&mut self, ord: usize) {
        self.set_attr("ORDER", Value::Integer(ord as i64));
        self.order = ord;
    }
    pub fn level(&self) -> usize {
        self.level
    }
    pub fn set_level(&mut self, level: usize) {
        self.set_attr("LEVEL", Value::Integer(level as i64));
        self.level = level;
    }
    pub fn attr(&self, key: &str) -> Option<Value> {
        self.attrs.get(key).cloned()
    }
    pub fn has_attr(&self, key: &str) -> bool {
        self.attrs.contains_key(key)
    }
    pub fn set_attr<T: ToString>(&mut self, key: T, value: Value) {
        self.attrs.insert(key.to_string(), value);
    }
    pub fn extend_attrs(&mut self, table: Table) {
        self.attrs.extend(table);
    }
    pub fn load_attr(&mut self, file: &Path) -> anyhow::Result<()> {
        let contents = std::fs::read_to_string(file)?;
        let attrs: NodeAttrs = toml::from_str(&contents)?;
        self.attrs.extend(attrs.attrs);
        if let Some(mut ts) = attrs.timeseries {
            ts.values_mut()
                .for_each(|v| v.resolve_path(file.parent().unwrap()));
            self.lazy_timeseries.extend(ts);
        }
        Ok(())
    }

    pub fn set_timeseries(&mut self, name: &str, ts: LoadedTimeSeries) {
        self.timeseries.insert(name.to_string(), ts);
    }

    pub fn has_timeseries(&self, name: &str) -> bool {
        self.timeseries.contains_key(name) || self.lazy_timeseries.contains_key(name)
    }

    pub fn list_timeseries(&self) -> Vec<&str> {
        self.timeseries
            .keys()
            .chain(self.lazy_timeseries.keys())
            .map(|s| s.as_str())
            .collect()
    }

    pub fn timeseries(&mut self, name: &str) -> anyhow::Result<&LoadedTimeSeries> {
        match self.timeseries.entry(name.to_string()) {
            Entry::Occupied(ts) => Ok(ts.into_mut()),
            Entry::Vacant(vts) => {
                let tsdef = self
                    .lazy_timeseries
                    .get(name)
                    .context("Timeseries with given name doesn't exist")?;
                let ts = tsdef.load()?;
                self.lazy_timeseries.remove(name);
                Ok(vts.insert(ts))
            }
        }
    }

    pub fn input(&self, index: usize) -> Option<&Node> {
        self.inputs.get(index)
    }
    pub fn order_inputs(&mut self) {
        self.inputs
            .sort_by(|a, b| b.borrow().order.partial_cmp(&a.borrow().order).unwrap());
    }
    pub fn inputs(&self) -> impl Iterator<Item = &Node> {
        self.inputs.iter()
    }
    pub fn output(&self) -> Option<Node> {
        self.output.clone()
    }
    pub fn add_input(&mut self, inp: &Node) {
        self.inputs.push(inp.clone());
    }
    pub fn overwrite_input(&mut self, inputs: Vec<Node>) {
        self.inputs = inputs;
        self.order_inputs();
    }
    pub fn set_output(&mut self, output: &Node) {
        self.output = Some(output.clone());
    }
    pub fn unset_output(&mut self) {
        self.output = None;
    }

    pub fn render(&self, templ: &Template) -> anyhow::Result<String> {
        let mut op = RenderOptions::default();
        for var in templ.parts().iter().flat_map(|p| p.variables()) {
            if let Some(val) = self.attr(var) {
                op.variables.insert(var.to_string(), val.to_string());
            }
            if let Some(val) = var.strip_prefix('_') {
                if let Some(Value::String(s)) = self.attr(val) {
                    op.variables.insert(var.to_string(), s);
                }
            }
        }
        templ.render(&op)
    }

    pub fn print(&self) {
        println!("[{}] {}", self.index, self.name);
    }

    pub fn attributes(&self) -> Vec<&str> {
        self.attrs.keys().map(|k| k.as_str()).collect()
    }

    pub fn print_attrs(&self) {
        for (k, v) in &self.attrs {
            println!("{} = {}", k, v);
        }
    }
}
