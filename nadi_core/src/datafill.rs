use crate::attrs::{Attribute, Date, DateTime, FromAttribute, Time};
use crate::timeseries::{CompleteSeries, MaskedSeries, Series};
use abi_stable::std_types::{ROption, RString, RVec};

#[derive(Debug)]
pub enum DataImputeError {
    IncompatibleMethod(String),
    AttributeError(String),
}

impl std::fmt::Display for DataImputeError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::IncompatibleMethod(s) => write!(f, "IncompatibleMethod: {s}"),
            Self::AttributeError(s) => write!(f, "AttributeError: {s}"),
        }
    }
}

impl std::error::Error for DataImputeError {}

#[derive(FromAttribute)]
pub enum DataImputeMethod {
    Series(SeriesMethod),
    Network(NetworkMethod),
}

pub enum SeriesMethod {
    Forward(Option<usize>),
    Backward(Option<usize>),
    Zero,
    One,
    // TODO these later
    Minimum,
    Maximum,
    Mean,
    Median,
    Linear,
    Nearest,
}

pub enum NetworkMethod {
    InputRatio(String),
    OutputRatio(String),
}

fn fill_gaps<T: Clone>(vals: RVec<ROption<T>>, fill: T) -> RVec<T> {
    vals.into_iter()
        .map(|v| v.unwrap_or(fill.clone()))
        .collect()
}

impl MaskedSeries {
    pub fn fill_gaps(self, value: Attribute) -> Result<CompleteSeries, DataImputeError> {
        Ok(match (self, value) {
            (Self::Floats(v), Attribute::Float(f)) => CompleteSeries::Floats(fill_gaps(v, f)),
            (Self::Floats(v), Attribute::Integer(i)) => {
                CompleteSeries::Floats(fill_gaps(v, i as f64))
            }
            (Self::Integers(v), Attribute::Integer(f)) => CompleteSeries::Integers(fill_gaps(v, f)),
            (Self::Strings(v), Attribute::String(f)) => CompleteSeries::Strings(fill_gaps(v, f)),
            (Self::Booleans(v), Attribute::Bool(f)) => CompleteSeries::Booleans(fill_gaps(v, f)),
            (Self::Dates(v), Attribute::Date(f)) => CompleteSeries::Dates(fill_gaps(v, f)),
            (Self::Times(v), Attribute::Time(f)) => CompleteSeries::Times(fill_gaps(v, f)),
            (Self::DateTimes(v), Attribute::DateTime(f)) => {
                CompleteSeries::DateTimes(fill_gaps(v, f))
            }
            (Self::DateTimes(v), Attribute::Date(f)) => {
                let f: DateTime = f.into();
                CompleteSeries::DateTimes(fill_gaps(v, f))
            }
            (Self::Attributes(v), f) => CompleteSeries::Attributes(fill_gaps(v, f)),
            (a, b) => {
                return Err(DataImputeError::AttributeError(format!(
                    "{} Series can not be filled with {} value",
                    a.type_name(),
                    b.type_name()
                )));
            }
        })
    }

    pub fn fill_zero(self) -> CompleteSeries {
        match self {
            Self::Floats(v) => CompleteSeries::Floats(fill_gaps(v, 0.0)),
            Self::Integers(v) => CompleteSeries::Integers(fill_gaps(v, 0)),
            Self::Strings(v) => CompleteSeries::Strings(fill_gaps(v, RString::new())),
            Self::Booleans(v) => CompleteSeries::Booleans(fill_gaps(v, false)),
            Self::Dates(v) => CompleteSeries::Dates(fill_gaps(v, Date::default())),
            Self::Times(v) => CompleteSeries::Times(fill_gaps(v, Time::default())),
            Self::DateTimes(v) => CompleteSeries::DateTimes(fill_gaps(v, DateTime::default())),
            Self::Attributes(v) => CompleteSeries::Attributes(fill_gaps(v, Attribute::default())),
        }
    }

    pub fn fill_one(self) -> Result<CompleteSeries, DataImputeError> {
        match self {
            Self::Floats(v) => Ok(CompleteSeries::Floats(fill_gaps(v, 1.0))),
            Self::Integers(v) => Ok(CompleteSeries::Integers(fill_gaps(v, 1))),
            Self::Booleans(v) => Ok(CompleteSeries::Booleans(fill_gaps(v, true))),
            x => Err(DataImputeError::IncompatibleMethod(format!(
                "Series of {} can not be filled with value of one",
                x.type_name()
            ))),
        }
    }

    pub fn fill_forward(self, _limit: Option<usize>) -> Result<Series, DataImputeError> {
        todo!()
    }

    pub fn fill_backward(self, _limit: Option<usize>) -> Result<Series, DataImputeError> {
        todo!()
    }

    pub fn impute_gaps(self, method: SeriesMethod) -> Result<Series, DataImputeError> {
        match method {
            SeriesMethod::Zero => Ok(self.fill_zero()).map(Series::Complete),
            SeriesMethod::One => self.fill_one().map(Series::Complete),
            SeriesMethod::Forward(v) => self.fill_forward(v),
            SeriesMethod::Backward(v) => self.fill_backward(v),
            _ => todo!(),
        }
    }
}

impl FromAttribute for SeriesMethod {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }
    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        let strval = String::try_from_attr(value)?;
        let (name, data) = strval.split_once(':').unwrap_or((&strval, ""));
        let no_data = |m: SeriesMethod| -> Result<Self, String> {
            if data.is_empty() {
                Ok(m)
            } else {
                Err(format!("Unused part {:?} for Data fill method", data))
            }
        };
        match name {
            "backward" => Ok(if data.is_empty() {
                Self::Backward(None)
            } else {
                let limit = data.parse::<usize>().map_err(|e| e.to_string())?;
                Self::Backward(Some(limit))
            }),
            "forward" => Ok(if data.is_empty() {
                Self::Forward(None)
            } else {
                let limit = data.parse::<usize>().map_err(|e| e.to_string())?;
                Self::Forward(Some(limit))
            }),
            "zero" => no_data(Self::Zero),
            "one" => no_data(Self::One),
            "min" | "minimum" => no_data(Self::Minimum),
            "max" | "maximum" => no_data(Self::Maximum),
            "mean" => no_data(Self::Mean),
            "median" => no_data(Self::Median),
            "linear" => no_data(Self::Linear),
            "nearest" => no_data(Self::Nearest),
            x => Err(format!("Data fill method {x:?} not recognized")),
        }
    }
}

impl FromAttribute for NetworkMethod {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }
    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        let strval = String::try_from_attr(value)?;
        let (name, data) = strval.split_once(':').unwrap_or((&strval, ""));
        match name {
            "input_ratio" | "iratio" => {
                if data.is_empty() {
                    Err(format!("Data fill method {:?} requires variables", data))
                } else {
                    Ok(Self::InputRatio(data.to_string()))
                }
            }
            "output_ratio" | "oratio" => {
                if data.is_empty() {
                    Err(format!("Data fill method {:?} requires variables", data))
                } else {
                    Ok(Self::OutputRatio(data.to_string()))
                }
            }
            x => Err(format!("Data fill method {x:?} not recognized")),
        }
    }
}
