use crate::attrs::{type_name, Attribute, Date, DateTime, FromAttribute, Time};

use abi_stable::{
    external_types::RMutex,
    std_types::{RArc, RHashMap, RString, RVec},
    StableAbi,
};

pub type TimeLine = RArc<RMutex<TimeLineInner>>;
pub type TsMap = RHashMap<RString, TimeSeries>;
pub type SeriesMap = RHashMap<RString, Series>;

pub trait HasTimeSeries {
    fn ts_map(&self) -> &TsMap;
    fn ts_map_mut(&mut self) -> &mut TsMap;
    fn ts(&self, name: &str) -> Option<&TimeSeries> {
        self.ts_map().get(name)
    }
    fn del_ts(&mut self, name: &str) -> Option<TimeSeries> {
        self.ts_map_mut().remove(name.into()).into()
    }
    fn set_ts(&mut self, name: &str, val: TimeSeries) -> Option<TimeSeries> {
        self.ts_map_mut().insert(name.into(), val).into()
    }

    fn try_ts(&self, name: &str) -> Result<&TimeSeries, String> {
        self.ts_map()
            .get(name)
            .ok_or(format!("Timeseries `{name}` not found"))
    }
}

pub trait HasSeries {
    fn series_map(&self) -> &SeriesMap;
    fn series_map_mut(&mut self) -> &mut SeriesMap;
    fn series(&self, name: &str) -> Option<&Series> {
        self.series_map().get(name)
    }
    fn del_series(&mut self, name: &str) -> Option<Series> {
        self.series_map_mut().remove(name.into()).into()
    }
    fn set_series(&mut self, name: &str, val: Series) -> Option<Series> {
        self.series_map_mut().insert(name.into(), val).into()
    }

    fn try_series(&self, name: &str) -> Result<&Series, String> {
        self.series_map()
            .get(name)
            .ok_or(format!("Series `{name}` not found"))
    }
}

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
    values: Series,
}

impl TimeSeries {
    pub fn new(timeline: TimeLine, values: Series) -> Self {
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
        self.values.clone().to_attributes()
    }

    pub fn series(&self) -> &Series {
        &self.values
    }

    pub fn values<'a, T: FromSeries<'a>>(&'a self) -> Option<&'a [T]> {
        FromSeries::from_series(&self.values)
    }

    pub fn values_mut<'a, T: FromSeries<'a>>(&'a mut self) -> Option<&'a mut [T]> {
        FromSeries::from_series_mut(&mut self.values)
    }

    pub fn try_values<'a, T: FromSeries<'a>>(&'a self) -> Result<&'a [T], String> {
        FromSeries::try_from_series(&self.values)
    }
    pub fn try_values_mut<'a, T: FromSeries<'a>>(&'a mut self) -> Result<&'a mut [T], String> {
        FromSeries::try_from_series_mut(&mut self.values)
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
pub enum Series {
    Floats(RVec<f64>),
    Integers(RVec<i64>),
    Strings(RVec<RString>),
    Booleans(RVec<bool>),
    Dates(RVec<Date>),
    Times(RVec<Time>),
    DateTimes(RVec<DateTime>),
    Attributes(RVec<Attribute>),
}

impl Series {
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

    pub fn from_attr(vals: &Attribute, dtype: &str) -> Result<Self, String> {
        let sr = match dtype {
            "Floats" => {
                let vals: Vec<f64> = FromAttribute::try_from_attr(vals)?;
                Self::Floats(vals.into())
            }
            "Integers" => {
                let vals: Vec<i64> = FromAttribute::try_from_attr(vals)?;
                Self::Integers(vals.into())
            }
            "Strings" => {
                let vals: Vec<RString> = FromAttribute::try_from_attr(vals)?;
                Self::Strings(vals.into())
            }
            "Booleans" => {
                let vals: Vec<bool> = FromAttribute::try_from_attr(vals)?;
                Self::Booleans(vals.into())
            }
            "Dates" => {
                let vals: Vec<Date> = FromAttribute::try_from_attr(vals)?;
                Self::Dates(vals.into())
            }
            "Times" => {
                let vals: Vec<Time> = FromAttribute::try_from_attr(vals)?;
                Self::Times(vals.into())
            }
            "DateTimes" => {
                let vals: Vec<DateTime> = FromAttribute::try_from_attr(vals)?;
                Self::DateTimes(vals.into())
            }
            "Attributes" => {
                let vals: Vec<Attribute> = FromAttribute::try_from_attr(vals)?;
                Self::Attributes(vals.into())
            }
            t => return Err(format!("Unknown Series dtype {t}")),
        };
        Ok(sr)
    }

    pub fn to_attributes(self) -> Vec<Attribute> {
        match self {
            Series::Floats(v) => v.into_iter().map(Attribute::Float).collect(),
            Series::Integers(v) => v.into_iter().map(Attribute::Integer).collect(),
            Series::Strings(v) => v.into_iter().map(Attribute::String).collect(),
            Series::Booleans(v) => v.into_iter().map(Attribute::Bool).collect(),
            Series::Dates(v) => v.into_iter().map(Attribute::Date).collect(),
            Series::Times(v) => v.into_iter().map(Attribute::Time).collect(),
            Series::DateTimes(v) => v.into_iter().map(Attribute::DateTime).collect(),
            Series::Attributes(v) => v.into(),
        }
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

pub trait FromSeries<'a>: Sized {
    fn from_series(value: &'a Series) -> Option<&'a [Self]>;
    fn from_series_mut(value: &'a mut Series) -> Option<&'a mut [Self]>;
    fn try_from_series(value: &'a Series) -> Result<&'a [Self], String> {
        let ermsg = format!(
            "Incorrect Type: series of `{}` cannot be converted to `{}`",
            value.type_name(),
            type_name::<Self>()
        );
        FromSeries::from_series(value).ok_or(ermsg)
    }
    fn try_from_series_mut(value: &'a mut Series) -> Result<&'a mut [Self], String> {
        let ermsg = format!(
            "Incorrect Type: series of `{}` cannot be converted to `{}`",
            value.type_name(),
            type_name::<Self>()
        );
        FromSeries::from_series_mut(value).ok_or(ermsg)
    }
}

macro_rules! impl_from_series {
    ($t: tt, $x: path) => {
        impl<'a> FromSeries<'a> for $t {
            fn from_series(value: &Series) -> Option<&[$t]> {
                match value {
                    $x(v) => Some(v.as_slice()),
                    _ => None,
                }
            }
            fn from_series_mut(value: &mut Series) -> Option<&mut [$t]> {
                match value {
                    $x(v) => Some(v.as_mut_slice()),
                    _ => None,
                }
            }
        }

        impl From<&[$t]> for Series {
            fn from(item: &[$t]) -> Self {
                $x(item.into())
            }
        }
        impl From<Vec<$t>> for Series {
            fn from(item: Vec<$t>) -> Self {
                $x(RVec::from(item))
            }
        }
        impl From<RVec<$t>> for Series {
            fn from(item: RVec<$t>) -> Self {
                $x(item)
            }
        }
    };
}

impl_from_series!(f64, Series::Floats);
impl_from_series!(i64, Series::Integers);
impl_from_series!(RString, Series::Strings);
impl_from_series!(bool, Series::Booleans);
impl_from_series!(Date, Series::Dates);
impl_from_series!(Time, Series::Times);
impl_from_series!(DateTime, Series::DateTimes);
impl_from_series!(Attribute, Series::Attributes);
