use crate::prelude::*;
use abi_stable::{
    std_types::{RHashMap, ROption, RString},
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

#[derive(Clone, Debug)]
/// Data type for struct definition as well as use
pub struct NadiStruct {
    name: RString,
    fields: RHashMap<RString, NadiType>,
    values: RHashMap<RString, Attribute>,
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
