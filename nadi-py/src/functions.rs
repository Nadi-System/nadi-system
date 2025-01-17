use crate::{
    attrs::{PyAttrMap, PyAttribute},
    network::PyNetwork,
    node::PyNode,
};
use nadi_core::abi_stable::std_types::RString;
use nadi_core::functions::{FunctionCtx, FunctionRet};
use nadi_core::functions::{NadiFunctions, NetworkFunctionBox, NodeFunctionBox};
use nadi_core::prelude::*;

use pyo3::{
    exceptions::{PyKeyError, PyRuntimeError},
    prelude::*,
};

#[pyclass(unsendable, module = "nadi", name = "NodeFunction")]
pub struct PyNodeFunction {
    pub func: NodeFunctionBox,
    pub sig: RString,
    pub pysig: RString,
}

impl PyNodeFunction {
    fn new(func: NodeFunctionBox) -> Self {
        let sig = func.signature();
        let pysig = sig_to_py(sig.as_str(), "node", true).into();
        let sig = sig_to_py(sig.as_str(), "node", false).into();
        Self { func, sig, pysig }
    }
}

#[pymethods]
impl PyNodeFunction {
    #[pyo3(signature = (node, *args, **kwargs))]
    fn __call__(
        &self,
        node: PyNode,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        match self.func.call(&mut node.0.lock(), &ctx) {
            FunctionRet::None => Ok(None),
            FunctionRet::Some(v) => Ok(Some(v.into())),
            FunctionRet::Error(s) => Err(PyRuntimeError::new_err(s.to_string())),
        }
    }

    #[getter]
    fn __name__(&self) -> String {
        self.func.name().to_string()
    }

    #[getter]
    fn __doc__(&self) -> String {
        self.func.help().to_string()
    }

    #[getter]
    fn __code__(&self) -> String {
        self.func.code().to_string()
    }

    #[getter]
    fn __signature__(&self) -> &str {
        self.pysig.as_str()
    }

    #[getter]
    fn __text_signature__(&self) -> &str {
        self.sig.as_str()
    }
}

#[pyclass(unsendable, module = "nadi", name = "NetworkFunction")]
pub struct PyNetworkFunction {
    pub func: NetworkFunctionBox,
    pub sig: RString,
    pub pysig: RString,
}

impl PyNetworkFunction {
    fn new(func: NetworkFunctionBox) -> Self {
        let sig = func.signature();
        let pysig = sig_to_py(sig.as_str(), "network", true).into();
        let sig = sig_to_py(sig.as_str(), "network", false).into();
        Self { func, sig, pysig }
    }
}
#[pymethods]
impl PyNetworkFunction {
    #[pyo3(signature = (network, *args, **kwargs))]
    fn __call__(
        &self,
        mut network: PyNetwork,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        match self.func.call(&mut network.0, &ctx) {
            FunctionRet::None => Ok(None),
            FunctionRet::Some(v) => Ok(Some(v.into())),
            FunctionRet::Error(s) => Err(PyRuntimeError::new_err(s.to_string())),
        }
    }

    #[getter]
    fn __name__(&self) -> String {
        self.func.name().to_string()
    }

    #[getter]
    fn __doc__(&self) -> String {
        self.func.help().to_string()
    }

    #[getter]
    fn __code__(&self) -> String {
        self.func.code().to_string()
    }

    #[getter]
    fn __signature__(&self) -> &str {
        self.pysig.as_str()
    }

    #[getter]
    fn __text_signature__(&self) -> &str {
        self.sig.as_str()
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

    #[pyo3(signature = (function, node, *args, **kwargs))]
    fn node(
        &self,
        function: &str,
        node: PyNode,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        let func = match self.0.node(function) {
            Some(f) => f,
            None => {
                return Err(PyKeyError::new_err(format!(
                    "Node Function {function} not found"
                )))
            }
        };
        match func.call(&mut node.0.lock(), &ctx) {
            FunctionRet::None => Ok(None),
            FunctionRet::Some(v) => Ok(Some(v.into())),
            FunctionRet::Error(s) => Err(PyRuntimeError::new_err(s.to_string())),
        }
    }

    #[pyo3(signature = (function, network, *args, **kwargs))]
    fn network(
        &self,
        function: &str,
        mut network: PyNetwork,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        let func = match self.0.network(function) {
            Some(f) => f,
            None => {
                return Err(PyKeyError::new_err(format!(
                    "Network Function {function} not found"
                )))
            }
        };
        match func.call(&mut network.0, &ctx) {
            FunctionRet::None => Ok(None),
            FunctionRet::Some(v) => Ok(Some(v.into())),
            FunctionRet::Error(s) => Err(PyRuntimeError::new_err(s.to_string())),
        }
    }

    // todo register python functions into nadi/node function

    fn node_function(&self, name: &str) -> PyResult<PyNodeFunction> {
        match self.0.node(name) {
            Some(f) => Ok(PyNodeFunction::new(f.clone())),
            None => Err(PyKeyError::new_err(format!(
                "Node Function {name} not found"
            ))),
        }
    }

    fn network_function(&self, name: &str) -> PyResult<PyNetworkFunction> {
        match self.0.network(name) {
            Some(f) => Ok(PyNetworkFunction::new(f.clone())),
            None => Err(PyKeyError::new_err(format!(
                "Network Function {name} not found"
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

fn sig_to_py(sig: &str, arg0: &str, notype: bool) -> String {
    let sig = sig.replace(" ", "");
    if sig == "()" {
        return format!("({arg0})");
    }
    let args: Vec<String> = sig
        .trim_start_matches('(')
        .trim_end_matches(')')
        .split(",")
        .map(|a| {
            let (key, ty, val) = match a.split_once(":") {
                Some((key, tyval)) => match tyval.split_once("=") {
                    Some((ty, val)) => (key, Some(ty), Some(val)),
                    None => (key, Some(tyval), None),
                },
                None => match a.split_once("=") {
                    Some((key, val)) => (key, None, Some(val)),
                    None => (a, None, None),
                },
            };
            let mut arg = key.to_string();
            if let Some(ty) = ty {
                let ty = ty.trim_matches('\'').trim_start_matches('&');
                if !notype {
                    arg.push_str(": ");
                    arg.push_str(type_to_py(ty));
                }
                if ty.starts_with("Option") {
                    arg.push_str(" = None");
                }
            }
            if let Some(val) = val {
                arg.push_str(" = ");
                arg.push_str(val_to_py(val));
            }
            arg
        })
        .collect();
    format!("({arg0}, {})", args.join(", "))
}

// this is not used now as type annotation is not supported in __signature__
#[allow(dead_code)]
fn type_to_py(ty: &str) -> &'static str {
    match ty {
        "i64" => "int",
        "f64" => "float",
        "String" | "Template" | "str" => "str",
        "bool" => "bool",
        _ => "Any",
    }
}

fn val_to_py(val: &str) -> &str {
    match val {
        "true" => "True",
        "false" => "False",
        v => {
            if v.parse::<f64>().is_ok() {
                v
            } else if v.starts_with('"') && v.ends_with('"') {
                v
            } else {
                "..."
            }
        }
    }
}

fn py_args_kwargs_to_ctx(args: Vec<PyAttribute>, kwargs: Option<PyAttrMap>) -> FunctionCtx {
    let args: Vec<Attribute> = args.into_iter().map(|v| v.into()).collect();
    let kwargs = kwargs
        .map(|kw| kw.into_iter().map(|(k, v)| (k.into(), v.into())).collect())
        .unwrap_or_default();
    FunctionCtx::from_arg_kwarg(args, kwargs)
}
