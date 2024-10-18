use crate::{attrs::AttrValue, network::PyNetwork, node::PyNode};
use nadi_core::abi_stable::std_types::{RResult, RString, RVec};
use nadi_core::functions::FunctionCtx;
use nadi_core::functions::{NadiFunctions, NetworkFunctionBox, NodeFunctionBox};
use nadi_core::prelude::*;
use std::collections::HashMap;

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

    #[pyo3(signature = (function, nodes, *args, **kwargs))]
    fn nodes(
        &self,
        function: &str,
        nodes: Vec<PyNode>,
        args: Vec<AttrValue>,
        kwargs: Option<HashMap<String, AttrValue>>,
    ) -> PyResult<()> {
        let args: Vec<Attribute> = args.into_iter().map(|v| v.into()).collect();
        let kwargs = kwargs
            .map(|kw| kw.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
            .unwrap_or_default();
        let ctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        let nodes: RVec<Node> = nodes.into_iter().map(|n| n.0).collect();
        self.0.call_node(function, nodes.as_rslice(), &ctx)?;
        Ok(())
    }

    #[pyo3(signature = (function, network, *args, **kwargs))]
    fn network(
        &self,
        function: &str,
        mut network: PyNetwork,
        args: Vec<AttrValue>,
        kwargs: Option<HashMap<String, AttrValue>>,
    ) -> PyResult<()> {
        let args: Vec<Attribute> = args.into_iter().map(|v| v.into()).collect();
        let kwargs = kwargs
            .map(|kw| kw.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
            .unwrap_or_default();
        let ctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        self.0.call_network(function, &mut network.0, &ctx)?;
        Ok(())
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
