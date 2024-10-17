use crate::parser::attrs::attr_file;
use anyhow::Context;
use colored::Colorize;

use std::path::PathBuf;
use string_template_plus::Template;

use abi_stable::{
    std_types::{
        RHashMap,
        ROption::{self, RNone},
        RSlice, RStr, RString, RVec, Tuple2,
    },
    StableAbi,
};

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum Attribute {
    Bool(bool),
    String(RString),
    Integer(i64),
    Float(f64),
    Date(Date),
    Time(Time),
    DateTime(DateTime),
    Array(RVec<Attribute>),
    Table(AttrMap),
}

impl Default for Attribute {
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl ToString for Attribute {
    fn to_string(&self) -> String {
        match self {
            Self::Bool(v) => format!("{v:?}"),
            Self::String(v) => format!("{v:?}"),
            Self::Integer(v) => format!("{v:?}"),
            Self::Float(v) => format!("{v:?}"),
            Self::Date(v) => v.to_string(),
            Self::Time(v) => v.to_string(),
            Self::DateTime(v) => v.to_string(),
            Self::Array(v) => format!("{v:?}"),
            Self::Table(v) => format!("{v:?}"),
        }
    }
}

impl Attribute {
    pub fn to_colored_string(&self) -> String {
        match self {
            Self::Bool(v) => format!("{v:?}").magenta().to_string(),
            Self::String(v) => format!("{v:?}").green().to_string(),
            Self::Integer(v) => format!("{v:?}").red().to_string(),
            Self::Float(v) => format!("{v:?}").yellow().to_string(),
            Self::Date(v) => v.to_string().blue().to_string(),
            Self::Time(v) => v.to_string().blue().to_string(),
            Self::DateTime(v) => v.to_string().blue().to_string(),
            Self::Array(v) => format!(
                "[{}]",
                v.iter()
                    .map(|a| a.to_colored_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::Table(v) => format!(
                "{{{}}}",
                v.iter()
                    .map(|Tuple2(k, v)| format!(
                        "{}={}",
                        k.to_string().blue(),
                        v.to_colored_string()
                    ))
                    .collect::<Vec<String>>()
                    .join(", ")
            )
            .to_string(),
        }
    }

    pub fn type_name(&self) -> &str {
        match self {
            Self::Bool(_) => "Bool",
            Self::String(_) => "String",
            Self::Integer(_) => "Integer",
            Self::Float(_) => "Float",
            Self::Date(_) => "Date",
            Self::Time(_) => "Time",
            Self::DateTime(_) => "DateTime",
            Self::Array(_) => "Array",
            Self::Table(_) => "Table",
        }
    }

    pub fn get_string(&self) -> Option<RStr> {
        match self {
            Self::String(s) => Some(s.as_rstr()),
            _ => None,
        }
    }

    pub fn get_table(&self) -> Option<&AttrMap> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }

    pub fn get_mut_table(&mut self) -> Option<&mut AttrMap> {
        match self {
            Self::Table(ref mut t) => Some(t),
            _ => None,
        }
    }
}

pub trait FromAttribute: Sized {
    fn from_attr(value: &Attribute) -> Option<Self>;
    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        FromAttribute::from_attr(value).ok_or_else(|| {
            format!(
                "Incorrect Type: got {} instead of {}",
                value.type_name(),
                type_name::<Self>()
            )
        })
    }
}

/// Trait to loosely convert attributes from one into another
pub trait FromAttributeRelaxed: Sized {
    fn from_attr_relaxed(value: &Attribute) -> Option<Self> {
        FromAttributeRelaxed::try_from_attr_relaxed(value).ok()
    }
    fn try_from_attr_relaxed(value: &Attribute) -> Result<Self, String>;
}

/// Macro to implement the FromAttribute and FromAttributeRelaxed
///
/// The macro takes the type, primary enum member, and alternative
/// conversions.  The primary enum member will be used to extract the
/// value for FromAttribute, and for FromAttributeRelaxed the primary
/// along with other conversions are used.
macro_rules! impl_from_attr {
    ($t: tt, $x: path, $($y: pat => $e: expr),*) => {
	impl From<$t> for Attribute {
	    fn from(value: $t) -> Self {
		$x(value)
	    }
	}

        impl FromAttribute for $t {
            fn from_attr(value: &Attribute) -> Option<$t> {
                match value {
                    $x(v) => Some(v.clone()),
                    _ => None,
                }
            }
        }

        impl FromAttributeRelaxed for $t {
            fn try_from_attr_relaxed(value: &Attribute) -> Result<$t, String> {
                match value {
                    $x(v) => Ok(v.clone()),
		    $($y => Ok($e),)*
                    _ => Err(format!(
                        "Incorrect Type: `{}` cannot be converted to `{}`",
                        value.type_name(),
			type_name::<Self>()
                    )),
                }
            }
        }
    };
}

/// Get String representation of different types
pub fn type_name<P>() -> String {
    // function returns the full path, but we'll only use the last
    let org = std::any::type_name::<P>();
    let parts = org.split(&[',', '(', ')', '<', '>']);
    let mut name = String::new();
    let mut offset = 0;
    for part in parts {
        name.push_str(part.split("::").last().unwrap_or("_"));
        offset += part.len();
        if offset < org.len() {
            // this part is to reinsert the char we used to split at
            // this location
            name.push_str(&org[offset..(offset + 1)]);
            offset += 1;
        }
    }
    name
}

// impls for standard types used in enum
impl_from_attr!(bool, Attribute::Bool,
		Attribute::Integer(v) => *v != 0,
		Attribute::Float(v) => *v != 0.0,
		Attribute::String(s) => s.is_empty(),
		Attribute::Array(s) => s.is_empty(),
		Attribute::Table(s) => s.is_empty());
impl_from_attr!(RString, Attribute::String,);
impl_from_attr!(i64, Attribute::Integer,
		Attribute::Bool(v) => *v as i64);
impl_from_attr!(f64, Attribute::Float,
		Attribute::Integer(v) => *v as f64,
		Attribute::Bool(v) => *v as i64 as f64);
impl_from_attr!(Date, Attribute::Date,);
impl_from_attr!(Time, Attribute::Time,);
impl_from_attr!(DateTime, Attribute::DateTime,
		Attribute::Date(v) => DateTime::new(v.clone(), Time::default(), None));
impl_from_attr!(AttrMap, Attribute::Table,);

// impl for tuples of different types
macro_rules! tuple_impls {
    ( $($name:ident $gen:ident $ind:expr),+ ) => {
        impl<$($gen: FromAttribute),+> FromAttribute for ($($gen,)+)
        {
	    fn from_attr(value: &Attribute) -> Option<Self> {
		match value {
		    Attribute::Array(a) => {
			$(let $name = FromAttribute::from_attr(
			    a.get($ind)?)?;)+
			Some(($($name,)+))
		    },
		    _ => None
		}
            }

	    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
		match value {
		    Attribute::Array(a) => {
			$(let $name = FromAttribute::try_from_attr(
			    a.get($ind).ok_or("Not enough members".to_string())?)?;)+
			Ok(($($name,)+))
		    },
		    _ => Err(format!(
                        "Incorrect Type: got `{}` instead of `{}`",
                        value.type_name(),
			type_name::<Self>()
                    )),
		}
            }
        }

        impl<$($gen: FromAttributeRelaxed),+> FromAttributeRelaxed for ($($gen,)+)
        {
	    fn try_from_attr_relaxed(value: &Attribute) -> Result<Self, String> {
		match value {
		    Attribute::Array(a) => {
			$(let $name = FromAttributeRelaxed::try_from_attr_relaxed(
			    a.get($ind).ok_or("Not enough members".to_string())?)?;)+
			Ok (($($name,)+))
		    }
		    _ => Err(format!(
                        "Incorrect Type: got {} instead of {}",
                        value.type_name(),
			type_name::<Self>()
                    ))
		}
            }
        }
    };
}

// a A repetition is needed; otherwise it'll throw error due to case
// of generic and identifier needing to be different case; 0-5 numbers
// are used so that we can stop using `${index()}` which is unstable
// #![feature(macro_metavar_expr)]
tuple_impls!(a A 0);
tuple_impls!(a A 0, b B 1);
tuple_impls!(a A 0, b B 1, c C 2);
tuple_impls!(a A 0, b B 1, c C 2, d D 3);
tuple_impls!(a A 0, b B 1, c C 2, d D 3, e E 4);
tuple_impls!(a A 0, b B 1, c C 2, d D 3, e E 4, f F 5);

// impl for popular/useful types not in enum
impl FromAttribute for String {
    fn from_attr(value: &Attribute) -> Option<String> {
        match value {
            Attribute::String(v) => Some(v.to_string()),
            _ => None,
        }
    }
}

impl From<String> for Attribute {
    fn from(value: String) -> Self {
        Self::String(RString::from(value))
    }
}

impl FromAttribute for Attribute {
    fn from_attr(value: &Attribute) -> Option<Attribute> {
        Some(value.clone())
    }
}

impl FromAttribute for u64 {
    fn from_attr(value: &Attribute) -> Option<Self> {
        FromAttribute::try_from_attr(value).ok()
    }
    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        match value {
            Attribute::Integer(v) => u64::try_from(*v).map_err(|e| e.to_string()),
            _ => Err(format!(
                "Incorrect Type: `{}` cannot be converted to `{}`",
                value.type_name(),
                type_name::<Self>()
            )),
        }
    }
}

impl FromAttribute for PathBuf {
    fn from_attr(value: &Attribute) -> Option<PathBuf> {
        match value {
            Attribute::String(v) => Some(PathBuf::from(v.as_str())),
            _ => None,
        }
    }
}

impl FromAttribute for Template {
    fn from_attr(value: &Attribute) -> Option<Self> {
        Template::parse_template(&String::from_attr(value)?).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<Self, String> {
        Template::parse_template(&String::try_from_attr(value)?).map_err(|e| e.to_string())
    }
}

impl<T> From<Vec<T>> for Attribute
where
    Attribute: From<T>,
{
    fn from(value: Vec<T>) -> Self {
        Self::Array(
            value
                .into_iter()
                .map(Attribute::from)
                .collect::<Vec<Attribute>>()
                .into(),
        )
    }
}

impl<T> FromAttribute for Vec<T>
where
    T: FromAttribute,
{
    fn from_attr(value: &Attribute) -> Option<Vec<T>> {
        FromAttribute::try_from_attr(value).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<Vec<T>, String> {
        match value {
            Attribute::Array(v) => v.iter().map(FromAttribute::try_from_attr).collect(),
            _ => Err(format!(
                "Incorrect Type: got {} instead of Array",
                value.type_name()
            )),
        }
    }
}

impl<T> FromAttributeRelaxed for Vec<T>
where
    T: FromAttributeRelaxed,
{
    fn from_attr_relaxed(value: &Attribute) -> Option<Vec<T>> {
        FromAttributeRelaxed::try_from_attr_relaxed(value).ok()
    }

    fn try_from_attr_relaxed(value: &Attribute) -> Result<Vec<T>, String> {
        match value {
            Attribute::Array(v) => v
                .iter()
                .map(FromAttributeRelaxed::try_from_attr_relaxed)
                .collect(),
            _ => Err(format!(
                "Incorrect Type: got {} instead of Array",
                value.type_name()
            )),
        }
    }
}

pub type AttrSlice<'a> = RSlice<'a, Attribute>;
pub type AttrMap = RHashMap<RString, Attribute>;

#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Debug)]
pub struct DateTime {
    pub date: Date,
    pub time: Time,
    pub offset: ROption<Offset>,
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}

impl DateTime {
    pub fn new(date: Date, time: Time, offset: Option<Offset>) -> Self {
        Self {
            date,
            time,
            offset: offset.into(),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Debug)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

impl Date {
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        // TODO check valid dates
        Self { year, month, day }
    }

    pub fn with_time(self, time: Time) -> DateTime {
        DateTime {
            date: self,
            time,
            offset: RNone,
        }
    }

    pub fn doy(&self) -> u8 {
        let ly = Date::leap_year(self.year);
        let mut doy = 0;
        for m in 1..(self.month) {
            doy += Date::days_in_month(m, ly);
        }
        doy + self.day
    }

    pub fn leap_year(year: u16) -> bool {
        (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))
    }

    pub fn days_in_month(month: u8, leap_year: bool) -> u8 {
        match month {
            2 if leap_year => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Debug)]
pub struct Time {
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    pub nanosecond: u32,
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.min, self.sec)
    }
}

impl Time {
    pub fn new(hour: u8, min: u8, sec: u8, nanosecond: u32) -> Self {
        // TODO check valid time
        Self {
            hour,
            min,
            sec,
            nanosecond,
        }
    }

    pub fn seconds_since_midnight(&self) -> u32 {
        (self.hour as u32 * 60 + self.min as u32) * 60 + self.sec as u32
    }

    pub fn from_seconds_since_midnight(secs: u32) -> Self {
        let sec = secs % 60;
        let mins = (secs - sec) / 60;
        let min = mins % 60;
        let hour = (mins - min) / 60;
        Self {
            hour: hour as u8,
            min: min as u8,
            sec: sec as u8,
            nanosecond: 0,
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Debug)]
pub struct Offset {
    pub hour: u8,
    pub min: u8,
}

pub fn parse_attr_file(txt: &str) -> anyhow::Result<AttrMap> {
    let mut attrs = AttrMap::new();
    let (rest, (grp, grp_attrs, parts)) = attr_file(txt)
            .map_err(|e| anyhow::Error::msg(e.to_string()))// .map_err(|e| e.to_owned())
	?;
    if !rest.is_empty() {
        println!("{rest}");
        return Err(anyhow::Error::msg("Cannot parse the attr file completely."));
    }
    let mut curr_map: &mut AttrMap = &mut attrs;
    if let Some(grp) = grp {
        for g in grp {
            if !curr_map.contains_key(g) {
                curr_map.insert(g.into(), Attribute::Table(AttrMap::new()));
            }
            curr_map = curr_map
                .get_mut(g)
                .expect("Either the key should be there, or inserted above")
                .get_mut_table()
                .context("The Key is not empty or a table")?;
        }
    }

    for (k, v) in grp_attrs {
        curr_map.insert(k, v);
    }

    for (grp, grp_attrs) in parts {
        for g in grp {
            if !curr_map.contains_key(g) {
                curr_map.insert(g.into(), Attribute::Table(AttrMap::new()));
            }
            curr_map = curr_map.get_mut(g).unwrap().get_mut_table().unwrap();
        }
        for (k, v) in grp_attrs {
            curr_map.insert(k, v);
        }
    }
    Ok(attrs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    fn from_attr_test() {
        let val: bool = FromAttribute::from_attr(&Attribute::Bool(true)).unwrap();
        assert!(val);
        let val: bool = FromAttribute::from_attr(&Attribute::Bool(false)).unwrap();
        assert!(!val);
        assert!(i64::from_attr(&Attribute::Bool(false)).is_none());
        let val: i64 = FromAttribute::from_attr(&Attribute::Integer(2)).unwrap();
        assert_eq!(val, 2);
        let _: bool = FromAttribute::from_attr(&Attribute::Bool(true)).unwrap();

        let val: (i64, bool) = FromAttribute::from_attr(&Attribute::Array(
            vec![Attribute::Integer(2), Attribute::Bool(true)].into(),
        ))
        .unwrap();
        assert_eq!(val, (2, true));
    }

    #[rstest]
    fn try_from_attr_test() {
        let val: bool = FromAttribute::try_from_attr(&Attribute::Bool(true)).unwrap();
        assert!(val);
        let val: bool = FromAttribute::try_from_attr(&Attribute::Bool(false)).unwrap();
        assert!(!val);
        assert!(i64::try_from_attr(&Attribute::Bool(false)).is_err());
        let val: i64 = FromAttribute::try_from_attr(&Attribute::Integer(2)).unwrap();
        assert_eq!(val, 2);
        let val: bool = FromAttribute::try_from_attr(&Attribute::Bool(true)).unwrap();
        assert!(val);
        let val: (i64, bool) = FromAttribute::try_from_attr(&Attribute::Array(
            vec![Attribute::Integer(2), Attribute::Bool(true)].into(),
        ))
        .unwrap();
        assert_eq!(val, (2, true));

        let val: (Template, bool) = FromAttribute::try_from_attr(&Attribute::Array(
            vec![Attribute::String("2 {name}".into()), Attribute::Bool(true)].into(),
        ))
        .unwrap();
        assert_eq!(val.0.original(), "2 {name}");
    }

    #[rstest]
    fn try_from_attr_relaxed_test() {
        let val: bool =
            FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Bool(true)).unwrap();
        assert!(val);
        let val: bool =
            FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Bool(false)).unwrap();
        assert!(!val);
        let val: bool =
            FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Integer(2)).unwrap();
        assert!(val);
        let val: i64 =
            FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Bool(false)).unwrap();
        assert_eq!(val, 0);
        let val: i64 = FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Bool(true)).unwrap();
        assert_eq!(val, 1);
        let val: i64 = FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Integer(2)).unwrap();
        assert_eq!(val, 2);
        let val: bool =
            FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Bool(true)).unwrap();
        assert!(val);
        let val: (i64, bool) = FromAttributeRelaxed::try_from_attr_relaxed(&Attribute::Array(
            vec![Attribute::Integer(2), Attribute::Integer(1)].into(),
        ))
        .unwrap();
        assert_eq!(val, (2, true));
    }
}
