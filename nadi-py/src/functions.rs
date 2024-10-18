use crate::node::PyNode;
use nadi_core::functions::NadiFunctions;
use nadi_core::network::Network;

use pyo3::{
    exceptions::{PyAttributeError, PyKeyError},
    prelude::*,
};

#[pyclass(unsendable, name = "NadiFunctions")]
pub struct PyNadiFunctions(pub NadiFunctions);

#[pymethods]
impl PyNadiFunctions {
    #[new]
    fn new() -> Self {
        Self(NadiFunctions::new())
    }

    // todo register python functions into nadi/node functions

    fn node_functions(&self) -> Vec<String> {
        self.0
            .node_functions()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn network_functions(&self) -> Vec<String> {
        self.0
            .node_functions()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn help(&self, func: &str) -> Option<String> {
        self.0.help(func)
    }
    fn help_node(&self, func: &str) -> Option<String> {
        self.0.help_node(func)
    }
    fn help_network(&self, func: &str) -> Option<String> {
        self.0.help_network(func)
    }

    fn code(&self, func: &str) -> Option<String> {
        self.0.code(func)
    }
    fn code_node(&self, func: &str) -> Option<String> {
        self.0.code_node(func)
    }
    fn code_network(&self, func: &str) -> Option<String> {
        self.0.code_network(func)
    }
}
