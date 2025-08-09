use crate::expressions::EvalError;
use crate::valid_var;
use abi_stable::{
    std_types::{
        RHashMap,
        ROption::{self, RNone},
        RSlice, RStr, RString, RVec, Tuple2,
    },
    StableAbi,
};
use colored::Colorize;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use string_template_plus::{Render, RenderOptions, Template};

#[cfg(feature = "chrono")]
use abi_stable::std_types::RSome;

#[cfg(feature = "chrono")]
use chrono::{Datelike, Timelike};

static DEFAULT_ATTR: Attribute = Attribute::Bool(true);

/// For anything that can store attributes in a map
///
/// This trait helps us implement same things for attribute storage by
/// simply pointing to the variable that stores the attribute
/// map. Components like node and network can implement this.  This
/// means we don't have to manually implement functionality like
/// rendering the string template on each one of them.
pub trait HasAttributes {
    /// If a node, return its name
    ///
    /// Network and attrmaps can ignore this. Useful to generate error
    /// messages.
    fn node_name(&self) -> Option<&str> {
        None
    }

    /// Reference to the [`HashMap`] of the attributes
    fn attr_map(&self) -> &AttrMap;

    /// Mutable reference to the [`HashMap`] of the attributes
    fn attr_map_mut(&mut self) -> &mut AttrMap;

    /// Get the reference to the attribute by name
    ///
    /// `_` is a dummy attribute that is always returned as `true`
    fn attr(&self, name: &str) -> Option<&Attribute> {
        if name == "_" {
            // always available
            Some(&DEFAULT_ATTR)
        } else {
            self.attr_map().get(name)
        }
    }

    /// Get mutable reference to the attribute by name
    fn attr_mut(&mut self, name: &str) -> Option<&mut Attribute> {
        self.attr_map_mut().get_mut(name)
    }

    /// Delete the attribute by name
    ///
    /// You can not delete the dummy attribute `_`, it'll be silently
    /// ignored
    fn del_attr(&mut self, name: &str) -> Option<Attribute> {
        if name == "_" {
            // ignore delete
            return None;
        }
        self.attr_map_mut().remove(name).into()
    }

    /// Set attribute to a value
    ///
    /// Will return the old value if that attribute is already present.
    fn set_attr(&mut self, name: &str, val: Attribute) -> Option<Attribute> {
        if name == "_" {
            // cannot be set, it's also for discarding results
            return None;
        }
        if let Some(v) = self.attr(name) {
            if v == &val {
                return None;
            }
        }
        self.attr_map_mut().insert(name.into(), val).into()
    }

    /// Get nested attribute to a value by `.` joined name
    fn attr_dot(&self, names: &str) -> Result<Option<&Attribute>, String> {
        match names.rsplit_once(".") {
            Some((pre, name)) => self.attr_nested(
                &pre.split(".").map(String::from).collect::<Vec<String>>(),
                name,
            ),
            None => Ok(self.attr(names)),
        }
    }

    /// Get nested attribute by name and nested map names
    fn attr_nested(&self, prefix: &[String], name: &str) -> Result<Option<&Attribute>, String> {
        let mut map = self.attr_map();
        for m in prefix {
            map = match map.attr(m) {
                Some(Attribute::Table(mp)) => mp,
                Some(_) => return Err(format!("Key {m} is not a Table")),
                None => return Err(format!("Key {m} not found")),
            };
        }
        Ok(map.attr(name))
    }

    /// Set nested attributes by `.` joined name
    fn set_attr_dot(&mut self, names: &str, val: Attribute) -> Result<Option<Attribute>, String> {
        match names.rsplit_once(".") {
            Some((pre, name)) => self.set_attr_nested(
                &pre.split(".").map(String::from).collect::<Vec<String>>(),
                name,
                val,
            ),
            None => Ok(self.set_attr(names, val)),
        }
    }

    /// Set nested attribute by name and nested map names
    fn set_attr_nested(
        &mut self,
        prefix: &[String],
        name: &str,
        val: Attribute,
    ) -> Result<Option<Attribute>, String> {
        let mut map = self.attr_map_mut();
        for m in prefix {
            map = match map
                .entry(m.to_string().into())
                .or_insert(Attribute::Table(AttrMap::new()))
            {
                Attribute::Table(ref mut mp) => mp,
                _ => return Err(format!("Key {m} is not a Table")),
            };
        }
        Ok(map.set_attr(name, val))
    }

    /// Delete nested attribute by `.` joined name
    fn del_attr_dot(&mut self, names: &str) -> Result<Option<Attribute>, String> {
        match names.rsplit_once(".") {
            Some((pre, name)) => self.del_attr_nested(
                &pre.split(".").map(String::from).collect::<Vec<String>>(),
                name,
            ),
            None => Ok(self.del_attr(names)),
        }
    }

    /// Delete nested attribute by name and nested map names
    fn del_attr_nested(
        &mut self,
        prefix: &[String],
        name: &str,
    ) -> Result<Option<Attribute>, String> {
        let mut map = self.attr_map_mut();
        for m in prefix {
            map = match map
                .entry(m.to_string().into())
                .or_insert(Attribute::Table(AttrMap::new()))
            {
                Attribute::Table(ref mut mp) => mp,
                _ => return Err(format!("Key {m} is not a Table")),
            };
        }
        Ok(map.del_attr(name))
    }

    /// Get attribute into any generic type that implements [`FromAttribute`]
    fn try_attr<T: FromAttribute>(&self, name: &str) -> Result<T, String> {
        match self.attr_dot(name)? {
            Some(v) => FromAttribute::try_from_attr(v),
            None => Err(format!(
                "Attribute Error: Attribute {name} not found in Node"
            )),
        }
    }

    /// Get attribute into any generic type that implements [`FromAttributeRelaxed`]
    fn try_attr_relaxed<T: FromAttributeRelaxed>(&self, name: &str) -> Result<T, String> {
        match self.attr_dot(name)? {
            Some(v) => FromAttributeRelaxed::try_from_attr_relaxed(v),
            None => Err(format!(
                "Attribute Error: Attribute {name} not found in Node"
            )),
        }
    }

    /// Render the given template using the attribute values
    ///
    /// The attributes will be available to be used in the template
    /// based on the following rules:
    /// - String attributes will be quoted, extra variable with `_`
    ///   prefix will be available to use unquoted string variables,
    /// - nested variables will be available using the `.` separator
    /// - all other variables will be available with their name,
    ///   their value will be their string representation.
    fn render(&self, template: &Template) -> anyhow::Result<String> {
        let mut op = RenderOptions::default();
        let used_vars = template.parts().iter().flat_map(|p| p.variables());
        for var in used_vars {
            if let Some(val) = self.attr(var) {
                op.variables.insert(var.to_string(), val.to_string());
            }
            if let Some((pre, name)) = var.rsplit_once(".") {
                let pre: Vec<String> = pre.split(".").map(|s| s.to_string()).collect();
                if let Ok(Some(val)) = self.attr_nested(&pre, name) {
                    op.variables.insert(var.to_string(), val.to_string());
                }
            }
            if let Some(val) = var.strip_prefix('_') {
                if let Some(Attribute::String(s)) = self.attr(val) {
                    op.variables.insert(var.to_string(), s.to_string());
                }
                if let Some((pre, name)) = val.rsplit_once(".") {
                    let pre: Vec<String> = pre.split(".").map(|s| s.to_string()).collect();
                    if let Ok(Some(Attribute::String(val))) = self.attr_nested(&pre, name) {
                        op.variables.insert(var.to_string(), val.to_string());
                    }
                }
            }
        }
        template.render(&op)
    }
}

/// This Blanket Implementation helps us use the functions recursively
impl HasAttributes for AttrMap {
    fn attr_map(&self) -> &AttrMap {
        self
    }
    fn attr_map_mut(&mut self) -> &mut AttrMap {
        self
    }
}

/// Generic attribute value for nadi system
///
/// Attribute implements various operation traits that can provide
/// functions like logical bitand/bitor, or arithmetic add/multiply
/// etc depending on what variable attribute contains.
///
/// It also implements type conversions from/to various rust native
/// and external crate formats. You can also use [`convert_impls`]
/// macro to generate implementation from [`Attribute`] if you know
/// your type has `From<T>` for a type that implements
/// [`FromAttribute`].
#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum Attribute {
    /// Boolean value (`true` or `false`)
    Bool(bool),
    /// String value (Double quoted `"`)
    String(RString),
    /// Integer value
    Integer(i64),
    /// Float value (supports `nan`, `inf`, `-inf`)
    Float(f64),
    /// Date value with year, month, day
    Date(Date),
    /// Time value with hour, minute, second
    Time(Time),
    /// Date and Time value
    DateTime(DateTime),
    /// Array/List of [`Attribute`]s
    Array(RVec<Attribute>),
    /// HashMap of [`Attribute`]s by name
    Table(AttrMap),
}

impl Default for Attribute {
    fn default() -> Self {
        Self::Bool(false)
    }
}

impl std::fmt::Display for Attribute {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Bool(v) => write!(f, "{v}"),
            Self::String(v) => write!(f, "{v:?}"),
            Self::Integer(v) => write!(f, "{v}"),
            // lower for nan and inf
            Self::Float(v) => write!(f, "{}", v.to_string().to_lowercase()),
            Self::Date(v) => write!(f, "{v}"),
            Self::Time(v) => write!(f, "{v}"),
            Self::DateTime(v) => write!(f, "{v}"),
            Self::Array(v) => {
                write!(
                    f,
                    "[{}]",
                    v.iter()
                        .map(|a| a.to_string())
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
            Self::Table(v) => {
                write!(
                    f,
                    "{{{}}}",
                    v.iter()
                        .map(|a| {
                            if valid_var(a.0) {
                                format!("{} = {}", a.0, a.1.to_string())
                            } else {
                                format!("\"{}\" = {}", a.0, a.1.to_string())
                            }
                        })
                        .collect::<Vec<String>>()
                        .join(", ")
                )
            }
        }
    }
}

/// Check if the given string is a valid variable name
///
/// The `manual` in name means we are checking it manually with a
/// logic here, instead of using the parser to verify it is
/// successful. Use the [`parser::tokenizer::valid_variable_name`] if
/// `parser` feature is activated.
pub fn valid_var_manual(n: &str) -> bool {
    let mut chars = n.chars();
    match chars.next() {
        Some('_') => (),
        Some(c) => {
            if !c.is_alphabetic() {
                return false;
            }
        }
        None => return true,
    }
    chars.all(|c| c == '_' || c.is_alphanumeric())
}

impl PartialOrd for Attribute {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        match self {
            Self::Bool(b) => {
                if let Self::Bool(v) = other {
                    return b.partial_cmp(v);
                }
            }
            Self::String(b) => {
                if let Self::String(v) = other {
                    return b.partial_cmp(v);
                }
            }
            // FIXME: PartialEq is derived and doesn't have this
            Self::Integer(b) => {
                return match other {
                    Self::Integer(v) => b.partial_cmp(v),
                    Self::Float(v) => (*b as f64).partial_cmp(v),
                    _ => None,
                };
            }
            Self::Float(b) => {
                return match other {
                    Self::Float(v) => b.partial_cmp(v),
                    Self::Integer(v) => b.partial_cmp(&(*v as f64)),
                    _ => None,
                };
            }
            Self::Date(b) => {
                return match other {
                    Self::Date(v) => b.partial_cmp(v),
                    Self::DateTime(v) => b.partial_cmp(v),
                    _ => None,
                };
            }
            Self::Time(b) => {
                if let Self::Time(v) = other {
                    return b.partial_cmp(v);
                }
            }
            Self::DateTime(b) => {
                return match other {
                    Self::DateTime(v) => b.partial_cmp(v),
                    Self::Date(v) => b.partial_cmp(v),
                    _ => None,
                };
            }
            _ => (),
        };
        None
    }
}

impl std::ops::Not for Attribute {
    type Output = Result<Self, EvalError>;

    fn not(self) -> Self::Output {
        match self {
            Self::Bool(b) => Ok(Self::Bool(!b)),
            Self::Array(ar) => Ok(Self::Array(
                ar.into_iter()
                    .map(|a| std::ops::Not::not(a))
                    .collect::<Result<Vec<Self>, EvalError>>()?
                    .into(),
            )),
            _ => Err(EvalError::NotABool),
        }
    }
}

impl std::ops::Neg for Attribute {
    type Output = Result<Self, EvalError>;

    fn neg(self) -> Self::Output {
        match self {
            Self::Integer(v) => Ok(Self::Integer(-v)),
            Self::Float(v) => Ok(Self::Float(-v)),
            Self::Array(ar) => Ok(Self::Array(
                ar.into_iter()
                    .map(|a| std::ops::Neg::neg(a))
                    .collect::<Result<Vec<Self>, EvalError>>()?
                    .into(),
            )),
            _ => Err(EvalError::NotANumber),
        }
    }
}

/// Implement the operations for array of values
macro_rules! array_impl {
    ($a:ident, $oth:ident, $imp:path) => {
        match $oth {
            Self::Array(ar2) => {
                if $a.len() != ar2.len() {
                    Err(EvalError::DifferentLength($a.len(), ar2.len()))
                } else {
                    Ok(Self::Array(
                        $a.into_iter()
                            .zip(ar2)
                            .map(|(a, b)| $imp(a, b))
                            .collect::<Result<Vec<Self>, EvalError>>()?
                            .into(),
                    ))
                }
            }
            o => Ok(Self::Array(
                $a.into_iter()
                    .map(|a| $imp(a, o.clone()))
                    .collect::<Result<Vec<Self>, EvalError>>()?
                    .into(),
            )),
        }
    };
}

impl std::ops::BitAnd for Attribute {
    type Output = Result<Self, EvalError>;

    fn bitand(self, other: Self) -> Self::Output {
        match self {
            Self::Bool(a) => match other {
                Self::Bool(b) => Ok(Self::Bool(a & b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::BitAnd::bitand(Self::Bool(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::NotABool),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::BitAnd::bitand),
            _ => Err(EvalError::NotABool),
        }
    }
}

impl std::ops::BitOr for Attribute {
    type Output = Result<Self, EvalError>;

    fn bitor(self, other: Self) -> Self::Output {
        match self {
            Self::Bool(a) => match other {
                Self::Bool(b) => Ok(Self::Bool(a | b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::BitOr::bitor(Self::Bool(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::NotABool),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::BitOr::bitor),
            _ => Err(EvalError::NotABool),
        }
    }
}

// find a way to write macro for this?
impl std::ops::Add for Attribute {
    type Output = Result<Self, EvalError>;

    fn add(self, other: Self) -> Self::Output {
        // todo date/time + integer
        match self {
            Self::Integer(a) => match other {
                Self::Integer(b) => Ok(Self::Integer(a + b)),
                Self::Float(b) => Ok(Self::Float(a as f64 + b)),
                Self::Array(ar) => std::ops::Add::add(Self::Array(ar), Self::Integer(a)),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Float(a) => match other {
                Self::Integer(b) => Ok(Self::Float(a + b as f64)),
                Self::Float(b) => Ok(Self::Float(a + b)),
                Self::Array(ar) => std::ops::Add::add(Self::Array(ar), Self::Float(a)),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::Add::add),
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

impl std::ops::Sub for Attribute {
    type Output = Result<Self, EvalError>;

    fn sub(self, other: Self) -> Self::Output {
        // todo date/time - integer
        match self {
            Self::Integer(a) => match other {
                Self::Integer(b) => Ok(Self::Integer(a - b)),
                Self::Float(b) => Ok(Self::Float(a as f64 - b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::Sub::sub(Self::Integer(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Float(a) => match other {
                Self::Integer(b) => Ok(Self::Float(a - b as f64)),
                Self::Float(b) => Ok(Self::Float(a - b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::Sub::sub(Self::Float(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::Sub::sub),
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

impl std::ops::Mul for Attribute {
    type Output = Result<Self, EvalError>;

    fn mul(self, other: Self) -> Self::Output {
        match self {
            Self::Integer(a) => match other {
                Self::Integer(b) => Ok(Self::Integer(a * b)),
                Self::Float(b) => Ok(Self::Float(a as f64 * b)),
                Self::Array(ar) => std::ops::Mul::mul(Self::Array(ar), Self::Integer(a)),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Float(a) => match other {
                Self::Integer(b) => Ok(Self::Float(a * b as f64)),
                Self::Float(b) => Ok(Self::Float(a * b)),
                Self::Array(ar) => std::ops::Mul::mul(Self::Array(ar), Self::Float(a)),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::Mul::mul),
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

impl std::ops::Div for Attribute {
    type Output = Result<Self, EvalError>;

    fn div(self, other: Self) -> Self::Output {
        match self {
            Self::Integer(a) => match other {
                // doing integer div by default might be confusing to people
                Self::Integer(b) => Ok(Self::Float(a as f64 / b as f64)),
                Self::Float(b) => Ok(Self::Float(a as f64 / b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::Div::div(Self::Integer(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Float(a) => match other {
                Self::Integer(b) => Ok(Self::Float(a / b as f64)),
                Self::Float(b) => Ok(Self::Float(a / b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::Div::div(Self::Float(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::Div::div),
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

impl std::ops::Rem for Attribute {
    type Output = Result<Self, EvalError>;

    fn rem(self, other: Self) -> Self::Output {
        match self {
            Self::Integer(a) => match other {
                Self::Integer(b) => Ok(Self::Integer(a % b)),
                Self::Array(ar) => Ok(Self::Array(
                    ar.into_iter()
                        .map(|b| std::ops::Rem::rem(Self::Integer(a), b))
                        .collect::<Result<Vec<Self>, EvalError>>()?
                        .into(),
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            Self::Array(ar) => array_impl!(ar, other, std::ops::Rem::rem),
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

impl Attribute {
    /// Convert the attribute into a valid JSON [`String`]
    pub fn to_json(&self) -> String {
        match self {
            Self::Date(v) => format!("\"{v}\""),
            Self::Time(v) => format!("\"{v}\""),
            Self::DateTime(v) => format!("\"{v}\""),
            Self::Array(v) => format!(
                "[{}]",
                v.iter()
                    .map(|a| a.to_json())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::Table(v) => format!(
                "{{{}}}",
                v.iter()
                    .map(|Tuple2(k, v)| format!("\"{}\": {}", k, v.to_json()))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            v => v.to_string(),
        }
    }

    #[deprecated(since = "0.7.1", note = "please use `self.to_string()` instead")]
    pub fn to_toml_string(&self) -> String {
        self.to_string()
    }

    /// Get a string with terminal coloring inside it
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

    /// Get the name of the type
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

    /// If it is a string get the reference
    pub fn get_string(&self) -> Option<RStr> {
        match self {
            Self::String(s) => Some(s.as_rstr()),
            _ => None,
        }
    }

    /// If it is a table get the reference
    pub fn get_table(&self) -> Option<&AttrMap> {
        match self {
            Self::Table(t) => Some(t),
            _ => None,
        }
    }

    /// If it is a table get the mutable reference
    pub fn get_mut_table(&mut self) -> Option<&mut AttrMap> {
        match self {
            Self::Table(ref mut t) => Some(t),
            _ => None,
        }
    }

    /// Integer division (both have to be integers)
    pub fn int_div(&self, other: &Self) -> Result<Self, EvalError> {
        match self {
            Self::Integer(a) => match other {
                Self::Integer(b) => Ok(Self::Integer(
                    a.checked_div(*b).ok_or(EvalError::DivideByZero)?,
                )),
                _ => Err(EvalError::InvalidOperation),
            },
            _ => Err(EvalError::InvalidOperation),
        }
    }

    /// Check if the value contains other
    ///
    /// Only valid for String, Array and Table,
    /// - It checks for substring for String (other cannot be Array/Table),
    /// - It checks for attribute value for Array (other can be any Attribute),
    /// - It checks for Key of HashMap for Table (other should be String),
    pub fn contains(&self, other: &Self) -> Result<bool, EvalError> {
        match self {
            Self::String(st) => match other {
                Self::String(s) => Ok(st.contains(s.as_str())),
                Self::Array(_) => Err(EvalError::InvalidOperation),
                Self::Table(_) => Err(EvalError::InvalidOperation),
                a => Ok(st.contains(a.to_string().as_str())),
            },
            Self::Array(ar) => Ok(ar.iter().any(|v| v == other)),
            Self::Table(am) => match other {
                Self::String(s) => Ok(am.contains_key(s)),
                _ => Err(EvalError::InvalidOperation),
            },
            _ => Err(EvalError::InvalidOperation),
        }
    }

    /// Checks if it matches a Regex pattern
    ///
    /// the self and other value should both be strings
    pub fn str_match(&self, other: &Self) -> Result<bool, EvalError> {
        match self {
            Self::String(st) => match other {
                Self::String(s) => regex::Regex::new(s.as_str())
                    .map(|p| p.is_match(st.as_str()))
                    .map_err(EvalError::RegexError),
                _ => Err(EvalError::InvalidOperation),
            },
            _ => Err(EvalError::InvalidOperation),
        }
    }
}

/// Trait to convert values from [`Attribute`] into target type
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

/// Trait to loosely convert [`Attribute`] into target type
///
/// Loosely or Related here means that if it makes sense for the type
/// to be converted to another even if it's not represented internally
/// as such. For example, integer attribute can be read as string, or
/// float.
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
///
/// It uses [`std::any::type_name`] internally and only uses the type
/// name instead of the whole path.
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
		Attribute::String(s) => !s.is_empty(),
		Attribute::Array(s) => !s.is_empty(),
		Attribute::Table(s) => !s.is_empty());
impl_from_attr!(RString, Attribute::String,
        Attribute::Bool(s) => s.to_string().into(),
        Attribute::Integer(s) => s.to_string().into(),
        Attribute::Float(s) => s.to_string().to_lowercase().into(),
        Attribute::Date(s) => s.to_string().into(),
        Attribute::Time(s) => s.to_string().into(),
        Attribute::DateTime(s) => s.to_string().into());
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

/// impl for tuples of different types
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

/// Macro to generate attribute value with different patterns.
///
/// It uses (val) for single attribute, [val,...] for array attribute
/// and [key => val,...] for attrmap attribute
#[macro_export]
macro_rules! attr {
    ($val:expr) => {
	::nadi_core::attrs::Attribute::from($val)
    };
    [ $($val:expr),* $(,)? ] => {
	::nadi_core::attrs::Attribute::Array(
	    vec![
		$(::nadi_core::attrs::Attribute::from($val),)+
	    ].into()
	)
    };
    [ $($key:ident => $val:expr),+ $(,)? ] => {
	::nadi_core::attrs::AttrMap::from(
	    std::collections::HashMap::from([$(
		(::nadi_core::abi_stable::std_types::RString::from(stringify!($key)), ::nadi_core::attrs::Attribute::from($val)),
	    )+
	    ])
	)
    };
}

// TODO: in the next version, remove the bottom two.

/// Macro to create a Array from list of values
#[macro_export]
macro_rules! attr_array {
    ( $($val:expr),+ ) => {
	::nadi_core::attrs::Attribute::Array(
	    vec![
		$(::nadi_core::attrs::Attribute::from($val),)+
	    ].into()
	)
    }
}

/// Macro to create a AttrMap from key, value pairs
#[macro_export]
macro_rules! attr_map {
    ( $($key:ident => $val:expr),+ ) => {
	::nadi_core::attrs::AttrMap::from(
	    std::collections::HashMap::from([$(
		(::nadi_core::abi_stable::std_types::RString::from(stringify!($key)), ::nadi_core::attrs::Attribute::from($val)),
	    )+
	    ])
	)
    }
}

impl From<usize> for Attribute {
    fn from(value: usize) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<i32> for Attribute {
    fn from(value: i32) -> Self {
        Self::Integer(value as i64)
    }
}

impl From<f32> for Attribute {
    fn from(value: f32) -> Self {
        Self::Float(value as f64)
    }
}

impl From<&str> for Attribute {
    fn from(value: &str) -> Self {
        Self::String(RString::from(value))
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

/// impl for different types that can be converted from ones that has
/// FromAttribute. Can't do this automatically because there will be
/// duplicate implementation
#[macro_export]
macro_rules! convert_impls {
    ($src: tt => $dest: tt) => {
        impl FromAttribute for $dest {
            fn from_attr(value: &Attribute) -> Option<Self> {
                FromAttribute::try_from_attr(value).ok()
            }
            fn try_from_attr(value: &Attribute) -> Result<Self, String> {
                let val: $src = FromAttribute::try_from_attr(value)?;
                $dest::try_from(val).map_err(|e| e.to_string())
            }
        }

        impl FromAttributeRelaxed for $dest {
            fn try_from_attr_relaxed(value: &Attribute) -> Result<Self, String> {
                let val: $src = FromAttributeRelaxed::try_from_attr_relaxed(value)?;
                $dest::try_from(val).map_err(|e| e.to_string())
            }
        }
    };
}

convert_impls!(i64 => i32);
convert_impls!(i64 => u32);
convert_impls!(i64 => u64);
convert_impls!(i64 => usize);
convert_impls!(RString => String);
// since we have String now, we can use that to convert to others
convert_impls!(String => PathBuf);
convert_impls!(String => Regex);

// TODO impl try_from for String => Template in string_template crate
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

impl<T, const N: usize> From<[T; N]> for Attribute
where
    Attribute: From<T>,
{
    fn from(value: [T; N]) -> Self {
        Self::Array(
            value
                .into_iter()
                .map(Attribute::from)
                .collect::<Vec<Attribute>>()
                .into(),
        )
    }
}

impl<T, const N: usize> From<&[T; N]> for Attribute
where
    Attribute: From<T>,
    T: Clone,
{
    fn from(value: &[T; N]) -> Self {
        Self::Array(
            value
                .iter()
                .map(|a| Attribute::from(a.clone()))
                .collect::<Vec<Attribute>>()
                .into(),
        )
    }
}

impl<U, T> From<HashMap<U, T>> for Attribute
where
    Attribute: From<T>,
    RString: From<U>,
{
    fn from(value: HashMap<U, T>) -> Self {
        Self::Table(
            value
                .into_iter()
                .map(|(k, v)| (RString::from(k), Attribute::from(v)))
                .collect::<HashMap<RString, Attribute>>()
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

impl<T> FromAttribute for HashMap<String, T>
where
    T: FromAttribute,
{
    fn from_attr(value: &Attribute) -> Option<HashMap<String, T>> {
        FromAttribute::try_from_attr(value).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<HashMap<String, T>, String> {
        match value {
            Attribute::Table(t) => t
                .iter()
                .map(|Tuple2(k, v)| FromAttribute::try_from_attr(v).map(|v| (k.to_string(), v)))
                .collect(),
            _ => Err(format!(
                "Incorrect Type: got {} instead of Array",
                value.type_name()
            )),
        }
    }
}

impl<T> FromAttribute for HashSet<T>
where
    T: FromAttribute + std::hash::Hash + std::cmp::Eq,
{
    fn from_attr(value: &Attribute) -> Option<HashSet<T>> {
        FromAttribute::try_from_attr(value).ok()
    }

    fn try_from_attr(value: &Attribute) -> Result<HashSet<T>, String> {
        match value {
            Attribute::Array(t) => t.iter().map(|v| FromAttribute::try_from_attr(v)).collect(),
            _ => Err(format!(
                "Incorrect Type: got {} instead of Array",
                value.type_name()
            )),
        }
    }
}

/// Slice of Attributes
pub type AttrSlice<'a> = RSlice<'a, Attribute>;
/// Map of [`Attribute`]s by their name
pub type AttrMap = RHashMap<RString, Attribute>;

/// Datetime with [`Date`], [`Time`] and [`Offset`]
#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Debug)]
pub struct DateTime {
    /// date part of datetime
    pub date: Date,
    /// time part of datetime
    pub time: Time,
    /// offset from GMT (currently not used)
    pub offset: ROption<Offset>,
}

impl PartialOrd for DateTime {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        // TODO don't ignore offset
        Some(self.date.cmp(&other.date).then(self.time.cmp(&other.time)))
    }
}

impl PartialEq<Date> for DateTime {
    fn eq(&self, other: &Date) -> bool {
        // TODO don't ignore offset
        self.date.eq(other) && self.time.eq(&Time::default())
    }
}

impl PartialOrd<Date> for DateTime {
    fn partial_cmp(&self, other: &Date) -> Option<std::cmp::Ordering> {
        // TODO don't ignore offset
        Some(self.date.cmp(other).then(self.time.cmp(&Time::default())))
    }
}

impl std::fmt::Display for DateTime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}", self.date, self.time)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDateTime> for DateTime {
    fn from(value: chrono::NaiveDateTime) -> Self {
        Date::from(value.date()).with_time(Time::from(value.time()))
    }
}

#[cfg(feature = "chrono")]
impl From<DateTime> for chrono::NaiveDateTime {
    fn from(val: DateTime) -> Self {
        let d: chrono::NaiveDate = val.date.into();
        let t: chrono::NaiveTime = val.time.into();
        chrono::NaiveDateTime::new(d, t)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::DateTime<chrono::FixedOffset>> for DateTime {
    fn from(value: chrono::DateTime<chrono::FixedOffset>) -> Self {
        Self::new(
            Date::from(value.date_naive()),
            Time::from(value.time()),
            Some(Offset::from(*value.offset())),
        )
    }
}

#[cfg(feature = "chrono")]
impl From<DateTime> for chrono::DateTime<chrono::FixedOffset> {
    fn from(val: DateTime) -> Self {
        let d: chrono::NaiveDate = val.date.into();
        let t: chrono::NaiveTime = val.time.into();
        if let RSome(offset) = val.offset {
            let o: chrono::FixedOffset = offset.into();
            chrono::NaiveDateTime::new(d, t)
                .and_local_timezone(o)
                .single()
                .expect("Offset should be valid")
        } else {
            chrono::NaiveDateTime::new(d, t).and_utc().fixed_offset()
        }
    }
}

impl DateTime {
    /// new datetime with given date, time and offset
    pub fn new(date: Date, time: Time, offset: Option<Offset>) -> Self {
        Self {
            date,
            time,
            offset: offset.into(),
        }
    }
}

/// Date with year, month and day
#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Date {
    pub year: u16,
    pub month: u8,
    pub day: u8,
}

impl PartialEq<DateTime> for Date {
    fn eq(&self, other: &DateTime) -> bool {
        // TODO don't ignore offset
        other.eq(self)
    }
}

impl PartialOrd<DateTime> for Date {
    fn partial_cmp(&self, other: &DateTime) -> Option<std::cmp::Ordering> {
        // TODO don't ignore offset
        other.partial_cmp(self).map(|o| o.reverse())
    }
}

// impl PartialOrd for Date {
//     fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
//         Some(self.cmp(other))
//     }
// }

// impl Ord for Date {
//     fn cmp(&self, other: &Self) -> std::cmp::Ordering {
//         self.year
//             .cmp(&other.year)
//             .then(self.month.cmp(&other.month))
//             .then(self.day.cmp(&other.day))
//     }
// }

impl std::fmt::Display for Date {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}-{:02}-{:02}", self.year, self.month, self.day)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveDate> for Date {
    fn from(value: chrono::NaiveDate) -> Self {
        Self::new(value.year() as u16, value.month() as u8, value.day() as u8)
    }
}

#[cfg(feature = "chrono")]
impl From<Date> for chrono::NaiveDate {
    fn from(val: Date) -> Self {
        chrono::NaiveDate::from_ymd_opt(val.year as i32, val.month as u32, val.day as u32)
            .expect("should be valid date")
    }
}

impl Date {
    /// new unchecked date with year, month and day
    pub fn new(year: u16, month: u8, day: u8) -> Self {
        // TODO check valid dates
        Self { year, month, day }
    }

    /// Add time to the date and make [`DateTime`]
    pub fn with_time(self, time: Time) -> DateTime {
        DateTime {
            date: self,
            time,
            offset: RNone,
        }
    }

    /// get day of the year
    pub fn doy(&self) -> u8 {
        let ly = Date::leap_year(self.year);
        let mut doy = 0;
        for m in 1..(self.month) {
            doy += Date::days_in_month(m, ly);
        }
        doy + self.day
    }

    /// get if the year is leap year or not
    pub fn leap_year(year: u16) -> bool {
        (year % 4 == 0) && ((year % 100 != 0) || (year % 400 == 0))
    }

    /// Get days in the month
    pub fn days_in_month(month: u8, leap_year: bool) -> u8 {
        match month {
            2 if leap_year => 29,
            2 => 28,
            4 | 6 | 9 | 11 => 30,
            _ => 31,
        }
    }
}

/// Time with hour, minute, and second
#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Time {
    pub hour: u8,
    pub min: u8,
    pub sec: u8,
    /// Nanosecond is not used internally
    pub nanosecond: u32,
}

impl std::fmt::Display for Time {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02}:{:02}:{:02}", self.hour, self.min, self.sec)
    }
}

#[cfg(feature = "chrono")]
impl From<chrono::NaiveTime> for Time {
    fn from(value: chrono::NaiveTime) -> Self {
        Self::new(
            value.hour() as u8,
            value.minute() as u8,
            value.second() as u8,
            value.nanosecond(),
        )
    }
}

#[cfg(feature = "chrono")]
impl From<Time> for chrono::NaiveTime {
    fn from(val: Time) -> Self {
        chrono::NaiveTime::from_hms_nano_opt(
            val.hour as u32,
            val.min as u32,
            val.sec as u32,
            val.nanosecond,
        )
        .expect("should be valid time")
    }
}

impl Time {
    /// New unchecked time from hour, minute, second, and nanosecond
    pub fn new(hour: u8, min: u8, sec: u8, nanosecond: u32) -> Self {
        // TODO check valid time here instead of from_str
        Self {
            hour,
            min,
            sec,
            nanosecond,
        }
    }

    /// Count of seconds since the midnight
    pub fn seconds_since_midnight(&self) -> u32 {
        (self.hour as u32 * 60 + self.min as u32) * 60 + self.sec as u32
    }

    /// Build time from seconds since midnight
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

/// Offset for the datetime for timezone implementation
#[repr(C)]
#[derive(StableAbi, Default, Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub struct Offset {
    pub hour: u8,
    pub min: u8,
    /// whether it is east or west timezone
    pub east: bool,
}

#[cfg(feature = "chrono")]
impl From<chrono::FixedOffset> for Offset {
    fn from(value: chrono::FixedOffset) -> Self {
        let (secs, east) = {
            let s = value.local_minus_utc();
            if s > 0 {
                (s, false)
            } else {
                (s.abs(), true)
            }
        };
        let m = secs / 60;
        let h = m / 60;
        let min = (m - h * 60) as u8;
        let hour = h as u8;
        Self { hour, min, east }
    }
}

#[cfg(feature = "chrono")]
impl From<Offset> for chrono::FixedOffset {
    fn from(val: Offset) -> Self {
        let secs = (val.hour as i32 * 60 + val.min as i32) * 60;
        if val.east {
            chrono::FixedOffset::east_opt(secs).expect("should be valid offset")
        } else {
            chrono::FixedOffset::west_opt(secs).expect("should be valid offset")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("something", true.into())]
    #[case("sething", 12.into())]
    #[case("someng", 12.0.into())]
    #[case("SOMETHING", "same_true".into())]
    #[case("SoMe", attr!(true, "something").into())]
    #[case("SoMe", attr!(x => true, y => "something").into())]
    #[should_panic]
    #[case("_", attr!("some value"))]
    fn test_set_get_attr(#[case] key: &str, #[case] val: Attribute) {
        let mut am = AttrMap::new();
        assert!(am.set_attr(key, val.clone()).is_none());
        assert_eq!(am.attr(key), Some(&val));
        assert_eq!(am.del_attr(key), Some(val));
        assert!(am.attr(key).is_none());
    }

    #[rstest]
    #[case("something", true.into())]
    #[case("sething", 12.into())]
    #[case("someng", 12.0.into())]
    #[case("SOMETHING", "same_true".into())]
    #[case("SoMe", attr!(true, "something").into())]
    #[case("SoMe", attr!(x => true, y => "something").into())]
    #[should_panic]
    #[case("_", attr!("some value"))]
    #[case("something.else", true.into())]
    #[case("set.hing", 12.into())]
    #[case("some.ng", 12.0.into())]
    #[case("SOMET.HING", "same_true".into())]
    #[case("So.Me", attr!(true, "something").into())]
    #[case("SoMe.STH", attr!(x => true, y => "something").into())]
    #[should_panic]
    #[case("hsh._.shs", attr!("some value"))]
    fn test_set_get_attr_dot(#[case] key: &str, #[case] val: Attribute) {
        let mut am = AttrMap::new();
        assert!(am.set_attr_dot(key, val.clone()).unwrap().is_none());
        assert_eq!(am.attr_dot(key), Ok(Some(&val)));
        assert_eq!(am.del_attr_dot(key), Ok(Some(val)));
        assert_eq!(am.attr_dot(key), Ok(None));
    }

    // this tests the conversion using into, as well as the type name
    #[rstest]
    #[case(attr![true,], attr!(true), Ok(true))]
    #[case(attr![true, 1], attr!(12), Ok(false))]
    #[case(attr!["vals", 1], attr!("vals"), Ok(true))]
    #[case(attr!["value is this"], attr!("this"), Ok(true))]
    #[case(attr!["value is this"], attr!("that"), Ok(false))]
    #[case(attr![true], attr!(12), Err(EvalError::InvalidOperation))]
    // you can check other attributes in a string
    #[case(attr!["12 numbers"], attr!(12), Ok(true))]
    #[case(attr!["12 numbers"], attr!(true), Ok(false))]
    #[case(attr!["true numbers"], attr!(true), Ok(true))]
    #[case(attr![2024], attr!(12), Err(EvalError::InvalidOperation))]
    #[case(attr![1.1232], attr!(12), Err(EvalError::InvalidOperation))]
    fn test_contains(
        #[case] a: Attribute,
        #[case] b: Attribute,
        #[case] res: Result<bool, EvalError>,
    ) {
        println!("{a:?} {b:?}");
        assert_eq!(a.contains(&b), res)
    }

    // this tests the conversion using into, as well as the type name
    #[rstest]
    #[case(true.into(), "Bool")]
    #[case(1i32.into(), "Integer")]
    #[case(1usize.into(), "Integer")]
    #[case(1i64.into(), "Integer")]
    #[case(1.0f32.into(), "Float")]
    #[case(1.0f64.into(), "Float")]
    #[case(String::new().into(), "String")]
    #[case(Vec::<Attribute>::new().into(), "Array")]
    #[case(AttrMap::new().into(), "Table")]
    fn test_into_data_type(#[case] v: Attribute, #[case] t: &str) {
        assert_eq!(v.type_name(), t)
    }

    #[rstest]
    #[case(1usize.into(), 1i32.into())]
    #[case(true.into(), true.into())]
    #[case(1i32.into(), 1i64.into())]
    #[case(String::new().into(), String::new().into())]
    fn test_partial_eq(#[case] a: Attribute, #[case] b: Attribute) {
        assert!(a == b)
    }

    #[rstest]
    #[case(true.into(), 1i32.into())]
    #[case(true.into(), false.into())]
    #[case(1i32.into(), 2i64.into())]
    #[case(1usize.into(), 1.0.into())]
    #[case(String::new().into(), String::from("hi").into())]
    fn test_partial_neq(#[case] a: Attribute, #[case] b: Attribute) {
        assert!(a != b)
    }

    #[rstest]
    #[case(true.into(), false.into())]
    #[case(3i32.into(), 2i64.into())]
    #[case(2usize.into(), 1.0.into())]
    #[case(String::from("hi").into(), String::new().into())]
    #[case(String::from("hi").into(), String::from("ha").into())]
    fn test_partial_gt(#[case] a: Attribute, #[case] b: Attribute) {
        assert!(a > b)
    }

    #[rstest]
    fn test_tuple_impl() {
        let vals = Attribute::Array(vec![Attribute::Integer(0), Attribute::Bool(true)].into());
        let (val, flag): (i64, bool) = FromAttribute::from_attr(&vals).unwrap();
        assert!(val == 0);
        assert!(flag);

        let vals = attr_array!(true, 1.0, "values");
        let (flag, flt, st): (bool, f64, String) = FromAttribute::from_attr(&vals).unwrap();
        assert!(flt == 1.0);
        assert!(flag);
        assert_eq!(st, "values");

        assert!(<(bool, f64, i64) as FromAttribute>::from_attr(&vals).is_none());
    }

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
