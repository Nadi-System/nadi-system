use crate::attrs::{type_name, Attribute, Date, DateTime, Time};

use abi_stable::{
    external_types::RMutex,
    std_types::{RArc, RString, RVec},
    StableAbi,
};

pub type TimeLine = RArc<RMutex<TimeLineInner>>;

#[repr(C)]
#[derive(StableAbi, Clone, Debug)]
pub struct TimeLineInner {
    /// timestamp of the start datetime
    start: i64,
    /// timestamp of the end datetime
    end: i64,
    /// step in seconds
    step: i64,
    /// is regular timeseries or not
    regular: bool,
    /// values in string format so that we don't have to deal with time
    str_values: RVec<RString>,
    /// format string used in the str_values,
    datetimefmt: RString,
}

impl std::cmp::PartialEq for TimeLineInner {
    fn eq(&self, other: &Self) -> bool {
        // str_values and datetimefmt are for exporting/printing them
        // only, so the other fields should be good enough for eq
        self.start == other.start
            && self.end == other.end
            && self.step == other.step
            && self.regular == other.regular
    }
}

impl<'a> TimeLineInner {
    pub fn new(
        start: i64,
        end: i64,
        step: i64,
        regular: bool,
        str_values: Vec<String>,
        datetimefmt: &str,
    ) -> Self {
        Self {
            start,
            end,
            step,
            regular,
            str_values: RVec::from(
                str_values
                    .into_iter()
                    .map(RString::from)
                    .collect::<Vec<RString>>(),
            ),
            datetimefmt: RString::from(datetimefmt),
        }
    }
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

    pub fn values_as_attributes(&self) -> Vec<Attribute> {
        match self.values.clone() {
            TimeSeriesValues::Floats(v) => v.into_iter().map(Attribute::Float).collect(),
            TimeSeriesValues::Integers(v) => v.into_iter().map(Attribute::Integer).collect(),
            TimeSeriesValues::Strings(v) => v.into_iter().map(Attribute::String).collect(),
            TimeSeriesValues::Booleans(v) => v.into_iter().map(Attribute::Bool).collect(),
            TimeSeriesValues::Dates(v) => v.into_iter().map(Attribute::Date).collect(),
            TimeSeriesValues::Times(v) => v.into_iter().map(Attribute::Time).collect(),
            TimeSeriesValues::DateTimes(v) => v.into_iter().map(Attribute::DateTime).collect(),
            TimeSeriesValues::Attributes(v) => v.into(),
        }
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

    pub fn same_timeline(&self, other: &Self) -> bool {
        self.is_timeline(&other.timeline)
    }

    pub fn is_timeline(&self, tl: &TimeLine) -> bool {
        // counting on RArc PartialEq to compare properly
        abi_stable::pointer_trait::AsPtr::as_ptr(&self.timeline)
            == abi_stable::pointer_trait::AsPtr::as_ptr(tl)
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
    pub fn floats(v: Vec<f64>) -> Self {
        Self::Floats(v.into())
    }
    pub fn integers(v: Vec<i64>) -> Self {
        Self::Integers(v.into())
    }
    pub fn strings(v: Vec<RString>) -> Self {
        Self::Strings(v.into())
    }
    pub fn booleans(v: Vec<bool>) -> Self {
        Self::Booleans(v.into())
    }
    pub fn dates(v: Vec<Date>) -> Self {
        Self::Dates(v.into())
    }
    pub fn times(v: Vec<Time>) -> Self {
        Self::Times(v.into())
    }
    pub fn datetimes(v: Vec<DateTime>) -> Self {
        Self::DateTimes(v.into())
    }
    pub fn attributes(v: Vec<Attribute>) -> Self {
        Self::Attributes(v.into())
    }

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
            Self::Floats(_) => "Floats",
            Self::Integers(_) => "Integers",
            Self::Strings(_) => "Strings",
            Self::Booleans(_) => "Booleans",
            Self::Dates(_) => "Dates",
            Self::Times(_) => "Times",
            Self::DateTimes(_) => "DateTimes",
            Self::Attributes(_) => "Attributes",
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
