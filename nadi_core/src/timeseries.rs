use crate::attrs::{type_name, Attribute, Date, DateTime, FromAttribute, Time};
use crate::datafill::DataImputeError;
use crate::eval::{EvalError, EvalErrorType};
use crate::expressions::ExprResult;
use crate::tasks::{TaskContext, TaskCtxConsts};

use abi_stable::{
    external_types::RMutex,
    std_types::{RArc, RHashMap, RNone, ROption, RSome, RString, RVec},
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
    fn ts_mut(&mut self, name: &str) -> Option<&mut TimeSeries> {
        self.ts_map_mut().get_mut(name)
    }
    fn del_ts(&mut self, name: &str) -> Option<TimeSeries> {
        self.ts_map_mut().remove(name).into()
    }
    fn set_ts(&mut self, name: &str, val: TimeSeries) -> Option<TimeSeries> {
        self.ts_map_mut().insert(name.into(), val).into()
    }

    fn try_ts(&self, name: &str) -> Result<&TimeSeries, String> {
        self.ts_map()
            .get(name)
            .ok_or(format!("Timeseries `{name}` not found"))
    }

    fn try_ts_mut(&mut self, name: &str) -> Result<&mut TimeSeries, String> {
        self.ts_map_mut()
            .get_mut(name)
            .ok_or(format!("Timeseries `{name}` not found"))
    }
}

pub trait HasSeries {
    fn series_map(&self) -> &SeriesMap;
    fn series_map_mut(&mut self) -> &mut SeriesMap;
    fn series(&self, name: &str) -> Option<&Series> {
        self.series_map().get(name)
    }

    fn get_series_ref(&self, name: &str) -> Result<&Series, EvalError> {
        self.series(name)
            .ok_or(EvalErrorType::SeriesNotFound(name.to_string()).no_pos())
    }

    fn get_attribute(&self, name: &str, ind: usize) -> Result<Option<Attribute>, EvalError> {
        self.get_series_ref(name)?
            .get_attribute(ind)
            .ok_or(EvalErrorType::IndexError.no_pos())
    }

    fn del_series(&mut self, name: &str) -> Option<Series> {
        self.series_map_mut().remove(name).into()
    }
    fn set_series(&mut self, name: &str, val: Series) -> Option<Series> {
        self.series_map_mut().insert(name.into(), val).into()
    }

    fn try_series(&self, name: &str) -> Result<&Series, String> {
        self.series_map()
            .get(name)
            .ok_or(format!("Series `{name}` not found"))
    }
    fn try_series_mut(&mut self, name: &str) -> Result<&mut Series, String> {
        self.series_map_mut()
            .get_mut(name)
            .ok_or(format!("Series `{name}` not found"))
    }

    fn fill_series(&mut self, name: &str, value: Attribute) -> Result<(), String> {
        let ser: Option<Series> = self.series_map_mut().remove(name).into();
        self.set_series(
            name,
            ser.ok_or(format!("Series `{name}` not found"))?
                .fill_gaps(value)
                .map_err(|e| e.to_string())?,
        );
        Ok(())
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
        str_values: Vec<RString>,
        datetimefmt: &str,
    ) -> Self {
        Self {
            start,
            end,
            step,
            regular,
            str_values: str_values.into(),

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

    pub fn len(&self) -> usize {
        self.str_values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.str_values.is_empty()
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

impl From<TimeSeries> for Series {
    fn from(val: TimeSeries) -> Self {
        val.values
    }
}

impl std::fmt::Display for TimeSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TimeSeries {{..., {}}}", self.values)
    }
}

// to make it work with ExprResult
impl std::fmt::Debug for TimeSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "TimeSeries {{..., {}}}", self.values)
    }
}

impl PartialEq for TimeSeries {
    fn eq(&self, other: &Self) -> bool {
        self.same_timeline(other) & (self.series() == other.series())
    }
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

    pub fn len(&self) -> usize {
        self.values.len()
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    pub fn series(&self) -> &Series {
        &self.values
    }

    pub fn replace_series(&mut self, sr: Series) -> Result<(), (usize, usize)> {
        if self.len() != sr.len() {
            return Err((self.len(), sr.len()));
        }
        self.values = sr;
        Ok(())
    }

    pub fn maybe_complete(mut self) -> Self {
        self.values = self.values.maybe_complete();
        self
    }

    pub fn values<'a, T: FromSeries<'a>>(&'a self) -> Option<&'a [T]> {
        FromSeries::from_series(&self.values)
    }

    pub fn str_values<'b, 'a: 'b>(&'a self, na: &'b str) -> Box<dyn Iterator<Item = String> + 'b> {
        self.values.str_values(na)
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
    /// Masked Series and optional fill value
    Masked(MaskedSeries, ROption<Attribute>),
    /// Series without values
    Complete(CompleteSeries),
}

impl std::fmt::Display for Series {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Masked(..) => write!(f, "MaskedSeries {{...}}"),
            Self::Complete(..) => write!(f, "CompleteSeries {{...}}"),
        }
    }
}

impl TaskContext {
    pub fn show_ts(&self, ts: &TimeSeries) -> String {
        let tl = ts.timeline.lock();
        let tl = match tl.str_values.as_slice() {
            [] => format!("[]"),
            [a] => format!("[{a}]"),
            [a, b] => format!("[{a}, {b}]"),
            [a, b, c] => format!("[{a}, {b}, {c}]"),
            [a, b, .., l] => format!("[{a}, {b}, ..., {l}]"),
        };
        format!("TimeSeries({tl}, values: {})", self.show_sr(ts.series()))
    }

    pub fn show_sr(&self, sr: &Series) -> String {
        let na = TaskCtxConsts::series_show_na_as(self);
        let (pre, len, vals) = match sr {
            Series::Masked(ms, RSome(fv)) => (
                format!(
                    "MaskedSeries(len: {}, dtype: {}, valid: {}, fill: {fv})",
                    ms.len(),
                    ms.type_name(),
                    ms.len_valid()
                ),
                ms.len(),
                ms.str_values(&na),
            ),
            Series::Masked(ms, RNone) => (
                format!(
                    "MaskedSeries(len: {}, dtype: {}, valid: {})",
                    ms.len(),
                    ms.type_name(),
                    ms.len_valid()
                ),
                ms.len(),
                ms.str_values(&na),
            ),
            Series::Complete(cs) => (
                format!("Series(len: {}, dtype: {})", cs.len(), cs.type_name()),
                cs.len(),
                cs.str_values(),
            ),
        };
        let max_series_len = TaskCtxConsts::max_series_length(self);
        if len > max_series_len {
            format!(
                "{pre} [{}, ...]",
                vals.take(max_series_len)
                    .collect::<Vec<String>>()
                    .join(", ")
            )
        } else {
            format!("{pre} [{}]", vals.collect::<Vec<String>>().join(", "))
        }
    }
}

impl From<MaskedSeries> for Series {
    fn from(val: MaskedSeries) -> Series {
        if val.has_gaps() {
            return Series::Masked(val, RNone);
        }
        // if there are gaps then to_complete(_) will panic
        Series::Complete(match val {
            MaskedSeries::Floats(v) => CompleteSeries::Floats(to_complete(v)),
            MaskedSeries::Integers(v) => CompleteSeries::Integers(to_complete(v)),
            MaskedSeries::Strings(v) => CompleteSeries::Strings(to_complete(v)),
            MaskedSeries::Booleans(v) => CompleteSeries::Booleans(to_complete(v)),
            MaskedSeries::Dates(v) => CompleteSeries::Dates(to_complete(v)),
            MaskedSeries::Times(v) => CompleteSeries::Times(to_complete(v)),
            MaskedSeries::DateTimes(v) => CompleteSeries::DateTimes(to_complete(v)),
            MaskedSeries::Attributes(v) => CompleteSeries::Attributes(to_complete(v)),
        })
    }
}

impl From<CompleteSeries> for Series {
    fn from(val: CompleteSeries) -> Series {
        Series::Complete(val)
    }
}

// // TODO: This says conflicting implementation, look into it
// impl<T> From<T> for Series
// where
//     CompleteSeries: From<T>,
// {
//     fn from(val: T) -> Series {
//         CompleteSeries::from(val).into()
//     }
// }

// impl<T> From<T> for Series
// where
//     MaskedSeries: From<T>,
// {
//     fn from(val: T) -> Series {
//         MaskedSeries::from(val).into()
//     }
// }

/// Matches and calls respective functions for all variations of [`Series`]
macro_rules! forward_funcs {
    ($($func:ident -> $ret:ty),*) => {
	impl Series {
	    $(
		pub fn $func(&self) -> $ret {
		    match self {
			Self::Masked(v, _) => v. $func (),
			Self::Complete(v) => v. $func (),
		    }
		}
	    )*
	}
    }
}

forward_funcs! {
    len -> usize,
    is_empty -> bool,
    type_name -> &str,
    minimum -> Result<Option<Attribute>, EvalErrorType>,
    maximum -> Result<Option<Attribute>, EvalErrorType>
}

impl Series {
    pub fn is_masked(&self) -> bool {
        matches!(self, Self::Masked(_, _))
    }

    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete(_))
    }

    pub fn len_valid(&self) -> usize {
        match self {
            Self::Masked(v, _) => v.len_valid(),
            Self::Complete(v) => v.len(),
        }
    }

    pub fn from_attr(vals: &Attribute, dtype: &str) -> Result<Self, String> {
        CompleteSeries::from_attr(vals, dtype).map(Self::Complete)
    }

    pub fn shift(self, val: i64) -> Self {
        match self {
            Self::Masked(v, f) => Self::Masked(v.shift(val), f),
            Self::Complete(v) => Self::Masked(v.shift(val), RNone),
        }
    }

    pub fn str_values<'b, 'a: 'b>(&'a self, na: &'b str) -> Box<dyn Iterator<Item = String> + 'b> {
        match self {
            Self::Masked(v, _) => v.str_values(na),
            Self::Complete(v) => v.str_values(),
        }
    }

    /// Map the function to the values
    pub fn map_values(
        self,
        func: &dyn Fn(Option<Attribute>) -> Result<ExprResult, EvalError>,
    ) -> Result<Self, EvalError> {
        match self {
            Self::Masked(v, fv) => Ok(Self::Masked(v.map_values(func)?, fv)),
            Self::Complete(v) => v.map_values(func),
        }
    }

    /// Retype the attributes series into a type if homogeneous
    pub fn retype(self) -> Self {
        match self {
            Self::Masked(v, fv) => Self::Masked(v.retype(), fv),
            Self::Complete(v) => Self::Complete(v.retype()),
        }
    }

    pub fn maybe_complete(self) -> Self {
        match self {
            Self::Complete(_) => self,
            Self::Masked(ms, fill) => {
                if ms.has_gaps() {
                    Self::Masked(ms, fill)
                } else {
                    Self::Complete(ms.complete().unwrap())
                }
            }
        }
    }

    pub fn fill_gaps(self, value: Attribute) -> Result<Self, DataImputeError> {
        match self {
            Self::Complete(_) => Ok(self),
            Self::Masked(ms, _) => ms.fill_gaps(value).map(Self::Complete),
        }
    }

    /// outer Option is for index, inner is for data gap
    pub fn get_attribute(&self, index: usize) -> Option<Option<Attribute>> {
        match self {
            Self::Complete(cs) => cs.get_attribute(index).map(Some),
            Self::Masked(ms, fill) => ms
                .get_attribute(index)
                .map(|v| v.or_else(|| fill.clone().into_option())),
        }
    }

    pub fn to_attributes(self) -> Option<Vec<Attribute>> {
        Some(match self {
            Series::Masked(ms, _) => {
                let cs = ms.complete()?;
                cs.to_attributes()
            }
            Series::Complete(cs) => cs.to_attributes(),
        })
    }

    pub fn to_opt_attributes(self) -> Vec<ROption<Attribute>> {
        match self {
            Series::Masked(ms, _) => ms.to_attributes(),
            Series::Complete(cs) => cs.to_attributes().into_iter().map(RSome).collect(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum MaskedSeries {
    Floats(RVec<ROption<f64>>),
    Integers(RVec<ROption<i64>>),
    Strings(RVec<ROption<RString>>),
    Booleans(RVec<ROption<bool>>),
    Dates(RVec<ROption<Date>>),
    Times(RVec<ROption<Time>>),
    DateTimes(RVec<ROption<DateTime>>),
    Attributes(RVec<ROption<Attribute>>),
}

impl MaskedSeries {
    pub fn floats(v: Vec<ROption<f64>>) -> Self {
        Self::Floats(v.into())
    }
    pub fn integers(v: Vec<ROption<i64>>) -> Self {
        Self::Integers(v.into())
    }
    pub fn strings(v: Vec<ROption<RString>>) -> Self {
        Self::Strings(v.into())
    }
    pub fn booleans(v: Vec<ROption<bool>>) -> Self {
        Self::Booleans(v.into())
    }
    pub fn dates(v: Vec<ROption<Date>>) -> Self {
        Self::Dates(v.into())
    }
    pub fn times(v: Vec<ROption<Time>>) -> Self {
        Self::Times(v.into())
    }
    pub fn datetimes(v: Vec<ROption<DateTime>>) -> Self {
        Self::DateTimes(v.into())
    }
    pub fn attributes(v: Vec<ROption<Attribute>>) -> Self {
        Self::Attributes(v.into())
    }

    pub fn shift(self, val: i64) -> Self {
        match self {
            Self::Floats(v) => Self::Floats(shift_masked(v, val)),
            Self::Integers(v) => Self::Integers(shift_masked(v, val)),
            Self::Strings(v) => Self::Strings(shift_masked(v, val)),
            Self::Booleans(v) => Self::Booleans(shift_masked(v, val)),
            Self::Dates(v) => Self::Dates(shift_masked(v, val)),
            Self::Times(v) => Self::Times(shift_masked(v, val)),
            Self::DateTimes(v) => Self::DateTimes(shift_masked(v, val)),
            Self::Attributes(v) => Self::Attributes(shift_masked(v, val)),
        }
    }

    pub fn map_values(
        self,
        func: &dyn Fn(Option<Attribute>) -> Result<ExprResult, EvalError>,
    ) -> Result<Self, EvalError> {
        Ok(Self::attributes(
            self.to_attributes()
                .into_iter()
                .map(|v| Ok(func(v.into_option())?.to_attribute().into()))
                .collect::<Result<Vec<ROption<Attribute>>, EvalError>>()?,
        )
        .retype())
    }

    /// Retype the attributes series into a type if homogeneous
    pub fn retype(self) -> Self {
        match &self {
            Self::Attributes(attrs) => {
                let first = match attrs.iter().filter_map(|v| v.clone().into_option()).next() {
                    Some(f) => f,
                    None => return self,
                };
                match first {
                    Attribute::Bool(_) => {
                        let mut vals = Vec::<ROption<bool>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match bool::from_attr(v) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Booleans(vals.into())
                    }
                    Attribute::Integer(_) => {
                        let mut vals = Vec::<ROption<i64>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match i64::from_attr(v) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Integers(vals.into())
                    }
                    Attribute::Float(_) => {
                        let mut vals = Vec::<ROption<f64>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match f64::from_attr(v)
                                    .or_else(|| i64::from_attr(v).map(|v| v as f64))
                                {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Floats(vals.into())
                    }
                    Attribute::String(_) => {
                        let mut vals = Vec::<ROption<RString>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match RString::from_attr(v) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Strings(vals.into())
                    }
                    Attribute::Date(_) => {
                        let mut vals = Vec::<ROption<Date>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match Date::from_attr(v) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Dates(vals.into())
                    }
                    Attribute::Time(_) => {
                        let mut vals = Vec::<ROption<Time>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match Time::from_attr(v) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::Times(vals.into())
                    }
                    Attribute::DateTime(_) => {
                        let mut vals = Vec::<ROption<DateTime>>::with_capacity(attrs.len());
                        for a in attrs {
                            match a {
                                RSome(v) => match DateTime::from_attr(v).or_else(|| {
                                    Date::from_attr(v).map(|d| d.with_time(Time::default()))
                                }) {
                                    Some(v) => vals.push(RSome(v)),
                                    // if the value is not bool (first one was), we can not retype
                                    None => return self,
                                },
                                RNone => vals.push(RNone),
                            }
                        }
                        MaskedSeries::DateTimes(vals.into())
                    }
                    _ => self,
                }
            }
            _ => self,
        }
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

    pub fn len_valid(&self) -> usize {
        match self {
            Self::Floats(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Integers(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Strings(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Booleans(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Dates(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Times(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::DateTimes(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
            Self::Attributes(v) => v.iter().filter(|v| matches!(v, RSome(_))).count(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn str_values<'b, 'a: 'b>(&'a self, na: &'b str) -> Box<dyn Iterator<Item = String> + 'b> {
        match self {
            Self::Floats(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::Integers(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::Strings(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(|s| format!("{s:?}"))
                    .unwrap_or(na.to_string())
            })),
            Self::Booleans(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::Dates(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::Times(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::DateTimes(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
            Self::Attributes(v) => Box::new(v.iter().map(|v| {
                v.as_ref()
                    .map(ToString::to_string)
                    .unwrap_or(na.to_string())
            })),
        }
    }

    pub fn has_gaps(&self) -> bool {
        match self {
            Self::Floats(v) => has_gaps(v),
            Self::Integers(v) => has_gaps(v),
            Self::Strings(v) => has_gaps(v),
            Self::Booleans(v) => has_gaps(v),
            Self::Dates(v) => has_gaps(v),
            Self::Times(v) => has_gaps(v),
            Self::DateTimes(v) => has_gaps(v),
            Self::Attributes(v) => has_gaps(v),
        }
    }

    pub fn get_nulls(&self) -> Vec<bool> {
        match self {
            Self::Floats(v) => get_nulls(v),
            Self::Integers(v) => get_nulls(v),
            Self::Strings(v) => get_nulls(v),
            Self::Booleans(v) => get_nulls(v),
            Self::Dates(v) => get_nulls(v),
            Self::Times(v) => get_nulls(v),
            Self::DateTimes(v) => get_nulls(v),
            Self::Attributes(v) => get_nulls(v),
        }
    }

    pub fn get_valids(&self) -> Vec<bool> {
        match self {
            Self::Floats(v) => get_valids(v),
            Self::Integers(v) => get_valids(v),
            Self::Strings(v) => get_valids(v),
            Self::Booleans(v) => get_valids(v),
            Self::Dates(v) => get_valids(v),
            Self::Times(v) => get_valids(v),
            Self::DateTimes(v) => get_valids(v),
            Self::Attributes(v) => get_valids(v),
        }
    }

    pub fn data_blocks(&self, valid: bool) -> Vec<(usize, usize)> {
        let valids = if valid {
            self.get_valids()
        } else {
            self.get_nulls()
        };
        if valids.is_empty() {
            return Vec::new();
        }
        let change: Vec<bool> = valids
            .iter()
            .zip(&valids[1..])
            .map(|(a, b)| a != b)
            .collect();
        let num_blocks = change.iter().filter(|v| **v).count() + 1;
        // position length and flag for the blocks
        let mut blocks: Vec<(usize, usize, bool)> =
            (0..num_blocks).map(|_| (0, 0, false)).collect();
        blocks[0].2 = valids[0];
        let mut block_id: Vec<usize> = valids.iter().map(|_| 0).collect();
        for (i, ch) in change.into_iter().enumerate() {
            let id = block_id[i] + ch as usize;
            block_id[i + 1] = id;
            blocks[id].1 += 1;
            if ch {
                blocks[id].0 = i + 1;
                blocks[id].2 = valids[i + 1];
            }
        }
        blocks
            .into_iter()
            .filter_map(|(pos, len, val)| val.then_some((pos, len)))
            .collect()
    }

    pub fn minimum(&self) -> Result<Option<Attribute>, EvalErrorType> {
        Ok(match self {
            Self::Floats(v) => v
                .iter()
                .filter_map(|v| v.into_option())
                .reduce(f64::min)
                .map(Attribute::Float),
            Self::Integers(v) => minimum_masked(v).map(Attribute::Integer),
            Self::Strings(v) => minimum_masked(v).map(Attribute::String),
            Self::Booleans(v) => minimum_masked(v).map(Attribute::Bool),
            Self::Dates(v) => minimum_masked(v).map(Attribute::Date),
            Self::Times(v) => minimum_masked(v).map(Attribute::Time),
            Self::DateTimes(v) => minimum_masked(v).map(Attribute::DateTime),
            Self::Attributes(_) => return Err(EvalErrorType::InvalidOperation),
        })
    }

    pub fn maximum(&self) -> Result<Option<Attribute>, EvalErrorType> {
        Ok(match self {
            Self::Floats(v) => v
                .iter()
                .filter_map(|v| v.into_option())
                .reduce(f64::max)
                .map(Attribute::Float),
            Self::Integers(v) => maximum_masked(v).map(Attribute::Integer),
            Self::Strings(v) => maximum_masked(v).map(Attribute::String),
            Self::Booleans(v) => maximum_masked(v).map(Attribute::Bool),
            Self::Dates(v) => maximum_masked(v).map(Attribute::Date),
            Self::Times(v) => maximum_masked(v).map(Attribute::Time),
            Self::DateTimes(v) => maximum_masked(v).map(Attribute::DateTime),
            Self::Attributes(_) => return Err(EvalErrorType::InvalidOperation),
        })
    }

    pub fn to_attributes(self) -> Vec<ROption<Attribute>> {
        match self {
            Self::Floats(v) => v.into_iter().map(|a| a.map(Attribute::Float)).collect(),
            Self::Integers(v) => v.into_iter().map(|a| a.map(Attribute::Integer)).collect(),
            Self::Strings(v) => v.into_iter().map(|a| a.map(Attribute::String)).collect(),
            Self::Booleans(v) => v.into_iter().map(|a| a.map(Attribute::Bool)).collect(),
            Self::Dates(v) => v.into_iter().map(|a| a.map(Attribute::Date)).collect(),
            Self::Times(v) => v.into_iter().map(|a| a.map(Attribute::Time)).collect(),
            Self::DateTimes(v) => v.into_iter().map(|a| a.map(Attribute::DateTime)).collect(),
            Self::Attributes(v) => v.into(),
        }
    }

    pub fn get_attribute(&self, index: usize) -> Option<Option<Attribute>> {
        Some(match self {
            Self::Floats(v) => (*v.get(index)?).into_option().map(Attribute::Float),
            Self::Integers(v) => (*v.get(index)?).into_option().map(Attribute::Integer),
            Self::Strings(v) => v.get(index)?.clone().into_option().map(Attribute::String),
            Self::Booleans(v) => (*v.get(index)?).into_option().map(Attribute::Bool),
            Self::Dates(v) => v.get(index)?.clone().into_option().map(Attribute::Date),
            Self::Times(v) => v.get(index)?.clone().into_option().map(Attribute::Time),
            Self::DateTimes(v) => v.get(index)?.clone().into_option().map(Attribute::DateTime),
            Self::Attributes(v) => v.get(index)?.clone().into_option(),
        })
    }

    pub fn complete(self) -> Option<CompleteSeries> {
        if self.has_gaps() {
            return None;
        }
        Some(match self {
            Self::Floats(v) => CompleteSeries::Floats(to_complete(v)),
            Self::Integers(v) => CompleteSeries::Integers(to_complete(v)),
            Self::Strings(v) => CompleteSeries::Strings(to_complete(v)),
            Self::Booleans(v) => CompleteSeries::Booleans(to_complete(v)),
            Self::Dates(v) => CompleteSeries::Dates(to_complete(v)),
            Self::Times(v) => CompleteSeries::Times(to_complete(v)),
            Self::DateTimes(v) => CompleteSeries::DateTimes(to_complete(v)),
            Self::Attributes(v) => CompleteSeries::Attributes(to_complete(v)),
        })
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

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum CompleteSeries {
    Floats(RVec<f64>),
    Integers(RVec<i64>),
    Strings(RVec<RString>),
    Booleans(RVec<bool>),
    Dates(RVec<Date>),
    Times(RVec<Time>),
    DateTimes(RVec<DateTime>),
    Attributes(RVec<Attribute>),
}

fn shift<T>(vals: RVec<T>, s: i64) -> RVec<ROption<T>> {
    let mut new_vals = RVec::with_capacity(vals.len());
    if s > 0 {
        for _ in 0..s {
            new_vals.push(RNone);
        }
        let rest = vals.len() - s as usize;
        for v in vals.into_iter().take(rest) {
            new_vals.push(RSome(v));
        }
    } else {
        let rest = (-s) as usize;
        for v in vals.into_iter().skip(rest) {
            new_vals.push(RSome(v));
        }
        for _ in 0..rest {
            new_vals.push(RNone);
        }
    }
    new_vals
}

fn shift_masked<T>(vals: RVec<ROption<T>>, s: i64) -> RVec<ROption<T>> {
    let mut new_vals = RVec::with_capacity(vals.len());
    if s > 0 {
        for _ in 0..s {
            new_vals.push(RNone);
        }
        let rest = vals.len() - s as usize;
        for v in vals.into_iter().take(rest) {
            new_vals.push(v);
        }
    } else {
        let rest = (-s) as usize;
        for v in vals.into_iter().skip(rest) {
            new_vals.push(v);
        }
        for _ in 0..rest {
            new_vals.push(RNone);
        }
    }
    new_vals
}

impl CompleteSeries {
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

    pub fn shift(self, val: i64) -> MaskedSeries {
        match self {
            Self::Floats(v) => MaskedSeries::Floats(shift(v, val)),
            Self::Integers(v) => MaskedSeries::Integers(shift(v, val)),
            Self::Strings(v) => MaskedSeries::Strings(shift(v, val)),
            Self::Booleans(v) => MaskedSeries::Booleans(shift(v, val)),
            Self::Dates(v) => MaskedSeries::Dates(shift(v, val)),
            Self::Times(v) => MaskedSeries::Times(shift(v, val)),
            Self::DateTimes(v) => MaskedSeries::DateTimes(shift(v, val)),
            Self::Attributes(v) => MaskedSeries::Attributes(shift(v, val)),
        }
    }

    pub fn str_values<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        match self {
            Self::Floats(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Integers(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Strings(v) => Box::new(v.iter().map(|s| format!("{s:?}"))),
            Self::Booleans(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Dates(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Times(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::DateTimes(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Attributes(v) => Box::new(v.iter().map(ToString::to_string)),
        }
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

    /// Returns Series as the function could return null, converting it to MaskedSeries
    pub fn map_values(
        self,
        func: &dyn Fn(Option<Attribute>) -> Result<ExprResult, EvalError>,
    ) -> Result<Series, EvalError> {
        Ok(Series::from(MaskedSeries::attributes(
            self.to_attributes()
                .into_iter()
                .map(|a| Ok(ROption::from(func(Some(a))?.to_attribute())))
                .collect::<Result<Vec<ROption<Attribute>>, EvalError>>()?,
        ))
        .retype())
    }

    /// Retype the attributes series into a type if homogeneous
    pub fn retype(self) -> Self {
        match &self {
            Self::Attributes(attrs) => {
                let first = match attrs.iter().next() {
                    Some(f) => f,
                    None => return self,
                };
                match first {
                    Attribute::Bool(_) => {
                        let mut vals = Vec::<bool>::with_capacity(attrs.len());
                        for a in attrs {
                            match bool::from_attr(a) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Booleans(vals.into())
                    }
                    Attribute::Integer(_) => {
                        let mut vals = Vec::<i64>::with_capacity(attrs.len());
                        for a in attrs {
                            match i64::from_attr(a) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Integers(vals.into())
                    }
                    Attribute::Float(_) => {
                        let mut vals = Vec::<f64>::with_capacity(attrs.len());
                        for a in attrs {
                            match f64::from_attr(a).or_else(|| i64::from_attr(a).map(|v| v as f64))
                            {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Floats(vals.into())
                    }
                    Attribute::String(_) => {
                        let mut vals = Vec::<RString>::with_capacity(attrs.len());
                        for a in attrs {
                            match RString::from_attr(a) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Strings(vals.into())
                    }
                    Attribute::Date(_) => {
                        let mut vals = Vec::<Date>::with_capacity(attrs.len());
                        for a in attrs {
                            match Date::from_attr(a) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Dates(vals.into())
                    }
                    Attribute::Time(_) => {
                        let mut vals = Vec::<Time>::with_capacity(attrs.len());
                        for a in attrs {
                            match Time::from_attr(a) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::Times(vals.into())
                    }
                    Attribute::DateTime(_) => {
                        let mut vals = Vec::<DateTime>::with_capacity(attrs.len());
                        for a in attrs {
                            match DateTime::from_attr(a).or_else(|| {
                                Date::from_attr(a).map(|d| d.with_time(Time::default()))
                            }) {
                                Some(v) => vals.push(v),
                                // if the value is not bool (first one was), we can not retype
                                None => return self,
                            }
                        }
                        CompleteSeries::DateTimes(vals.into())
                    }
                    _ => self,
                }
            }
            _ => self,
        }
    }

    pub fn to_attributes(self) -> Vec<Attribute> {
        match self {
            Self::Floats(v) => v.into_iter().map(Attribute::Float).collect(),
            Self::Integers(v) => v.into_iter().map(Attribute::Integer).collect(),
            Self::Strings(v) => v.into_iter().map(Attribute::String).collect(),
            Self::Booleans(v) => v.into_iter().map(Attribute::Bool).collect(),
            Self::Dates(v) => v.into_iter().map(Attribute::Date).collect(),
            Self::Times(v) => v.into_iter().map(Attribute::Time).collect(),
            Self::DateTimes(v) => v.into_iter().map(Attribute::DateTime).collect(),
            Self::Attributes(v) => v.into(),
        }
    }

    pub fn get_attribute(&self, index: usize) -> Option<Attribute> {
        Some(match self {
            Self::Floats(v) => Attribute::Float(*v.get(index)?),
            Self::Integers(v) => Attribute::Integer(*v.get(index)?),
            Self::Strings(v) => Attribute::String(v.get(index)?.clone()),
            Self::Booleans(v) => Attribute::Bool(*v.get(index)?),
            Self::Dates(v) => Attribute::Date(v.get(index)?.clone()),
            Self::Times(v) => Attribute::Time(v.get(index)?.clone()),
            Self::DateTimes(v) => Attribute::DateTime(v.get(index)?.clone()),
            Self::Attributes(v) => v.get(index)?.clone(),
        })
    }

    pub fn minimum(&self) -> Result<Option<Attribute>, EvalErrorType> {
        Ok(match self {
            Self::Floats(v) => v.iter().copied().reduce(f64::min).map(Attribute::Float),
            Self::Integers(v) => minimum(v).map(Attribute::Integer),
            Self::Strings(v) => minimum(v).map(Attribute::String),
            Self::Booleans(v) => minimum(v).map(Attribute::Bool),
            Self::Dates(v) => minimum(v).map(Attribute::Date),
            Self::Times(v) => minimum(v).map(Attribute::Time),
            Self::DateTimes(v) => minimum(v).map(Attribute::DateTime),
            Self::Attributes(_) => return Err(EvalErrorType::InvalidOperation),
        })
    }

    pub fn maximum(&self) -> Result<Option<Attribute>, EvalErrorType> {
        Ok(match self {
            Self::Floats(v) => v.iter().copied().reduce(f64::max).map(Attribute::Float),
            Self::Integers(v) => maximum(v).map(Attribute::Integer),
            Self::Strings(v) => maximum(v).map(Attribute::String),
            Self::Booleans(v) => maximum(v).map(Attribute::Bool),
            Self::Dates(v) => maximum(v).map(Attribute::Date),
            Self::Times(v) => maximum(v).map(Attribute::Time),
            Self::DateTimes(v) => maximum(v).map(Attribute::DateTime),
            Self::Attributes(_) => return Err(EvalErrorType::InvalidOperation),
        })
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
    ($t:tt, $x:ident) => {
        impl<'a> FromSeries<'a> for $t {
            fn from_series(value: &Series) -> Option<&[$t]> {
                match value {
                    Series::Complete(CompleteSeries::$x(v)) => Some(v.as_slice()),
                    _ => None,
                }
            }
            fn from_series_mut(value: &mut Series) -> Option<&mut [$t]> {
                match value {
                    Series::Complete(CompleteSeries::$x(v)) => Some(v.as_mut_slice()),
                    _ => None,
                }
            }
        }

        impl<'a> FromSeries<'a> for ROption<$t> {
            fn from_series(value: &Series) -> Option<&[ROption<$t>]> {
                match value {
                    Series::Masked(MaskedSeries::$x(v), _) => Some(v.as_slice()),
                    _ => None,
                }
            }
            fn from_series_mut(value: &mut Series) -> Option<&mut [ROption<$t>]> {
                match value {
                    Series::Masked(MaskedSeries::$x(v), _) => Some(v.as_mut_slice()),
                    _ => None,
                }
            }
        }

        impl From<&[$t]> for CompleteSeries {
            fn from(item: &[$t]) -> Self {
                CompleteSeries::$x(item.into())
            }
        }
        impl From<Vec<$t>> for CompleteSeries {
            fn from(item: Vec<$t>) -> Self {
                CompleteSeries::$x(RVec::from(item))
            }
        }
        impl From<RVec<$t>> for CompleteSeries {
            fn from(item: RVec<$t>) -> Self {
                CompleteSeries::$x(item)
            }
        }

        impl From<Vec<Option<$t>>> for MaskedSeries {
            fn from(item: Vec<Option<$t>>) -> Self {
                MaskedSeries::$x(RVec::from_iter(item.into_iter().map(ROption::from)))
            }
        }
        impl From<RVec<ROption<$t>>> for MaskedSeries {
            fn from(item: RVec<ROption<$t>>) -> Self {
                MaskedSeries::$x(item)
            }
        }
    };
}

impl_from_series!(f64, Floats);
impl_from_series!(i64, Integers);
impl_from_series!(RString, Strings);
impl_from_series!(bool, Booleans);
impl_from_series!(Date, Dates);
impl_from_series!(Time, Times);
impl_from_series!(DateTime, DateTimes);
impl_from_series!(Attribute, Attributes);

// Generic functions to apply to all types of series

fn has_gaps<T>(vals: &RVec<ROption<T>>) -> bool {
    vals.iter().any(|v| v.is_none())
}

/// To convert a pre-checked not-null series to a complete series
fn to_complete<T>(vals: RVec<ROption<T>>) -> RVec<T> {
    vals.into_iter().map(|v| v.unwrap()).collect()
}

fn minimum<T: Clone + Ord>(vals: &RVec<T>) -> Option<T> {
    vals.iter().min().cloned()
}

fn minimum_masked<T: Clone + Ord>(vals: &RVec<ROption<T>>) -> Option<T> {
    vals.iter()
        .filter_map(|v| v.as_ref().into_option())
        .min()
        .cloned()
}
fn maximum<T: Clone + Ord>(vals: &RVec<T>) -> Option<T> {
    vals.iter().max().cloned()
}

fn maximum_masked<T: Clone + Ord>(vals: &RVec<ROption<T>>) -> Option<T> {
    vals.iter()
        .filter_map(|v| v.as_ref().into_option())
        .max()
        .cloned()
}

fn get_nulls<T>(vals: &RVec<ROption<T>>) -> Vec<bool> {
    vals.into_iter().map(|v| v.is_none()).collect()
}

fn get_valids<T>(vals: &RVec<ROption<T>>) -> Vec<bool> {
    vals.into_iter().map(|v| v.is_some()).collect()
}
