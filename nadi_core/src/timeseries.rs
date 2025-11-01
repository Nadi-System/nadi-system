use crate::attrs::{type_name, Attribute, Date, DateTime, FromAttribute, Time};
use crate::datafill::DataImputeError;

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

    // pub fn values_as_attributes(&self) -> Vec<Attribute> {
    //     self.values.clone().to_attributes()
    // }

    pub fn series(&self) -> &Series {
        &self.values
    }

    pub fn maybe_complete(mut self) -> Self {
        self.values = self.values.maybe_complete();
        self
    }

    pub fn values<'a, T: FromSeries<'a>>(&'a self) -> Option<&'a [T]> {
        FromSeries::from_series(&self.values)
    }

    pub fn str_values<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        self.values.str_values()
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
    type_name -> &str
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

    pub fn str_values<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        match self {
            Self::Masked(v, _) => v.str_values(),
            Self::Complete(v) => v.str_values(),
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

    // pub fn minimum(&self) -> Attribute {}
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

fn has_gaps<T>(vals: &RVec<ROption<T>>) -> bool {
    vals.iter().find(|v| v.is_none()).is_some()
}

fn to_complete<T>(vals: RVec<ROption<T>>) -> RVec<T> {
    vals.into_iter().map(|v| v.unwrap()).collect()
}

fn minimum<T>(vals: RVec<ROption<T>>) -> T {
    // Just to make it compile for now
    vals.into_iter().map(|v| v.unwrap()).next().unwrap()
}

fn get_nulls<T>(vals: &RVec<ROption<T>>) -> Vec<bool> {
    vals.into_iter().map(|v| v.is_none()).collect()
}

fn get_valids<T>(vals: &RVec<ROption<T>>) -> Vec<bool> {
    vals.into_iter().map(|v| v.is_some()).collect()
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

    pub fn str_values<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        match self {
            Self::Floats(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Integers(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Strings(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Booleans(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Dates(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Times(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::DateTimes(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
            Self::Attributes(v) => Box::new(
                v.iter()
                    .map(|v| v.as_ref().map(ToString::to_string).unwrap_or_default()),
            ),
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
            .filter_map(|(pos, len, val)| val.then(|| (pos, len)))
            .collect()
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

    pub fn str_values<'a>(&'a self) -> Box<dyn Iterator<Item = String> + 'a> {
        match self {
            Self::Floats(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Integers(v) => Box::new(v.iter().map(ToString::to_string)),
            Self::Strings(v) => Box::new(v.iter().map(ToString::to_string)),
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
