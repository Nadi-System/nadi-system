use crate::{
    attrs::{PyAttrMap, PyAttribute},
    network::PyNetwork,
    node::PyNode,
};
use nadi_core::abi_stable::std_types::{RResult, RString, RVec};
use nadi_core::functions::FunctionCtx;
use nadi_core::functions::{NadiFunctions, NetworkFunctionBox, NodeFunctionBox};
use nadi_core::prelude::*;
use std::collections::HashMap;

use pyo3::{
    exceptions::{PyAttributeError, PyKeyError, PyRuntimeError},
    prelude::*,
};

fn py_args_kwargs_to_ctx(args: Vec<PyAttribute>, kwargs: Option<PyAttrMap>) -> FunctionCtx {
    let args: Vec<Attribute> = args.into_iter().map(|v| v.into()).collect();
    let kwargs = kwargs
        .map(|kw| kw.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
        .unwrap_or_default();
    FunctionCtx::from_arg_kwarg(args, kwargs)
}

#[pyclass(unsendable, module = "nadi", name = "NodeFunction")]
pub struct PyNodeFunction(pub NodeFunctionBox);

#[pymethods]
impl PyNodeFunction {
    #[pyo3(signature = (nodes, *args, **kwargs))]
    fn __call__(
        &self,
        nodes: Vec<PyNode>,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<()> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        let nodes: RVec<Node> = nodes.into_iter().map(|n| n.0).collect();
        if let RResult::RErr(e) = self.0.call(nodes.as_rslice(), &ctx) {
            Err(PyRuntimeError::new_err(e.to_string()))
        } else {
            Ok(())
        }
    }

    #[getter]
    fn __name__(&self) -> String {
        self.0.name().to_string()
    }

    #[getter]
    fn __doc__(&self) -> String {
        self.0.help().to_string()
    }

    #[getter]
    fn __code__(&self) -> String {
        self.0.code().to_string()
    }

    #[getter]
    fn __text_signature__(&self) -> String {
        match self.0.help().split_once("# Signature:") {
            Some((_, sig)) => sig.trim(),
            None => "(*args, **kwargs)",
        }
        .to_string()
    }
}

#[pyclass(unsendable, module = "nadi", name = "NetworkFunction")]
pub struct PyNetworkFunction(pub NetworkFunctionBox);

#[pymethods]
impl PyNetworkFunction {
    #[pyo3(signature = (network, *args, **kwargs))]
    fn __call__(
        &self,
        mut network: PyNetwork,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<()> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        if let RResult::RErr(e) = self.0.call(&mut network.0, &ctx) {
            Err(PyRuntimeError::new_err(e.to_string()))
        } else {
            Ok(())
        }
    }

    #[getter]
    fn __name__(&self) -> String {
        self.0.name().to_string()
    }

    #[getter]
    fn __doc__(&self) -> String {
        self.0.help().to_string()
    }

    #[getter]
    fn __code__(&self) -> String {
        self.0.code().to_string()
    }

    #[getter]
    fn __text_signature__(&self) -> String {
        match self.0.help().split_once("# Signature:") {
            Some((_, sig)) => sig.trim(),
            None => "(*args, **kwargs)",
        }
        .to_string()
    }
}

// let's just make these into submodule of nadi; and put all functions
// into either nadi.functions.node.* or nadi.functions.network.*; then
// maybe add the execute thing to task instead. Optionally we could
// define functions to use as decorators that register new functions
// from python. Our Execute function on network would just take
// function from python look into the submodules and execute it.
// Maybe we need to store the rust nadi functions in the module somehow
#[pyclass(unsendable, module = "nadi", name = "NadiFunctions")]
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
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<()> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        let nodes: RVec<Node> = nodes.into_iter().map(|n| n.0).collect();
        self.0.call_node(function, nodes.as_rslice(), &ctx)?;
        Ok(())
    }

    #[pyo3(signature = (function, network, *args, **kwargs))]
    fn network(
        &self,
        function: &str,
        mut network: PyNetwork,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<()> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        self.0.call_network(function, &mut network.0, &ctx)?;
        Ok(())
    }

    // todo register python functions into nadi/node functions

    fn node_function(&self, name: &str) -> PyResult<PyNodeFunction> {
        match self.0.node_functions().get(name) {
            Some(f) => Ok(PyNodeFunction(f.clone())),
            None => Err(PyKeyError::new_err(format!(
                "Node Function {name} not found"
            ))),
        }
    }

    fn list_node_functions(&self) -> Vec<String> {
        self.0
            .node_functions()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    fn list_network_functions(&self) -> Vec<String> {
        self.0
            .network_functions()
            .keys()
            .map(|k| k.to_string())
            .collect()
    }

    #[pyo3(signature = (function, print=true))]
    fn help(&self, function: &str, print: bool) -> Option<String> {
        match self.0.help(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }
    #[pyo3(signature = (function, print=true))]
    fn help_node(&self, function: &str, print: bool) -> Option<String> {
        match self.0.help_node(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }
    #[pyo3(signature = (function, print=true))]
    fn help_network(&self, function: &str, print: bool) -> Option<String> {
        match self.0.help_network(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }

    #[pyo3(signature = (function, print=true))]
    fn code(&self, function: &str, print: bool) -> Option<String> {
        match self.0.code(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }
    #[pyo3(signature = (function, print=true))]
    fn code_node(&self, function: &str, print: bool) -> Option<String> {
        match self.0.code_node(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }
    #[pyo3(signature = (function, print=true))]
    fn code_network(&self, function: &str, print: bool) -> Option<String> {
        match self.0.code_network(function) {
            Some(h) if print => {
                println!("{h}");
                None
            }
            v => v,
        }
    }
}
