use core::ops::Range;
use iced::Color;
use iced::Font;
use iced_core::text::highlighter::{Format, Highlighter};
use nadi_core::parser::tokenizer::{TaskToken, get_tokens};

struct Tokens {
    offset: usize,
    error: Option<usize>,
    tokens: Vec<(TaskToken, usize)>,
}

pub enum Highlight {
    Token(TaskToken),
    Error,
}

impl Highlight {
    pub fn to_format(&self, _theme: &iced::Theme) -> Format<Font> {
        let color = match self {
            // TODO: make it better; also differentiate tasks/toml/network/ etc
            Self::Token(tk) => match tk {
                TaskToken::Comment => Some(Color::new(0.5, 0.5, 0.5, 1.0)),
                TaskToken::Keyword(_) => Some(Color::new(0.7, 0.0, 0.0, 1.0)),
                TaskToken::AngleStart => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::ParenStart => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::BraceStart => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::BracketStart => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::PathSep => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::Comma => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::Dot => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::And => Some(Color::new(0.0, 0.5, 0.5, 1.0)),
                TaskToken::Or => Some(Color::new(0.0, 0.5, 0.5, 1.0)),
                TaskToken::Not => Some(Color::new(0.0, 0.5, 0.5, 1.0)),
                TaskToken::AngleEnd => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::ParenEnd => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::BraceEnd => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::BracketEnd => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::Variable => Some(Color::new(0.0, 0.5, 0.0, 1.0)),
                TaskToken::Function => Some(Color::new(0.5, 0.2, 0.2, 1.0)),
                TaskToken::Assignment => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
                TaskToken::Bool => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::String(_) => Some(Color::new(0.0, 0.6, 0.4, 1.0)),
                TaskToken::Integer => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::Float => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::Date => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::Time => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::DateTime => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
                TaskToken::Quote => Some(Color::new(1.0, 0.0, 0.0, 1.0)),
                _ => None,
            },
            Self::Error => Some(Color::new(1.0, 0.0, 0.0, 1.0)),
        };
        Format { color, font: None }
    }
}

impl Tokens {
    fn new(line: &str) -> Self {
        match get_tokens(line) {
            Ok(tk) => Self {
                offset: 0,
                error: None,
                tokens: tk
                    .into_iter()
                    .rev()
                    .map(|t| (t.ty, t.content.len()))
                    .collect(),
            },
            Err(_) => Self {
                offset: 0,
                error: Some(line.len()),
                tokens: vec![],
            },
        }
    }
}

impl Iterator for Tokens {
    type Item = (Range<usize>, Highlight);

    fn next(&mut self) -> Option<Self::Item> {
        match self.error.take() {
            Some(err) => Some((0..err, Highlight::Error)),
            None => {
                let (tk, l) = self.tokens.pop()?;
                let st = self.offset;
                self.offset += l;
                Some((st..self.offset, Highlight::Token(tk)))
            }
        }
    }
}

pub struct TaskHighlighter {
    curr_line: usize,
}

impl Highlighter for TaskHighlighter {
    type Settings = iced::highlighter::Settings;
    type Highlight = Highlight;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;
    fn new(_settings: &Self::Settings) -> Self {
        Self { curr_line: 0 }
    }
    fn update(&mut self, _new_settings: &Self::Settings) {
        ()
        // self.settings = new_settings;
    }
    fn change_line(&mut self, line: usize) {
        self.curr_line = line;
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        Box::new(Tokens::new(line))
    }
    fn current_line(&self) -> usize {
        self.curr_line
    }
}
