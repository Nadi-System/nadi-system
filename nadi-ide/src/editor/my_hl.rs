use core::ops::Range;
use iced::Color;
use iced::Font;
use iced_core::text::highlighter::{Format, Highlighter};
use nadi_core::parser::{
    highlight::{Highlight, NadiFileType},
    tokenizer::{TaskToken, get_tokens},
};
use std::collections::HashMap;

struct HlTokens {
    offset: usize,
    tokens: Vec<(Highlight, usize)>,
}

pub fn hlto_format(hl: &Highlight, _theme: &iced::Theme) -> Format<Font> {
    let color = match hl {
        Highlight::Comment => Some(Color::new(0.5, 0.5, 0.5, 0.7)),
        Highlight::Keyword => Some(Color::new(0.7, 0.0, 0.0, 1.0)),
        Highlight::Symbol => None,
        Highlight::Operator => Some(Color::new(0.2, 0.7, 0.7, 1.0)),
        Highlight::Paren => Some(Color::new(0.0, 0.0, 1.0, 1.0)),
        Highlight::Variable => Some(Color::new(0.0, 0.5, 0.0, 1.0)),
        Highlight::Function => Some(Color::new(0.5, 0.2, 0.2, 1.0)),
        Highlight::Bool => Some(Color::new(0.4, 0.6, 0.9, 1.0)),
        Highlight::Number => None,
        Highlight::DateTime => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
        Highlight::String => Some(Color::new(0.1, 0.7, 0.5, 1.0)),
        Highlight::Error => Some(Color::new(1.0, 0.3, 0.3, 1.0)),
        Highlight::None => None,
    };
    Format { color, font: None }
}

// pub struct Settings {
//     pub(super) theme: iced::highlighter::Theme,
//     pub(super) nft: NadiFileType,
// }

impl HlTokens {
    fn new(line: &str, nft: &NadiFileType) -> (Option<MultiLineStr>, Self) {
        let mut mls = None;
        let tk = get_tokens(line);
        let tokens = if let Some(p) = tk.iter().position(|t| t.ty == TaskToken::Invalid('"')) {
            mls = Some(MultiLineStr::Open);
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
        (mls, Self { offset: 0, tokens })
    }

    fn in_quote(line: &str, nft: &NadiFileType) -> (Option<MultiLineStr>, Self) {
        let mut mls = Some(MultiLineStr::In);
        if !line.contains('"') {
            return (
                mls,
                Self {
                    offset: 0,
                    tokens: vec![(Highlight::String, line.len())],
                },
            );
        }
        let temp_line = format!("\"{line}");
        let tk = get_tokens(&temp_line);
        let mut tokens = if let Some(t) = tk.first() {
            match t.ty {
                // the quote was not closed
                TaskToken::Invalid('"') => {
                    return (
                        mls,
                        Self {
                            offset: 0,
                            tokens: vec![(Highlight::String, line.len())],
                        },
                    );
                }
                // the quote was closed
                TaskToken::String(_) => {
                    mls = Some(
                        if tk
                            .iter()
                            .position(|t| t.ty == TaskToken::Invalid('"'))
                            .is_some()
                        {
                            // but another quote is open
                            MultiLineStr::CloseOpen
                        } else {
                            MultiLineStr::Close
                        },
                    );
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
        (
            mls,
            Self {
                offset: 0,
                tokens: tokens.into_iter().rev().collect(),
            },
        )
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

#[derive(Clone)]
enum MultiLineStr {
    Open,
    In,
    Close,
    CloseOpen,
}

pub struct NadiHighlighter {
    curr_line: usize,
    ml_str: HashMap<usize, MultiLineStr>,
    settings: NadiFileType,
}

impl Highlighter for NadiHighlighter {
    type Settings = (NadiFileType, usize);
    type Highlight = Highlight;
    type Iterator<'a> = Box<dyn Iterator<Item = (Range<usize>, Self::Highlight)> + 'a>;
    fn new(settings: &Self::Settings) -> Self {
        Self {
            curr_line: settings.1,
            ml_str: HashMap::new(),
            settings: settings.0.clone(),
        }
    }
    fn update(&mut self, new_settings: &Self::Settings) {
        if self.settings != new_settings.0 {
            self.settings = new_settings.0.clone();
            self.change_line(0);
        }
        self.change_line(new_settings.1);
    }

    fn change_line(&mut self, line: usize) {
        self.curr_line = line;
        // if line is changed, remove the saved states for
        // MultiLineStrings for all lines after this
        self.ml_str.retain(|l, _| l <= &line);
    }
    fn highlight_line(&mut self, line: &str) -> Self::Iterator<'_> {
        // Can't simply use get_highlight from nadi_core as there
        // could be multiline strings, and this just reads one line at
        // a time
        if self.settings == NadiFileType::Terminal {
            return Box::new(HlTokens::new(line, &self.settings).1);
        }

        let (mls, tk) = match self.ml_str.get(&self.curr_line) {
            None | Some(MultiLineStr::Open) => HlTokens::new(line, &self.settings),
            Some(MultiLineStr::In) | Some(MultiLineStr::Close) | Some(MultiLineStr::CloseOpen) => {
                HlTokens::in_quote(line, &self.settings)
            }
        };
        if let Some(mls) = mls {
            self.ml_str.insert(self.curr_line, mls.clone());
            match mls {
                MultiLineStr::Close => self.ml_str.remove(&(self.curr_line + 1)),
                MultiLineStr::Open | MultiLineStr::In | MultiLineStr::CloseOpen => {
                    self.ml_str.insert(self.curr_line + 1, MultiLineStr::In)
                }
            };
        } else {
            self.ml_str.remove(&self.curr_line);
        }
        self.curr_line += 1;
        Box::new(tk)
    }
    fn current_line(&self) -> usize {
        self.curr_line
    }
}
