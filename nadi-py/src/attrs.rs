use nadi_core::abi_stable::std_types::{RString, Tuple2};
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime, PyTime};
use std::collections::HashMap;
use std::str::FromStr;

#[pyclass(module = "nadi", name = "Date")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
pub struct PyNDate(Date);

#[pymethods]
impl PyNDate {
    // #[new]
    // #[pyo3(signature = (year, month = 1, day = 1))]
    // fn new(year: u16, month: u8, day: u8) -> Self {
    //     PyNDate(Date::new(year, month, day))
    // }

    #[new]
    fn parse(date: &str) -> PyResult<Self> {
        Ok(Date::from_str(&date)
            .map(PyNDate)
            .map_err(anyhow::Error::msg)?)
    }

    #[getter]
    fn year(&self) -> u16 {
        self.0.year
    }
    #[getter]
    fn month(&self) -> u8 {
        self.0.month
    }
    #[getter]
    fn day(&self) -> u8 {
        self.0.day
    }

    fn __repr__(&self) -> String {
        let d = &self.0;
        format!("<Date {}>", d)
    }
}

#[pyclass(module = "nadi", name = "Time")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
pub struct PyNTime(Time);

#[pymethods]
impl PyNTime {
    // #[new]
    // #[pyo3(signature = (hour, minute = 0, second = 0))]
    // fn new(hour: u8, minute: u8, second: u8) -> Self {
    //     // TODO add nanoseconds support later
    //     PyNTime(Time::new(hour, minute, second, 0))
    // }

    #[new]
    fn parse(time: &str) -> PyResult<Self> {
        Ok(Time::from_str(&time)
            .map(PyNTime)
            .map_err(anyhow::Error::msg)?)
    }

    #[getter]
    fn hour(&self) -> u8 {
        self.0.hour
    }
    #[getter]
    fn minute(&self) -> u8 {
        self.0.min
    }
    #[getter]
    fn second(&self) -> u8 {
        self.0.sec
    }

    fn __repr__(&self) -> String {
        let t = &self.0;
        format!("<Time {}>", t)
    }
}

#[pyclass(module = "nadi", name = "DateTime")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
pub struct PyNDateTime(DateTime);

#[pymethods]
impl PyNDateTime {
    // #[new]
    // #[pyo3(signature = (year, month = 1, day = 1, hour = 0, minute = 0, second = 0))]
    // fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
    //     let d = Date::new(year, month, day);
    //     let t = Time::new(hour, minute, second, 0);
    //     PyNDateTime(DateTime::new(d, t, None))
    // }

    #[new]
    fn parse(dt: &str) -> PyResult<Self> {
        Ok(DateTime::from_str(&dt)
            .map(PyNDateTime)
            .map_err(anyhow::Error::msg)?)
    }

    #[getter]
    fn year(&self) -> u16 {
        self.0.date.year
    }
    #[getter]
    fn month(&self) -> u8 {
        self.0.date.month
    }
    #[getter]
    fn day(&self) -> u8 {
        self.0.date.day
    }

    #[getter]
    fn hour(&self) -> u8 {
        self.0.time.hour
    }
    #[getter]
    fn minute(&self) -> u8 {
        self.0.time.min
    }
    #[getter]
    fn second(&self) -> u8 {
        self.0.time.sec
    }
    fn __repr__(&self) -> String {
        let dt = &self.0;
        format!("<DateTime {} {}>", dt.date, dt.time)
    }
}

#[derive(Clone, Debug, PartialEq, FromPyObject)]
pub enum PyAttribute {
    String(String),
    Bool(bool),
    Integer(i64), // int should be before float for pyo3
    Float(f64),
    Date(PyNDate),
    Time(PyNTime),
    DateTime(PyNDateTime),
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
            Self::Date(v) => v.into_py(py),
            Self::Time(v) => v.into_py(py),
            Self::DateTime(v) => v.into_py(py),
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
            PyAttribute::Date(v) => Self::Date(v.0),
            PyAttribute::Time(v) => Self::Time(v.0),
            PyAttribute::DateTime(v) => Self::DateTime(v.0),
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
            Attribute::Date(v) => Self::Date(PyNDate(v)),
            Attribute::Time(v) => Self::Time(PyNTime(v)),
            Attribute::DateTime(v) => Self::DateTime(PyNDateTime(v)),
            Attribute::Array(v) => Self::Array(v.into_iter().map(PyAttribute::from).collect()),
            Attribute::Table(t) => Self::Table(
                t.into_iter()
                    .map(|Tuple2(k, v)| (k.to_string(), PyAttribute::from(v)))
                    .collect(),
            ),
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
            Self::Date(v) => format!("{:?}", v.0),
            Self::Time(v) => format!("{:?}", v.0),
            Self::DateTime(v) => format!("{:?}", v.0),
            Self::Array(v) => format!("{v:?}"),
            Self::Table(v) => format!("{v:?}"),
        }
    }
}
