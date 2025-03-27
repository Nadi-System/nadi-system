use core::ops::Range;
use iced::Color;
use iced::Font;
use iced_core::text::highlighter::{Format, Highlighter};
use nadi_core::parser::tokenizer::{TaskToken, get_tokens};
use std::collections::HashSet;

struct HlTokens {
    offset: usize,
    tokens: Vec<(Highlight, usize)>,
}

#[derive(Clone, PartialEq)]
pub enum NadiFileType {
    Network,
    Attribute,
    Tasks,
}

impl std::str::FromStr for NadiFileType {
    type Err = ();
    fn from_str(val: &str) -> Result<Self, Self::Err> {
        match val {
            "net" | "network" => Ok(Self::Network),
            "task" | "tasks" => Ok(Self::Tasks),
            "toml" => Ok(Self::Attribute),
            _ => Err(()),
        }
    }
}

// pub struct Settings {
//     pub(super) theme: iced::highlighter::Theme,
//     pub(super) nft: NadiFileType,
// }

pub enum Highlight {
    Comment,
    Keyword,
    Symbol,
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
    fn from_token(tk: &TaskToken, ntf: &NadiFileType) -> Self {
        match ntf {
            NadiFileType::Network => match tk {
                TaskToken::Comment => Self::Comment,
                TaskToken::Keyword(_) => Self::Variable,
                TaskToken::PathSep => Self::Symbol,
                TaskToken::Variable => Self::Variable,
                TaskToken::Bool => Self::Variable,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Variable,
                TaskToken::Quote => Self::Error,
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
                TaskToken::Variable => Self::Variable,
                TaskToken::Bool => Self::Bool,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Number,
                TaskToken::Date | TaskToken::Time | TaskToken::DateTime => Self::DateTime,
                TaskToken::Quote => Self::Error,
                TaskToken::PathSep => Self::Error,
                TaskToken::Function => Self::Error,
                TaskToken::NewLine | TaskToken::WhiteSpace => Self::None,
                _ => Self::Error,
            },
            NadiFileType::Tasks => match tk {
                TaskToken::Comment => Self::Comment,
                TaskToken::Keyword(_) => Self::Keyword,
                TaskToken::AngleStart => Self::Paren,
                TaskToken::ParenStart => Self::Paren,
                TaskToken::BraceStart => Self::Paren,
                TaskToken::BracketStart => Self::Paren,
                TaskToken::PathSep => Self::Symbol,
                TaskToken::Comma => Self::Symbol,
                TaskToken::Dot => Self::Symbol,
                TaskToken::And => Self::Symbol,
                TaskToken::Or => Self::Symbol,
                TaskToken::Not => Self::Symbol,
                TaskToken::AngleEnd => Self::Paren,
                TaskToken::ParenEnd => Self::Paren,
                TaskToken::BraceEnd => Self::Paren,
                TaskToken::BracketEnd => Self::Paren,
                TaskToken::Variable => Self::Variable,
                TaskToken::Function => Self::Function,
                TaskToken::Assignment => Self::Symbol,
                TaskToken::Bool => Self::Bool,
                TaskToken::String(_) => Self::String,
                TaskToken::Integer | TaskToken::Float => Self::Number,
                TaskToken::Date | TaskToken::Time | TaskToken::DateTime => Self::DateTime,
                TaskToken::Quote => Self::Error,
                TaskToken::NewLine | TaskToken::WhiteSpace => Self::None,
            },
        }
    }

    pub fn to_format(&self, _theme: &iced::Theme) -> Format<Font> {
        let color = match self {
            Self::Comment => Some(Color::new(0.5, 0.5, 0.5, 0.7)),
            Self::Keyword => Some(Color::new(0.7, 0.0, 0.0, 1.0)),
            Self::Symbol => None,
            Self::Paren => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
            Self::Variable => Some(Color::new(0.0, 0.5, 0.0, 1.0)),
            Self::Function => Some(Color::new(0.5, 0.2, 0.2, 1.0)),
            Self::Bool => Some(Color::new(0.4, 0.6, 0.9, 1.0)),
            Self::Number => None,
            Self::DateTime => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
            Self::String => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
            Self::Error => Some(Color::new(1.0, 0.3, 0.3, 1.0)),
            Self::None => None,
        };
        Format { color, font: None }
    }
}

impl HlTokens {
    fn new(line: &str, nft: &NadiFileType) -> (bool, Self) {
        let mut quote = false;
        let tk = match get_tokens(line) {
            Ok(tk) => {
                let tokens = if let Some(p) = tk.iter().position(|t| t.ty == TaskToken::Quote) {
                    quote = true;
                    let mut tokens = vec![(
                        Highlight::String,
                        tk[p..].iter().map(|t| t.content.len()).sum(),
                    )];
                    tokens.extend(
                        tk[..p]
                            .iter()
                            .rev()
                            .map(|t| (Highlight::from_token(&t.ty, nft), t.content.len())),
                    );
                    tokens
                } else {
                    tk.iter()
                        .rev()
                        .map(|t| (Highlight::from_token(&t.ty, nft), t.content.len()))
                        .collect()
                };
                Self { offset: 0, tokens }
            }
            Err(_) => Self {
                offset: 0,
                tokens: vec![(Highlight::Error, line.len())],
            },
        };
        (quote, tk)
    }

    fn in_quote(line: &str, nft: &NadiFileType) -> (bool, Self) {
        let mut quote = true;
        if !line.contains('"') {
            return (
                quote,
                Self {
                    offset: 0,
                    tokens: vec![(Highlight::String, line.len())],
                },
            );
        }
        let tk = match get_tokens(&format!("\"{line}")) {
            Ok(tk) => {
                let mut tokens = if let Some(t) = tk.iter().next() {
                    match t.ty {
                        // the quote was not closed
                        TaskToken::Quote => {
                            return (
                                quote,
                                Self {
                                    offset: 0,
                                    tokens: vec![(Highlight::String, line.len())],
                                },
                            );
                        }
                        // the quote was closed
                        TaskToken::String(_) => {
                            quote = false;
                            vec![(Highlight::String, t.content.len() - 1)]
                        }
                        // shouldn't happen
                        _ => panic!("Logic Error: the quote should be closed or open"),
                    }
                } else {
                    panic!("There is a quote even if line is empty, so tokens shouldn't be empty")
                };
                tokens.extend(
                    tk.iter()
                        .skip(1)
                        .map(|t| (Highlight::from_token(&t.ty, nft), t.content.len())),
                );
                Self {
                    offset: 0,
                    tokens: tokens.into_iter().rev().collect(),
                }
            }
            // if there is error, there are probably extra characters inside string (temp fix)
            Err(_) => Self {
                offset: 0,
                tokens: vec![(Highlight::String, line.len())],
            },
        };
        (quote, tk)
    }
}

impl Iterator for HlTokens {
    type Item = (Range<usize>, Highlight);

    fn next(&mut self) -> Option<Self::Item> {
        let (tk, l) = self.tokens.pop()?;
        let st = self.offset;
        self.offset += l;
        Some((st..self.offset, tk))
    }
}

pub struct NadiHighlighter {
    curr_line: usize,
    in_quote: bool,
    str_lines: HashSet<usize>,
    settings: NadiFileType,
}

impl Highlighter for NadiHighlighter {
    type Settings = NadiFileType;
    type Highlight = Highlight;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;
    fn new(settings: &Self::Settings) -> Self {
        Self {
            curr_line: 0,
            in_quote: false,
            str_lines: HashSet::new(),
            settings: settings.clone(),
        }
    }
    fn update(&mut self, new_settings: &Self::Settings) {
        self.settings = new_settings.clone();
    }
    fn change_line(&mut self, line: usize) {
        self.curr_line = line;
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        let (q, tk) = if self.in_quote || self.str_lines.contains(&self.curr_line) {
            HlTokens::in_quote(line, &self.settings)
        } else {
            HlTokens::new(line, &self.settings)
        };
        self.in_quote = q;
        self.curr_line += 1;
        if self.in_quote {
            self.str_lines.insert(self.curr_line);
        } else {
            self.str_lines.remove(&self.curr_line);
        }
        Box::new(tk)
    }
    fn current_line(&self) -> usize {
        self.curr_line
    }
}
