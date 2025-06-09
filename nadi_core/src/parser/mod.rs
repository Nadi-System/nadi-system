use crate::attrs::{Date, DateTime, Time};
use crate::network::Propagation;
use crate::prelude::*;
use crate::table::Table;
use abi_stable::std_types::Tuple2;
use anyhow::Context;
use std::path::Path;
use std::str::FromStr;

pub mod attrs;
pub mod components;
pub mod errors;
pub mod expressions;
pub mod highlight;
pub mod network;
pub mod string;
pub mod table;
pub mod tasks;
pub mod tokenizer;

pub use errors::{ParseError, ParseErrorType};

impl std::str::FromStr for Attribute {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens = crate::parser::tokenizer::get_tokens(s);
        let (rest, val) =
            crate::parser::components::attribute(&tokens).map_err(|e| e.to_string())?;
        if !rest.is_empty() {
            Err(ParseError::new(&tokens, rest, ParseErrorType::InvalidToken).to_string())
        } else {
            Ok(val)
        }
    }
}

impl std::str::FromStr for Date {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split('-');
        let year = parts
            .next()
            .ok_or("Year not present")?
            .parse::<u16>()
            .map_err(|_| "Invalid Year")?;
        let month = parts
            .next()
            .ok_or("Month not present")?
            .parse::<u8>()
            .map_err(|_| "Invalid Month")?;
        let day = parts
            .next()
            .ok_or("Day not present")?
            .parse::<u8>()
            .map_err(|_| "Invalid Day")?;
        if !(1..=12).contains(&month) {
            return Err("Invalid Month (use 1-12)");
        }
        // doesn't make too many assumption on calendar type (leap
        // year or others)
        if !(1..=31).contains(&day) {
            return Err("Invalid Day (use 1-31)");
        }
        Ok(Date::new(year, month, day))
    }
}

impl std::str::FromStr for Time {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split(':');
        let hour = parts
            .next()
            .ok_or("Hour not present")?
            .parse::<u8>()
            .map_err(|_| "Invalid Hour")?;
        let min = parts
            .next()
            .ok_or("Minute not present")?
            .parse::<u8>()
            .map_err(|_| "Invalid Minute")?;
        let ss = parts.next().unwrap_or("00");
        let (sec, nanosecond) = if let Some((s, n)) = ss.split_once('.') {
            let n = (format!("0.{n}").parse::<f64>().unwrap_or(0.0) * 1e6).ceil() as u32;
            (s.parse::<u8>().map_err(|_| "Invalid Second")?, n)
        } else {
            (ss.parse::<u8>().map_err(|_| "Invalid Second")?, 0)
        };
        if hour >= 24 {
            return Err("Invalid Hour (use 0-23)");
        }
        if min >= 60 {
            return Err("Invalid Minute (use 0-59)");
        }
        if sec >= 60 {
            return Err("Invalid Second (use 0-59)");
        }
        Ok(Time::new(hour, min, sec, nanosecond))
    }
}

impl std::str::FromStr for DateTime {
    type Err = &'static str;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let (d, t) = if let Some((d, t)) = s.split_once(' ') {
            (d.trim(), t.trim())
        } else if let Some((d, t)) = s.split_once('T') {
            (d.trim(), t.trim())
        } else {
            return Err("Invalid DateTime use YYYY-mm-dd HH:MM[:SS]");
        };
        Ok(DateTime::new(Date::from_str(d)?, Time::from_str(t)?, None))
    }
}

impl FromStr for Network {
    type Err = ParseError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let tokens = tokenizer::get_tokens(s);
        let paths = network::parse(&tokens)?;
        let edges: Vec<(&str, &str)> = paths
            .iter()
            .map(|p| (p.start.as_str(), p.end.as_str()))
            .collect();
        Self::from_edges(&edges)
            .map_err(|e| ParseError::new(&tokens, &tokens, ParseErrorType::MultipleOutput(e)))
    }
}

impl Network {
    // TODO import DOT format as well, or maybe make it work through plugin
    pub fn from_file<P: AsRef<Path>>(filename: P) -> anyhow::Result<Self> {
        let content =
            std::fs::read_to_string(&filename).context("Error while accessing the network file")?;
        Self::from_str(&content)
            .map_err(|e| anyhow::Error::msg(e.user_msg(Some(&filename.as_ref().to_string_lossy()))))
    }
    pub fn load_attrs<P: AsRef<Path>>(&self, attr_dir: P) -> anyhow::Result<()> {
        self.nodes_map.iter().try_for_each(|Tuple2(name, node)| {
            // ignore the error on attribute read
            let attr_file = attr_dir.as_ref().join(format!("{}.toml", name));
            if attr_file.exists() && attr_file.is_file() {
                node.lock().load_attr(&attr_file)
            } else {
                Ok(())
            }
        })?;
        Ok(())
    }
}

impl NodeInner {
    pub fn load_attr<P: AsRef<Path>>(&mut self, file: P) -> anyhow::Result<()> {
        let contents = std::fs::read_to_string(file)?;
        let tokens = tokenizer::get_tokens(&contents);
        let attrs = attrs::parse(tokens)?;
        self.attributes.extend(attrs);
        Ok(())
    }
}

impl FromStr for Table {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let cols = crate::parser::table::parse_table_complete(s).map_err(anyhow::Error::msg)?;
        Ok(Self {
            columns: cols.into(),
        })
    }
}

impl Table {
    pub fn from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let contents = std::fs::read_to_string(path)?;
        Self::from_str(&contents)
    }
}

impl FromStr for Propagation {
    type Err = anyhow::Error;
    fn from_str(_s: &str) -> Result<Self, Self::Err> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("1223-12-23", Date::new(1223, 12, 23))]
    #[should_panic] // invalid month
    #[case("1223-24-23", Date::new(1223, 24, 23))]
    #[should_panic] // invalid month
    #[case("1223-04-32", Date::new(1223, 4, 32))]
    fn date_test(#[case] txt: &str, #[case] value: Date) {
        let dt = Date::from_str(txt).unwrap();
        assert!(dt == value)
    }
}
