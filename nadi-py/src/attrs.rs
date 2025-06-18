use nadi_core::abi_stable::std_types::{RString, Tuple2};
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use pyo3::prelude::*;
use pyo3::types::{PyDate, PyDateTime, PyTime};
use std::collections::HashMap;
use std::str::FromStr;

#[pyclass(module = "nadi", name = "Date")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
/// Nadi Date data type, contains year, month and day
pub struct PyNDate(Date);

#[pymethods]
impl PyNDate {
    #[new]
    /// Parse date from a string
    fn parse(date: &str) -> PyResult<Self> {
        Ok(Date::from_str(date)
            .map(PyNDate)
            .map_err(anyhow::Error::msg)?)
    }

    #[staticmethod]
    /// Build a date object from year, month and day
    fn from_ymd(year: u16, month: u8, day: u8) -> Self {
        Self(Date { year, month, day })
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

    /// Convert to `datetime.date`
    fn to_date<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDate>> {
        PyDate::new(py, self.year().into(), self.month(), self.day())
    }

    fn __repr__(&self) -> String {
        let d = &self.0;
        format!("<Date {}>", d)
    }
}

#[pyclass(module = "nadi", name = "Time")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
/// Python time data type, contains hour, minutes and seconds
pub struct PyNTime(Time);

#[pymethods]
impl PyNTime {
    #[new]
    /// Parse time from a string
    fn parse(time: &str) -> PyResult<Self> {
        Ok(Time::from_str(time)
            .map(PyNTime)
            .map_err(anyhow::Error::msg)?)
    }

    #[staticmethod]
    /// Build time from hour, minute and second
    fn from_hms(hour: u8, minute: u8, second: u8) -> Self {
        Self(Time {
            hour,
            min: minute,
            sec: second,
            nanosecond: 0,
        })
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

    /// Convert to `datetime.time`
    fn to_time<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyTime>> {
        PyTime::new(py, self.hour(), self.minute(), self.second(), 0, None)
    }
}

#[pyclass(module = "nadi", name = "DateTime")]
#[repr(transparent)]
#[derive(Clone, PartialEq, Debug)]
/// Nadi DateTime object
pub struct PyNDateTime(DateTime);

#[pymethods]
impl PyNDateTime {
    #[new]
    /// Parse datetime from string
    fn parse(dt: &str) -> PyResult<Self> {
        Ok(DateTime::from_str(dt)
            .map(PyNDateTime)
            .map_err(anyhow::Error::msg)?)
    }

    #[staticmethod]
    /// build datetime from year, month, day, hour, minute, second
    fn from_ymdhms(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self(DateTime {
            date: Date { year, month, day },
            time: Time {
                hour,
                min: minute,
                sec: second,
                nanosecond: 0,
            },
            offset: None.into(),
        })
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

    /// Convert to `datetime.datetime`
    fn to_datetime<'a>(&self, py: Python<'a>) -> PyResult<Bound<'a, PyDateTime>> {
        PyDateTime::new(
            py,
            self.year().into(),
            self.month(),
            self.day(),
            self.hour(),
            self.minute(),
            self.second(),
            0,
            None,
        )
    }
}

#[derive(Clone, Debug, PartialEq, FromPyObject, IntoPyObject)]
/// Nadi Attribute
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

impl std::fmt::Display for PyAttribute {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{v:?}"),
            Self::String(v) => write!(f, "{v:?}"),
            Self::Integer(v) => write!(f, "{v:?}"),
            Self::Float(v) => write!(f, "{v:?}"),
            Self::Date(v) => write!(f, "{:?}", v),
            Self::Time(v) => write!(f, "{:?}", v),
            Self::DateTime(v) => write!(f, "{:?}", v),
            Self::Array(v) => write!(f, "{v:?}"),
            Self::Table(v) => write!(f, "{v:?}"),
        }
    }
}
