use crate::parser::tokenizer::{get_tokens, TaskToken};
use core::ops::Range;

pub fn get_highlight(txt: &str, nft: &NadiFileType) -> Vec<(Range<usize>, Highlight)> {
    let mut offset = 0;
    get_tokens(txt)
        .into_iter()
        .map(|t| {
            let start = offset;
            offset += t.content.len();
            (start..offset, Highlight::from_token(&t.ty, nft))
        })
        .collect()
}

pub enum Highlight {
    Comment,
    Keyword,
    Symbol,
    Operator,
    Paren,
    Variable,
    Function,
    Bool,
    Number,
    DateTime,
    String,
    Error,
    None,
}

impl Highlight {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Comment => "comment",
            Self::Keyword => "keyword",
            Self::Symbol => "symbol",
            Self::Operator => "operator",
            Self::Paren => "paren",
            Self::Variable => "variable",
            Self::Function => "function",
            Self::Bool => "bool",
            Self::Number => "number",
            Self::DateTime => "datetime",
            Self::String => "string",
            Self::Error => "error",
            Self::None => "none",
        }
    }
}

#[derive(Clone, PartialEq, Default)]
pub enum NadiFileType {
    Network,
    Attribute,
    #[default]
    Tasks,
    Terminal,
}

impl std::str::FromStr for NadiFileType {
    type Err = ();
    fn from_str(val: &str) -> Result<Self, Self::Err> {
        match val {
            "net" | "network" => Ok(Self::Network),
            "task" | "tasks" => Ok(Self::Tasks),
            "toml" => Ok(Self::Attribute),
            "log" => Ok(Self::Terminal),
            _ => Err(()),
        }
    }
}

impl Highlight {
    pub fn from_token(tk: &TaskToken, ntf: &NadiFileType) -> Self {
        match ntf {
            NadiFileType::Network => match tk {
                TaskToken::Comment => Self::Comment,
                TaskToken::Keyword(_) => Self::Variable,
                TaskToken::PathSep => Self::Symbol,
                TaskToken::Variable => Self::Variable,
                TaskToken::Bool => Self::Variable,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Variable,
                TaskToken::Invalid(_) => Self::Error,
                TaskToken::NewLine | TaskToken::WhiteSpace => Self::None,
                _ => Self::Error,
            },
            NadiFileType::Attribute => match tk {
                TaskToken::Comment => Self::Comment,
                TaskToken::Keyword(_) => Self::Variable,
                TaskToken::ParenStart => Self::Paren,
                TaskToken::BraceStart => Self::Paren,
                TaskToken::BracketStart => Self::Paren,
                TaskToken::Comma => Self::Symbol,
                TaskToken::Dot => Self::Symbol,
                TaskToken::ParenEnd => Self::Paren,
                TaskToken::BraceEnd => Self::Paren,
                TaskToken::BracketEnd => Self::Paren,
                TaskToken::Assignment => Self::Symbol,
                TaskToken::Variable | TaskToken::Dash => Self::Variable,
                TaskToken::Bool => Self::Bool,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Number,
                TaskToken::Date | TaskToken::Time | TaskToken::DateTime => Self::DateTime,
                TaskToken::Invalid(_) => Self::Error,
                TaskToken::PathSep => Self::Error,
                TaskToken::Function => Self::Error,
                TaskToken::NewLine | TaskToken::WhiteSpace => Self::None,
                _ => Self::Error,
            },
            NadiFileType::Tasks | NadiFileType::Terminal => match tk {
                TaskToken::Comment => Self::Comment,
                TaskToken::Keyword(_) => Self::Keyword,
                TaskToken::AngleStart => Self::Paren,
                TaskToken::ParenStart => Self::Paren,
                TaskToken::BraceStart => Self::Paren,
                TaskToken::BracketStart => Self::Paren,
                TaskToken::PathSep => Self::Symbol,
                TaskToken::Comma => Self::Symbol,
                TaskToken::Dot => Self::Symbol,
                TaskToken::Dash => Self::Operator,
                TaskToken::Plus => Self::Operator,
                TaskToken::Star => Self::Operator,
                TaskToken::Slash => Self::Operator,
                TaskToken::Percentage => Self::Operator,
                TaskToken::Question => Self::Symbol,
                TaskToken::Semicolon => Self::Operator,
                TaskToken::None => Self::Comment,
                TaskToken::Caret => Self::Operator,
                TaskToken::And => Self::Operator,
                TaskToken::Or => Self::Operator,
                TaskToken::Not => Self::Operator,
                TaskToken::AngleEnd => Self::Paren,
                TaskToken::ParenEnd => Self::Paren,
                TaskToken::BraceEnd => Self::Paren,
                TaskToken::BracketEnd => Self::Paren,
                TaskToken::Variable => Self::Variable,
                TaskToken::Function => Self::Function,
                TaskToken::Assignment => Self::Operator,
                TaskToken::Bool => Self::Bool,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Number,
                TaskToken::Date | TaskToken::Time | TaskToken::DateTime => Self::DateTime,
                TaskToken::NaN | TaskToken::Infinity => Self::Number,
                TaskToken::Invalid(_) => Self::Error,
                TaskToken::NewLine | TaskToken::WhiteSpace => Self::None,
            },
        }
    }
}
