use core::ops::Range;
use iced::Color;
use iced::Font;
use iced_core::text::highlighter::{Format, Highlighter};
use nadi_core::parser::tokenizer::{TaskToken, get_tokens};

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
    fn from_token(tk: TaskToken, ntf: &NadiFileType) -> Self {
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
            Self::Symbol => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
            Self::Paren => None,
            Self::Variable => Some(Color::new(0.0, 0.5, 0.0, 1.0)),
            Self::Function => Some(Color::new(0.5, 0.2, 0.2, 1.0)),
            Self::Bool => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
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
    fn new(line: &str, nft: &NadiFileType) -> Self {
        match get_tokens(line) {
            Ok(tk) => {
                let tokens = tk
                    .into_iter()
                    .rev()
                    .map(|t| (Highlight::from_token(t.ty, nft), t.content.len()))
                    .collect();
                Self { offset: 0, tokens }
            }
            Err(_) => Self {
                offset: 0,
                tokens: vec![(Highlight::Error, line.len())],
            },
        }
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
    settings: NadiFileType,
}

impl Highlighter for NadiHighlighter {
    type Settings = NadiFileType;
    type Highlight = Highlight;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;
    fn new(settings: &Self::Settings) -> Self {
        Self {
            curr_line: 0,
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
        self.curr_line += 1;
        Box::new(HlTokens::new(line, &self.settings))
    }
    fn current_line(&self) -> usize {
        self.curr_line
    }
}
