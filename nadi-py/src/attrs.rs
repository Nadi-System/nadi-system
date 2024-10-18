use nadi_core::attrs::{Attribute, FromAttribute};
use pyo3::prelude::*;

#[derive(FromPyObject)]
pub enum AttrValue {
    String(String),
    Bool(bool),
    Float(f64),
    Integer(i64),
}

impl From<AttrValue> for Attribute {
    fn from(value: AttrValue) -> Self {
        match value {
            AttrValue::String(s) => Self::String(s.into()),
            AttrValue::Bool(b) => Self::Bool(b),
            AttrValue::Float(f) => Self::Float(f),
            AttrValue::Integer(i) => Self::Integer(i),
        }
    }
}

impl From<Attribute> for AttrValue {
    fn from(value: Attribute) -> Self {
        match value {
            Attribute::String(s) => Self::String(s.into()),
            Attribute::Bool(b) => Self::Bool(b),
            Attribute::Float(f) => Self::Float(f),
            Attribute::Integer(i) => Self::Integer(i),
            _ => panic!("Not implemented"),
        }
    }
}

impl IntoPy<PyObject> for AttrValue {
    fn into_py(self, py: Python<'_>) -> PyObject {
        match self {
            Self::String(s) => s.into_py(py),
            Self::Bool(b) => b.into_py(py),
            Self::Float(f) => f.into_py(py),
            Self::Integer(i) => i.into_py(py),
        }
    }
}

#[pyclass(name = "Attribute")]
#[repr(transparent)]
#[derive(Clone)]
pub struct PyAttribute(pub Attribute);

#[pymethods]
impl PyAttribute {
    #[new]
    fn new(value: AttrValue) -> PyResult<Self> {
        Ok(Self(Attribute::from(value)))
    }

    fn __repr__(&self) -> PyResult<String> {
        Ok(format!(
            "<Attribute {}: {}>",
            self.0.type_name(),
            self.0.to_string()
        ))
    }
}
