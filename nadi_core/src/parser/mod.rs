use crate::attrs::{Date, DateTime, Time};
use crate::functions::Propagation;
use crate::network::StrPath;
use crate::parser::tokenizer::{get_tokens, TaskToken, VecTokens};
use crate::prelude::*;
use crate::table::Table;
use abi_stable::std_types::{RString, Tuple2};
use anyhow::Context;
use colored::Colorize;
use std::path::Path;
use std::str::FromStr;

pub mod attrs;
pub mod network;
pub mod string;
pub mod table;
pub mod tasks;
pub mod tokenizer;

pub trait NadiError: std::error::Error {
    fn user_msg(&self, filename: Option<&str>) -> String {
        if let Some(fname) = filename {
            format!("Error on file: {fname:?}")
        } else {
            format!("Error occured")
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ParseError {
    pub ty: ParseErrorType,
    pub line: usize,
    pub col: usize,
    pub linestr: String,
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "ParseError: {} at line {} col {}",
            self.ty.message(),
            self.line,
            self.col
        )
    }
}

impl NadiError for ParseError {
    fn user_msg(&self, filename: Option<&str>) -> String {
        let mut msg = String::new();
        msg.push_str(&format!(
            "{}: Parse Error at Line {} Column {}\n",
            "Error".bright_red(),
            self.line,
            self.col
        ));
        if let Some(fname) = filename {
            msg.push_str(&format!(
                "  {} {}\n",
                "->".blue(),
                format!("{}:{}:{}", fname, self.line, self.col).blue()
            ));
        }
        msg.push_str(&format!("  {}\n", self.linestr));
        msg.push_str(&format!(
            "  {: >2$} {}",
            "^".yellow(),
            self.ty.message().yellow(),
            self.col + 1
        ));
        if let ParseErrorType::LogicalError(s) = &self.ty {
            msg.push_str(&format!("\n  {}", s.red()))
        }
        msg
    }
}

#[derive(Debug, PartialEq, Clone)]
pub enum ParseErrorType {
    LogicalError(&'static str),
    ValueError(&'static str),
    InvalidLineStart,
    Unclosed,
    InvalidPropagation,
    SyntaxError,
    InvalidToken,
}

impl ParseErrorType {
    pub fn message(&self) -> String {
        match self {
            Self::LogicalError(v) => {
                return format!("Unexpected Logic problem: {v}, please contact dev")
            }
            Self::ValueError(v) => return format!("Invalid Value: {v}"),
            Self::InvalidLineStart => "Lines should start with a keyword",
            Self::Unclosed => "Incomplete Input",
            Self::InvalidPropagation => "Invalid propagation value",
            Self::SyntaxError => "Invalid Syntax",
            Self::InvalidToken => "Invalid Token",
        }
        .to_string()
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
        if month < 1 && month > 12 {
            return Err("Invalid Month (use 1-12)");
        }
        // doesn't make too many assumption on calendar type (leap
        // year or others)
        if day < 1 && day > 31 {
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
        } else {
            if let Some((d, t)) = s.split_once('T') {
                (d.trim(), t.trim())
            } else {
                return Err("Invalid DateTime use YYYY-mm-dd HH:MM[:SS]");
            }
        };
        Ok(DateTime::new(Date::from_str(d)?, Time::from_str(t)?, None))
    }
}

impl Network {
    // TODO import DOT format as well, or maybe make it work through plugin
    pub fn from_file<P: AsRef<Path>>(filename: P) -> anyhow::Result<Self> {
        let mut network = Self::default();
        let content =
            std::fs::read_to_string(filename).context("Error while accessing the network file")?;
        let tokens = tokenizer::get_tokens(&content)?;
        let paths = network::parse(tokens)?;
        for path in paths {
            if !network.nodes_map.contains_key(&path.start) {
                network.insert_node_by_name(&path.start);
            }
            if !network.nodes_map.contains_key(&path.end) {
                network.insert_node_by_name(&path.end);
            }
            let inp = network.node_by_name(&path.start).unwrap();
            let out = network.node_by_name(&path.end).unwrap();
            {
                inp.lock().set_output(out.clone());
                out.lock().add_input(inp.clone());
            }
        }
        network.reorder();
        network.set_levels();
        Ok(network)
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
        let tokens = tokenizer::get_tokens(&contents)?;
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
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut tokens = VecTokens::new(get_tokens(&s)?);
        let tk = match tokens.next_no_ws(false) {
            None => return Err(anyhow::Error::msg("No propagation")),
            Some(t) => t,
        };
        match tk.ty {
            TaskToken::Variable => Ok(propagation(tk.content)?),
            TaskToken::ParenStart => {
                let tt = match tokens.next_no_ws(false) {
                    None => return Err(anyhow::Error::msg("No propagation")),
                    Some(t) => t,
                };
                match tt.ty {
                    TaskToken::Variable => Ok(propagation(tt.content)?),
                    _ => Err(tokens
                        .parse_error(ParseErrorType::InvalidPropagation)
                        .into()),
                }
            }
            TaskToken::BracketStart => {
                let mut path = false;
                let mut comma = false;
                let mut nodes = vec![];
                while let Some(t) = tokens.next_no_ws(false) {
                    if comma {
                        match t.ty {
                            TaskToken::Comma => {
                                comma = false;
                                continue;
                            }
                            TaskToken::PathSep => {
                                path = true;
                                continue;
                            }
                            _ => {
                                return Err(tokens
                                    .parse_error(ParseErrorType::InvalidPropagation)
                                    .into())
                            }
                        }
                    }
                    match t.ty {
                        TaskToken::Variable => {
                            nodes.push(t.content.to_string());
                            comma = true;
                        }
                        TaskToken::String(s) => {
                            nodes.push(s);
                            comma = true;
                        }
                        _ => {
                            return Err(tokens
                                .parse_error(ParseErrorType::InvalidPropagation)
                                .into())
                        }
                    }
                    if path && nodes.len() == 2 {
                        return Ok(Propagation::Path(StrPath::new(
                            nodes[0].as_str().into(),
                            nodes[1].as_str().into(),
                        )));
                    }
                }
                let nodes: Vec<RString> = nodes.into_iter().map(|s| s.into()).collect();
                Ok(Propagation::List(nodes.into()))
            }
            _ => Err(anyhow::Error::msg("No propagation")),
        }
    }
}

fn propagation(p: &str) -> anyhow::Result<Propagation> {
    match p {
        "sequential" => Ok(Propagation::Sequential),
        "inverse" => Ok(Propagation::Inverse),
        "inputsfirst" => Ok(Propagation::InputsFirst),
        "outputfirst" => Ok(Propagation::OutputFirst),
        _ => Err(anyhow::Error::msg("Invalid propagation type")),
    }
}
