use crate::prelude::*;
use abi_stable::{
    std_types::{RHashMap, ROption, RString, Tuple2},
    StableAbi,
};
use std::str::FromStr;

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum NadiAttrType {
    /// Boolean value (`true` or `false`)
    Bool,
    /// String value
    String,
    /// Integer value
    Integer,
    /// Float value
    Float,
    /// Date value with year, month, day
    Date,
    /// Time value with hour, minute, second
    Time,
    /// Date and Time value
    DateTime,
    /// Array/List of [`Attribute`]s
    Array,
    /// HashMap of [`Attribute`]s by name
    Table,
}

impl NadiAttrType {
    pub fn type_name(&self) -> &str {
        match self {
            Self::Bool => "Bool",
            Self::String => "String",
            Self::Integer => "Integer",
            Self::Float => "Float",
            Self::Date => "Date",
            Self::Time => "Time",
            Self::DateTime => "DateTime",
            Self::Array => "Array",
            Self::Table => "Table",
        }
    }
}

impl std::fmt::Display for NadiAttrType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", self.type_name())
    }
}

impl FromStr for NadiAttrType {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "Bool" => Self::Bool,
            "String" => Self::String,
            "Integer" => Self::Integer,
            "Float" => Self::Float,
            "Date" => Self::Date,
            "Time" => Self::Time,
            "DateTime" => Self::DateTime,
            "Array" => Self::Array,
            "Table" => Self::Table,
            _ => return Err(format!("Invalid type name: {s:?}")),
        })
    }
}

#[repr(C)]
#[derive(StableAbi, Clone, PartialEq, Debug)]
pub enum NadiType {
    /// Attribute
    Attribute(NadiAttrType),
    TimeSeries(ROption<NadiAttrType>),
    Series(ROption<NadiAttrType>),
}

#[derive(Clone, Debug, PartialEq)]
/// Data type for struct definition as well as use
pub struct NadiStruct {
    pub name: RString,
    pub fields: RHashMap<RString, NadiAttrType>,
    pub values: RHashMap<RString, Attribute>,
}

impl std::fmt::Display for NadiStruct {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "struct {} {{\n{}\n}}",
            self.name,
            self.fields
                .iter()
                .map(|Tuple2(k, t)| {
                    match self.values.get(k) {
                        Some(v) => format!("{k}: {t} = {v}"),
                        None => format!("{k}: {t}"),
                    }
                })
                .collect::<Vec<String>>()
                .join(",\n")
        )
    }
}

impl NadiStruct {
    pub fn with_name(name: String) -> Self {
        Self {
            name: name.into(),
            fields: RHashMap::new(),
            values: RHashMap::new(),
        }
    }
}

impl FromAttribute for NadiStruct {
    fn from_attr(value: &Attribute) -> Option<Self> {
        match value {
            Attribute::Table(am) => Some(NadiStruct {
                name: "".into(),
                fields: RHashMap::new(),
                values: am.clone(),
            }),
            _ => None,
        }
    }
}

impl From<NadiStruct> for Attribute {
    fn from(val: NadiStruct) -> Attribute {
        Attribute::Table(val.values)
    }
}

#[derive(Clone, Debug, PartialEq)]
/// Expression to build a struct
pub struct NadiStructExpr<T> {
    pub name: String,
    pub values: RHashMap<RString, T>,
}

impl<T> NadiStructExpr<T> {
    pub fn with_name(name: String) -> Self {
        Self {
            name: name,
            values: RHashMap::new(),
        }
    }
}

impl<T: std::fmt::Display> std::fmt::Display for NadiStructExpr<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{} {{\n{}\n}}",
            self.name,
            self.values
                .iter()
                .map(|Tuple2(k, v)| { format!("{k} = {v}") })
                .collect::<Vec<String>>()
                .join(",\n")
        )
    }
}
