use nadi_core::abi_stable::std_types::{RString, Tuple2};
use nadi_core::attrs::Attribute;
use pyo3::prelude::*;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, FromPyObject)]
pub enum PyAttribute {
    String(String),
    Bool(bool),
    Integer(i64), // int should be before float for pyo3
    Float(f64),
    Array(Vec<PyAttribute>),
    Table(PyAttrMap),
}

impl IntoPy<PyObject> for PyAttribute {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::String(s) => s.into_py(py),
            Self::Bool(b) => b.into_py(py),
            Self::Float(f) => f.into_py(py),
            Self::Integer(i) => i.into_py(py),
            Self::Array(v) => v.into_py(py),
            Self::Table(v) => v.into_py(py),
        }
    }
}

pub type PyAttrMap = HashMap<String, PyAttribute>;

impl From<PyAttribute> for Attribute {
    fn from(value: PyAttribute) -> Self {
        match value {
            PyAttribute::String(s) => Self::String(s.into()),
            PyAttribute::Bool(b) => Self::Bool(b),
            PyAttribute::Float(f) => Self::Float(f),
            PyAttribute::Integer(i) => Self::Integer(i),
            PyAttribute::Array(v) => Self::Array(v.into_iter().map(Attribute::from).collect()),
            PyAttribute::Table(m) => Self::Table(
                m.into_iter()
                    .map(|(k, v)| (RString::from(k), Attribute::from(v)))
                    .collect(),
            ),
        }
    }
}

impl From<Attribute> for PyAttribute {
    fn from(value: Attribute) -> Self {
        match value {
            Attribute::String(s) => Self::String(s.into()),
            Attribute::Bool(b) => Self::Bool(b),
            Attribute::Float(f) => Self::Float(f),
            Attribute::Integer(i) => Self::Integer(i),
            Attribute::Array(v) => Self::Array(v.into_iter().map(PyAttribute::from).collect()),
            Attribute::Table(t) => Self::Table(
                t.into_iter()
                    .map(|Tuple2(k, v)| (k.to_string(), PyAttribute::from(v)))
                    .collect(),
            ),
            _ => panic!("Not implemented"),
        }
    }
}

impl ToString for PyAttribute {
    fn to_string(&self) -> String {
        match self {
            Self::Bool(v) => format!("{v:?}"),
            Self::String(v) => format!("{v:?}"),
            Self::Integer(v) => format!("{v:?}"),
            Self::Float(v) => format!("{v:?}"),
            Self::Array(v) => format!("{v:?}"),
            Self::Table(v) => format!("{v:?}"),
        }
    }
}
