use crate::attrs::{type_name, Attribute, Date, DateTime, Time};

use abi_stable::{
    external_types::RMutex,
    std_types::{RArc, RString, RVec},
    StableAbi,
};

pub type TimeLine = RArc<RMutex<TimeLineInner>>;

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub struct TimeLineInner {
    /// timestamp of the start datetime
    start: i64,
    /// timestamp of the end datetime
    end: i64,
    /// step in seconds
    step: i64,
    /// is regular timeseries or not
    _regular: bool,
    /// values in string format so that we don't have to deal with time
    str_values: RVec<RString>,
    /// format string used in the str_values,
    datetimefmt: RString,
}

impl<'a> TimeLineInner {
    pub fn start(&self) -> i64 {
        self.start
    }

    pub fn end(&self) -> i64 {
        self.end
    }

    pub fn step(&self) -> i64 {
        self.step
    }

    pub fn str_values(&'a self) -> impl Iterator<Item = &'a str> {
        self.str_values.iter().map(|s| s.as_str())
    }

    pub fn datetimefmt(&'a self) -> &'a str {
        self.datetimefmt.as_str()
    }
}

#[repr(C)]
#[derive(StableAbi, Clone)]
pub struct TimeSeries {
    timeline: TimeLine,
    values: TimeSeriesValues,
}

impl TimeSeries {
    pub fn new(timeline: TimeLine, values: TimeSeriesValues) -> Self {
        Self { timeline, values }
    }

    pub fn start(&self) -> i64 {
        self.timeline.lock().start()
    }

    pub fn step(&self) -> i64 {
        self.timeline.lock().step()
    }

    pub fn timeline(&self) -> &TimeLine {
        &self.timeline
    }

    pub fn values<'a, T: FromTimeSeries<'a>>(&'a self) -> Option<&'a [T]> {
        FromTimeSeries::from_ts(&self.values)
    }

    pub fn values_mut<'a, T: FromTimeSeries<'a>>(&'a mut self) -> Option<&'a mut [T]> {
        FromTimeSeries::from_ts_mut(&mut self.values)
    }

    pub fn try_values<'a, T: FromTimeSeries<'a>>(&'a self) -> Result<&'a [T], String> {
        FromTimeSeries::try_from_ts(&self.values)
    }
    pub fn try_values_mut<'a, T: FromTimeSeries<'a>>(&'a mut self) -> Result<&'a mut [T], String> {
        FromTimeSeries::try_from_ts_mut(&mut self.values)
    }

    pub fn values_type(&self) -> &str {
        self.values.type_name()
    }
}

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum TimeSeriesValues {
    Floats(RVec<f64>),
    Integers(RVec<i64>),
    Strings(RVec<RString>),
    Booleans(RVec<bool>),
    Dates(RVec<Date>),
    Times(RVec<Time>),
    DateTimes(RVec<DateTime>),
    Attributes(RVec<Attribute>),
}

impl TimeSeriesValues {
    pub fn len(&self) -> usize {
        match self {
            Self::Floats(v) => v.len(),
            Self::Integers(v) => v.len(),
            Self::Strings(v) => v.len(),
            Self::Booleans(v) => v.len(),
            Self::Dates(v) => v.len(),
            Self::Times(v) => v.len(),
            Self::DateTimes(v) => v.len(),
            Self::Attributes(v) => v.len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn type_name(&self) -> &str {
        match self {
            Self::Floats(_) => "Float",
            Self::Integers(_) => "Integer",
            Self::Strings(_) => "String",
            Self::Booleans(_) => "Boolean",
            Self::Dates(_) => "Date",
            Self::Times(_) => "Time",
            Self::DateTimes(_) => "DateTime",
            Self::Attributes(_v) => "Attributes",
        }
    }
}

pub trait FromTimeSeries<'a>: Sized {
    fn from_ts(value: &'a TimeSeriesValues) -> Option<&'a [Self]>;
    fn from_ts_mut(value: &'a mut TimeSeriesValues) -> Option<&'a mut [Self]>;
    fn try_from_ts(value: &'a TimeSeriesValues) -> Result<&'a [Self], String> {
        let ermsg = format!(
            "Incorrect Type: timeseries of `{}` cannot be converted to `{}`",
            value.type_name(),
            type_name::<Self>()
        );
        FromTimeSeries::from_ts(value).ok_or(ermsg)
    }
    fn try_from_ts_mut(value: &'a mut TimeSeriesValues) -> Result<&'a mut [Self], String> {
        let ermsg = format!(
            "Incorrect Type: timeseries of `{}` cannot be converted to `{}`",
            value.type_name(),
            type_name::<Self>()
        );
        FromTimeSeries::from_ts_mut(value).ok_or(ermsg)
    }
}

macro_rules! impl_from_ts {
    ($t: tt, $x: path) => {
        impl<'a> FromTimeSeries<'a> for $t {
            fn from_ts(value: &TimeSeriesValues) -> Option<&[$t]> {
                match value {
                    $x(v) => Some(v.as_slice()),
                    _ => None,
                }
            }
            fn from_ts_mut(value: &mut TimeSeriesValues) -> Option<&mut [$t]> {
                match value {
                    $x(v) => Some(v.as_mut_slice()),
                    _ => None,
                }
            }
        }

        impl From<&[$t]> for TimeSeriesValues {
            fn from(item: &[$t]) -> Self {
                $x(item.into())
            }
        }
        impl From<Vec<$t>> for TimeSeriesValues {
            fn from(item: Vec<$t>) -> Self {
                $x(RVec::from(item))
            }
        }
    };
}

impl_from_ts!(f64, TimeSeriesValues::Floats);
impl_from_ts!(i64, TimeSeriesValues::Integers);
impl_from_ts!(RString, TimeSeriesValues::Strings);
impl_from_ts!(bool, TimeSeriesValues::Booleans);
impl_from_ts!(Date, TimeSeriesValues::Dates);
impl_from_ts!(Time, TimeSeriesValues::Times);
impl_from_ts!(DateTime, TimeSeriesValues::DateTimes);
impl_from_ts!(Attribute, TimeSeriesValues::Attributes);
