use crate::{
    attrs::{PyAttrMap, PyAttribute},
    network::PyNetwork,
    node::PyNode,
};
use nadi_core::abi_stable::std_types::RString;
use nadi_core::functions::{
    EnvFunctionBox, FuncArg, FuncArgType, NadiFunctions, NetworkFunctionBox, NodeFunctionBox,
};
use nadi_core::functions::{FunctionCtx, FunctionRet};
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
    pub fn new(func: NodeFunctionBox) -> Self {
        let args = func.args().to_vec();
        let pysig = sig_to_py(&args, Some("node"), true).into();
        let sig = sig_to_py(&args, Some("node"), false).into();
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
        match self.func.call_mut(&mut node.0.lock(), &ctx) {
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
    pub fn new(func: NetworkFunctionBox) -> Self {
        let args = func.args().to_vec();
        let pysig = sig_to_py(&args, Some("network"), true).into();
        let sig = sig_to_py(&args, Some("network"), false).into();
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
        match self.func.call_mut(&mut network.0, &ctx) {
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

#[pyclass(unsendable, module = "nadi", name = "EnvFunction")]
pub struct PyEnvFunction {
    pub func: EnvFunctionBox,
    pub sig: RString,
    pub pysig: RString,
}

impl PyEnvFunction {
    pub fn new(func: EnvFunctionBox) -> Self {
        let args = func.args().to_vec();
        let pysig = sig_to_py(&args, None, true).into();
        let sig = sig_to_py(&args, None, false).into();
        Self { func, sig, pysig }
    }
}
#[pymethods]
impl PyEnvFunction {
    #[pyo3(signature = (*args, **kwargs))]
    fn __call__(
        &self,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        match self.func.call(&ctx) {
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
        match func.call_mut(&mut node.0.lock(), &ctx) {
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
        match func.call_mut(&mut network.0, &ctx) {
            FunctionRet::None => Ok(None),
            FunctionRet::Some(v) => Ok(Some(v.into())),
            FunctionRet::Error(s) => Err(PyRuntimeError::new_err(s.to_string())),
        }
    }

    #[pyo3(signature = (function, *args, **kwargs))]
    fn env(
        &self,
        function: &str,
        args: Vec<PyAttribute>,
        kwargs: Option<PyAttrMap>,
    ) -> PyResult<Option<PyAttribute>> {
        let ctx = py_args_kwargs_to_ctx(args, kwargs);
        let func = match self.0.env(function) {
            Some(f) => f,
            None => {
                return Err(PyKeyError::new_err(format!(
                    "Env Function {function} not found"
                )))
            }
        };
        match func.call(&ctx) {
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

    fn env_function(&self, name: &str) -> PyResult<PyEnvFunction> {
        match self.0.env(name) {
            Some(f) => Ok(PyEnvFunction::new(f.clone())),
            None => Err(PyKeyError::new_err(format!(
                "Env Function {name} not found"
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

fn sig_to_py(sig: &[FuncArg], arg0: Option<&str>, notype: bool) -> String {
    let mut args = Vec::new();
    if let Some(a) = arg0 {
        let ty = if notype {
            String::new()
        } else {
            let mut c = a.chars();
            let ty = match c.next() {
                None => String::new(),
                Some(first_char) => String::from_iter(first_char.to_uppercase().chain(c)),
            };
            format!(" : {}", ty)
        };
        args.push(format!("{}{}", a, ty));
    }
    for a in sig {
        args.push(if notype {
            match &a.category {
                FuncArgType::Arg => format!("{}", a.name),
                FuncArgType::OptArg => format!("{}", a.name),
                FuncArgType::DefArg(val) => format!("{} = {}", a.name, val),
                FuncArgType::Args => format!("*{}", a.name),
                FuncArgType::KwArgs => format!("**{}", a.name),
            }
        } else {
            match &a.category {
                FuncArgType::Arg => format!("{}: {}", a.name, type_to_py(a.ty.as_str())),
                FuncArgType::OptArg => format!("{}: {}", a.name, type_to_py(a.ty.as_str())),
                FuncArgType::DefArg(val) => {
                    format!("{}: {} = {}", a.name, type_to_py(a.ty.as_str()), val)
                }
                FuncArgType::Args => format!("*{}", a.name),
                FuncArgType::KwArgs => format!("**{}", a.name),
            }
        });
    }
    format!("({})", args.join(", "))
}

fn type_to_py(ty: &str) -> String {
    ty.split(' ')
        .map(|p| match p {
            "i64" => "int",
            "f64" => "float",
            "String" | "Template" | "str" => "str",
            "bool" => "bool",
            "Date" => "NDate",
            "Time" => "NTime",
            "DateTime" => "NDateTime",
            "Array" | "Vec" => "List",
            "Table" | "HashMap" => "Dict",
            "Attribute" => "Any",
            "<" => "[",
            ">" => "]",
            _ => "...",
        })
        .collect::<Vec<&str>>()
        .join("")
}

fn py_args_kwargs_to_ctx(args: Vec<PyAttribute>, kwargs: Option<PyAttrMap>) -> FunctionCtx {
    let args: Vec<Attribute> = args.into_iter().map(|v| v.into()).collect();
    let kwargs = kwargs
        .map(|kw| kw.into_iter().map(|(k, v)| (k, v.into())).collect())
        .unwrap_or_default();
    FunctionCtx::from_arg_kwarg(args, kwargs)
}
