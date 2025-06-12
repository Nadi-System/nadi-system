use crate::parser::tokenizer::{TaskToken, Token};
use colored::Colorize;
use nom::error::ErrorKind;

#[derive(Debug, PartialEq, Clone, Default)]
pub struct ParseError {
    pub ty: ParseErrorType,
    pub line: usize,
    pub col: usize,
    pub linestr: String,
}

impl ParseError {
    pub fn new(tokens: &[Token<'_>], rest: &[Token<'_>], ty: ParseErrorType) -> Self {
        let tokens = &tokens[..(tokens.len() - rest.len())];
        let mut line = 1;
        let mut lstart = 0;
        for (i, t) in tokens.iter().enumerate() {
            if t.ty == TaskToken::NewLine {
                line += 1;
                lstart = i + 1;
            }
        }
        let mut curr_line: Vec<_> = tokens[lstart..].iter().collect();
        let col = curr_line.iter().map(|t| t.content.len()).sum::<usize>() + 1;
        for t in rest {
            if t.ty == TaskToken::NewLine {
                break;
            } else {
                curr_line.push(t);
            }
        }
        let mut linestr = String::new();
        curr_line.iter().for_each(|t| linestr.push_str(t.content));
        Self {
            ty,
            line,
            col,
            linestr,
        }
    }
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

impl ParseError {
    pub fn custom(msg: String) -> Self {
        Self {
            ty: ParseErrorType::Custom(msg),
            ..Default::default()
        }
    }

    pub fn user_msg(&self, filename: Option<&str>) -> String {
        let mut msg = String::new();
        if let ParseErrorType::Custom(m) = &self.ty {
            msg.push_str(m);
            if let Some(fname) = filename {
                msg.push_str(&format!("  -> {fname}\n"));
            }
        } else {
            msg.push_str(&format!(
                "Error: Parse Error at Line {} Column {}\n",
                self.line, self.col
            ));
            if let Some(fname) = filename {
                msg.push_str(&format!("  -> {}:{}:{}\n", fname, self.line, self.col));
            }
            msg.push_str(&format!("  {}\n", self.linestr));
            msg.push_str(&format!("  {: >2$} {}", "^", self.ty.message(), self.col));
        }
        msg
    }

    pub fn user_msg_color(&self, filename: Option<&str>) -> String {
        let mut msg = String::new();
        if let ParseErrorType::Custom(m) = &self.ty {
            msg.push_str(m);
            if let Some(fname) = filename {
                msg.push_str(&format!("  {} {}\n", "->".blue(), fname.blue()));
            }
        } else {
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
                self.col
            ));
        }
        msg
    }
}

#[derive(Debug, PartialEq, Clone, Default)]
pub enum ParseErrorType {
    LogicalError(&'static str),
    ValueError(&'static str),
    InvalidLineStart,
    Unclosed(&'static str),
    Incomplete,
    IncompletePath,
    IncompleteExpression,
    InvalidPropagation,
    InvalidKeyword,
    PropagationNotSupported,
    KeywordArgBeforePositional,
    KeywordNotVariable,
    #[default]
    SyntaxError,
    InvalidFunctionParameters,
    MissingValue,
    ExpectedPath,
    InvalidToken,
    TokenMismatch,
    MultipleOutput(String),
    Custom(String),
}

impl ParseErrorType {
    pub fn message(&self) -> String {
        match self {
            Self::LogicalError(v) => {
                return format!("Unexpected Logic problem: {}, please contact dev", v.red())
            }
            Self::ValueError(v) => return format!("Invalid Value: {v}"),
            Self::InvalidLineStart => "Lines should start with a keyword",
            Self::Unclosed(s) => return format!("Missing closing token {s:?}"),
            Self::Incomplete => "Incomplete Input",
            Self::IncompletePath => "Incomplete Path; expected node here",
            Self::IncompleteExpression => "Incomplete Expression",
            Self::InvalidPropagation => "Invalid propagation value",
            Self::InvalidKeyword => "Invalid keyword at this location",
            Self::PropagationNotSupported => "Propagation not supported here",
            Self::KeywordArgBeforePositional => "Positional Argument cannot come after keyword",
            Self::KeywordNotVariable => "Keywords cannot be used as variables",
            Self::SyntaxError => "Invalid Syntax",
            Self::InvalidFunctionParameters => "Invalid function parameters",
            Self::MissingValue => "Missing Value",
            Self::ExpectedPath => "Expected Path symbol here",
            Self::InvalidToken => "Unsupported Token",
            Self::TokenMismatch => "Unexpected Token",
            Self::MultipleOutput(msg) => return format!("Multiple output not supported: {msg}"),
            Self::Custom(msg) => msg.as_str(),
        }
        .to_string()
    }
}

#[derive(Debug)]
pub struct MatchErr<'a, 'b> {
    pub ty: ParseErrorType,
    pub internal: nom::error::Error<&'a [Token<'b>]>,
}

impl<'a, 'b> MatchErr<'a, 'b> {
    pub fn new(inp: &'a [Token<'b>]) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal: nom::error::Error::new(inp, ErrorKind::Tag),
        }
    }

    pub fn from_nom(internal: nom::error::Error<&'a [Token<'b>]>) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal,
        }
    }

    pub fn ty(mut self, ty: &ParseErrorType) -> Self {
        self.ty = ty.clone();
        self
    }
}

impl<'a, 'b> nom::error::ParseError<&'a [Token<'b>]> for MatchErr<'a, 'b> {
    fn from_error_kind(input: &'a [Token<'b>], kind: ErrorKind) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal: nom::error::Error::<&'a [Token<'b>]>::from_error_kind(input, kind),
        }
    }
    // what does it do?
    fn append(input: &'a [Token<'b>], kind: ErrorKind, other: Self) -> Self {
        MatchErr {
            ty: other.ty,
            internal: nom::error::Error::<&'a [Token<'b>]>::append(input, kind, other.internal),
        }
    }

    // Provided methods
    fn from_char(input: &'a [Token<'b>], c: char) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal: nom::error::Error::<&'a [Token<'b>]>::from_char(input, c),
        }
    }
    fn or(self, other: Self) -> Self {
        MatchErr {
            ty: self.ty,
            internal: nom::error::Error::<&'a [Token<'b>]>::or(other.internal, self.internal),
        }
    }
}
