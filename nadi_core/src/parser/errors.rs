use colored::Colorize;

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
    Incomplete,
    InvalidPropagation,
    InvalidKeyword,
    PropagationNotSupported,
    KeywordArgBeforePositional,
    KeywordNotVariable,
    SyntaxError,
    InvalidToken,
    TokenMismatch,
}

impl ParseErrorType {
    pub fn message(&self) -> String {
        match self {
            Self::LogicalError(v) => {
                return format!("Unexpected Logic problem: {v}, please contact dev")
            }
            Self::ValueError(v) => return format!("Invalid Value: {v}"),
            Self::InvalidLineStart => "Lines should start with a keyword",
            Self::Incomplete => "Incomplete Input",
            Self::InvalidPropagation => "Invalid propagation value",
            Self::InvalidKeyword => "Invalid keyword at this location",
            Self::PropagationNotSupported => "Propagation not supported here",
            Self::KeywordArgBeforePositional => "Positional Argument cannot come after keyword",
            Self::KeywordNotVariable => "Keywords cannot be used as variables",
            Self::SyntaxError => "Invalid Syntax",
            Self::InvalidToken => "Unsupported Token",
            Self::TokenMismatch => "Unexpected Token",
        }
        .to_string()
    }
}

#[derive(Debug)]
pub struct MatchErr<'a, 'b> {
    ty: ParseErrorType,
    internal: nom::error::Error<&'a [Token<'b>]>,
}

impl<'a, 'b> MatchErr<'a, 'b> {
    fn new(inp: &'a [Token<'b>]) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal: nom::error::Error::new(inp, ErrorKind::Tag),
        }
    }

    fn from_nom(internal: nom::error::Error<&'a [Token<'b>]>) -> Self {
        MatchErr {
            ty: ParseErrorType::SyntaxError,
            internal,
        }
    }

    fn ty(mut self, ty: ParseErrorType) -> Self {
        self.ty = ty;
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
            ty: other.ty,
            internal: nom::error::Error::<&'a [Token<'b>]>::or(self.internal, other.internal),
        }
    }
}
