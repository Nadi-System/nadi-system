use crate::parser::string::parse_string;
use crate::parser::NadiError;
use crate::parser::{ParseError as TaskParseError, ParseErrorType};
use crate::tasks::TaskKeyword;
use colored::Colorize;
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, char, digit1, one_of},
    combinator::{cut, map, opt, recognize},
    error::{context, VerboseError},
    multi::{many0, many1},
    sequence::{pair, preceded, terminated, tuple},
    IResult,
};
use std::str::FromStr;

#[derive(Clone, PartialEq, Debug)]
pub struct TokenError {
    pub line: usize,
    pub col: usize,
    pub linestr: String,
}

impl std::error::Error for TokenError {}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "TokenError: Error at line {} col {}",
            self.line, self.col
        )
    }
}

impl NadiError for TokenError {
    fn user_msg(&self, filename: Option<&str>) -> String {
        let mut msg = String::new();
        msg.push_str(&format!(
            "{}: Invalid Token at Line {} Column {}\n",
            "TokenError".bright_red(),
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
        let off = self.col - 1;
        msg.push_str(&format!(
            "  {}{}\n",
            &self.linestr[..off],
            self.linestr[off..].bright_red()
        ));
        msg.push_str(&format!(
            "  {: >2$} {}",
            "^".yellow(),
            "invalid token".yellow(),
            self.col
        ));
        msg
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Token<'a> {
    pub ty: TaskToken,
    pub content: &'a str,
}

impl<'a> Token<'a> {
    fn new(ty: TaskToken, content: &'a str) -> Self {
        Self { ty, content }
    }
}

pub struct VecTokens<'a> {
    tokens: Vec<Token<'a>>,
    pub line: usize,
    pub colstart: usize,
    colend: usize,
    linestr: String,
}

impl<'a> Iterator for VecTokens<'a> {
    type Item = Token<'a>;
    fn next(&mut self) -> Option<Token<'a>> {
        let t = self.tokens.pop()?;
        if t.ty == TaskToken::NewLine {
            self.line += 1;
            self.colstart = 0;
            self.colend = 0;
            self.linestr.clear();
        } else {
            self.colstart = self.colend;
            self.colend += t.content.len();
            self.linestr.push_str(&t.content);
        }
        Some(t)
    }
}

impl<'a> VecTokens<'a> {
    pub fn new(tokens: Vec<Token<'a>>) -> Self {
        Self {
            tokens: tokens.into_iter().rev().collect(),
            line: 0,
            colstart: 0,
            colend: 0,
            linestr: String::new(),
        }
    }

    pub fn peek_next(&self) -> Option<&Token<'a>> {
        self.tokens.iter().rev().next()
    }

    pub fn next_no_ws(&mut self, newline: bool) -> Option<Token<'a>> {
        loop {
            let t = self.next()?;
            match t.ty {
                TaskToken::Comment | TaskToken::WhiteSpace => (),
                TaskToken::NewLine if newline => (),
                _ => return Some(t),
            }
        }
    }

    pub fn peek_next_no_ws(&self, newline: bool) -> Option<&Token<'a>> {
        for t in self.tokens.iter().rev() {
            match t.ty {
                TaskToken::Comment | TaskToken::WhiteSpace => (),
                TaskToken::NewLine if newline => (),
                _ => return Some(t),
            }
        }
        None
    }

    pub fn next_if(&mut self, token: TaskToken) -> Option<Token<'a>> {
        let t = self.peek_next()?;
        if t.ty == token {
            self.next()
        } else {
            None
        }
    }

    pub fn next_no_ws_if(&mut self, newline: bool, token: TaskToken) -> Option<Token<'a>> {
        let t = self.peek_next_no_ws(newline)?;
        if t.ty == token {
            self.next_no_ws(newline)
        } else {
            None
        }
    }

    pub fn linestr_eol(&self) -> String {
        let mut linestr = self.linestr.clone();
        for t in self.tokens.iter().rev() {
            if t.ty == TaskToken::NewLine {
                break;
            }
            linestr.push_str(&t.content);
        }
        linestr
    }

    pub fn parse_error(&self, ty: ParseErrorType) -> TaskParseError {
        TaskParseError {
            ty,
            line: self.line,
            col: self.colstart,
            linestr: self.linestr_eol(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskToken {
    NewLine,
    WhiteSpace,
    Comment,
    Keyword(TaskKeyword),
    AngleStart,   // <>
    ParenStart,   // ()
    BraceStart,   // {}
    BracketStart, // []
    PathSep,      // ->
    Comma,
    Dot,
    And,
    Or,
    Not,
    AngleEnd,
    ParenEnd,
    BraceEnd,
    BracketEnd,
    Variable,
    Function,
    Assignment,
    Bool,
    String(String), // might need new value instead of slice (think escape seq)
    Integer,
    Float,
    Date,
    Time,
    DateTime,
    Quote, // strings within "" include the quote in
           // String token, this is for single quote,
           // that is not matched
}

impl TaskToken {
    pub fn syntax_color(&self) -> &'static str {
        match self {
            TaskToken::NewLine | TaskToken::WhiteSpace => "white",
            TaskToken::Comment => "gray",
            TaskToken::Keyword(_) => "red",
            TaskToken::AngleStart => "blue",
            TaskToken::ParenStart => "blue",
            TaskToken::BraceStart => "blue",
            TaskToken::BracketStart => "blue",
            TaskToken::PathSep => "blue",
            TaskToken::Comma => "blue",
            TaskToken::Dot => "blue",
            TaskToken::And => "yellow",
            TaskToken::Or => "yellow",
            TaskToken::Not => "yellow",
            TaskToken::AngleEnd => "blue",
            TaskToken::ParenEnd => "blue",
            TaskToken::BraceEnd => "blue",
            TaskToken::BracketEnd => "blue",
            TaskToken::Variable => "green",
            TaskToken::Function => "magenta",
            TaskToken::Assignment => "blue",
            TaskToken::Bool => "yellow",
            TaskToken::String(_) => "yellow",
            TaskToken::Integer => "yellow",
            TaskToken::Float => "yellow",
            TaskToken::Date => "cyan",
            TaskToken::Time => "cyan",
            TaskToken::DateTime => "cyan",
            TaskToken::Quote => "red",
        }
    }
}

impl<'a> Token<'a> {
    pub fn colored_print(&self) {
        print!("{}", self.colored());
    }

    pub fn colored(&self) -> String {
        match self.ty {
            TaskToken::NewLine | TaskToken::WhiteSpace => format!("{}", self.content),
            TaskToken::Comment => format!("{}", self.content.truecolor(100, 100, 100)),
            TaskToken::Keyword(_) => format!("{}", self.content.red()),
            TaskToken::AngleStart => format!("{}", self.content.blue()),
            TaskToken::ParenStart => format!("{}", self.content.blue()),
            TaskToken::BraceStart => format!("{}", self.content.blue()),
            TaskToken::BracketStart => format!("{}", self.content.blue()),
            TaskToken::PathSep => format!("{}", self.content.blue()),
            TaskToken::Comma => format!("{}", self.content.blue()),
            TaskToken::Dot => format!("{}", self.content.blue()),
            TaskToken::And => format!("{}", self.content.yellow()),
            TaskToken::Or => format!("{}", self.content.yellow()),
            TaskToken::Not => format!("{}", self.content.yellow()),
            TaskToken::AngleEnd => format!("{}", self.content.blue()),
            TaskToken::ParenEnd => format!("{}", self.content.blue()),
            TaskToken::BraceEnd => format!("{}", self.content.blue()),
            TaskToken::BracketEnd => format!("{}", self.content.blue()),
            TaskToken::Variable => format!("{}", self.content.green()),
            TaskToken::Function => format!("{}", self.content.magenta()),
            TaskToken::Assignment => format!("{}", self.content.blue()),
            TaskToken::Bool => format!("{}", self.content.yellow()),
            TaskToken::String(_) => format!("{}", self.content.yellow()),
            TaskToken::Integer => format!("{}", self.content.yellow()),
            TaskToken::Float => format!("{}", self.content.yellow()),
            TaskToken::Date => format!("{}", self.content.cyan()),
            TaskToken::Time => format!("{}", self.content.cyan()),
            TaskToken::DateTime => format!("{}", self.content.cyan()),
            TaskToken::Quote => format!("{}", self.content.red()),
        }
    }

    pub fn attribute(&self) -> Result<Option<Attribute>, &'static str> {
        let val = match self.ty {
            TaskToken::Bool => match self.content {
                "true" => true,
                "false" => false,
                _ => return Err("Boolean can only be true or false"),
            }
            .into(),
            TaskToken::String(ref s) => s.to_string().into(),
            TaskToken::Integer => self
                .content
                .parse::<i64>()
                .map_err(|_| "Invalid Integer")?
                .into(),
            TaskToken::Float => self
                .content
                .parse::<f64>()
                .map_err(|_| "Invalid Float")?
                .into(),
            TaskToken::Date => Attribute::Date(Date::from_str(self.content)?),
            TaskToken::Time => Attribute::Time(Time::from_str(self.content)?),
            TaskToken::DateTime => Attribute::DateTime(DateTime::from_str(self.content)?),
            _ => return Ok(None),
        };
        Ok(Some(val))
    }
}

pub(crate) type TokenRes<'a> = IResult<&'a str, Token<'a>, VerboseError<&'a str>>;

pub(crate) type VecTokenRes<'a> = IResult<&'a str, Vec<Token<'a>>, VerboseError<&'a str>>;

fn whitespace<'a>(i: &'a str) -> TokenRes<'a> {
    map(recognize(many1(alt((tag("\t"), tag(" "))))), |s| {
        Token::new(TaskToken::WhiteSpace, s)
    })(i)
}

fn newline<'a>(i: &'a str) -> TokenRes<'a> {
    // only unix, mac and windows line end supported for now
    map(alt((tag("\n\r"), tag("\r\n"), tag("\n"))), |s| {
        Token::new(TaskToken::NewLine, s)
    })(i)
}

fn comment<'a>(i: &'a str) -> TokenRes<'a> {
    map(recognize(pair(tag("#"), many0(is_not("\n\r")))), |s| {
        Token::new(TaskToken::Comment, s)
    })(i)
}

fn symbols<'a>(i: &'a str) -> TokenRes<'a> {
    alt((
        map(tag("<"), |s| Token::new(TaskToken::AngleStart, s)),
        map(tag(">"), |s| Token::new(TaskToken::AngleEnd, s)),
        map(tag("("), |s| Token::new(TaskToken::ParenStart, s)),
        map(tag(")"), |s| Token::new(TaskToken::ParenEnd, s)),
        map(tag("["), |s| Token::new(TaskToken::BracketStart, s)),
        map(tag("]"), |s| Token::new(TaskToken::BracketEnd, s)),
        map(tag("{"), |s| Token::new(TaskToken::BraceStart, s)),
        map(tag("}"), |s| Token::new(TaskToken::BraceEnd, s)),
        map(tag("."), |s| Token::new(TaskToken::Dot, s)),
        map(tag(","), |s| Token::new(TaskToken::Comma, s)),
        map(tag("->"), |s| Token::new(TaskToken::PathSep, s)),
        map(tag("="), |s| Token::new(TaskToken::Assignment, s)),
        map(tag("&"), |s| Token::new(TaskToken::And, s)),
        map(tag("|"), |s| Token::new(TaskToken::Or, s)),
        map(tag("!"), |s| Token::new(TaskToken::Not, s)),
        map(tag("\""), |s| Token::new(TaskToken::Quote, s)),
    ))(i)
}

pub fn valid_variable_name(txt: &str) -> bool {
    match variable(txt) {
        Ok((res, _)) => res.trim().is_empty(),
        _ => false,
    }
}

fn variable<'a>(i: &'a str) -> TokenRes<'a> {
    let mut get_var = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(pair(opt(tag("-")), many1(alt((alphanumeric1, tag("_")))))),
    ));
    let (mut rest, mut var) = get_var(i)?;
    let ty = match TaskKeyword::from_str(var) {
        Ok(kw) => TaskToken::Keyword(kw),
        Err(_) => {
            if rest.trim_start().starts_with('(') {
                TaskToken::Function
            } else {
                if let Some(re) = rest.trim_start().strip_prefix('.') {
                    let (r, _) = get_var(re)?;
                    if r.trim_start().starts_with('(') {
                        rest = r;
                        var = &i[..(i.len() - r.len())];
                        TaskToken::Function
                    } else {
                        TaskToken::Variable
                    }
                } else {
                    TaskToken::Variable
                }
            }
        }
    };
    Ok((rest, Token::new(ty, var)))
}

fn string<'a>(i: &'a str) -> TokenRes<'a> {
    let (rest, s) = context("string", parse_string)(i)?;
    Ok((
        rest,
        Token::new(TaskToken::String(s), &i[..(i.len() - rest.len())]),
    ))
}

fn boolean<'a>(i: &'a str) -> TokenRes<'a> {
    map(alt((tag("true"), tag("false"))), |s| {
        Token::new(TaskToken::Bool, s)
    })(i)
}

fn integer<'a>(i: &'a str) -> TokenRes<'a> {
    map(
        alt((
            recognize(tuple((
                one_of("+-"),
                many1(terminated(digit1, many0(char('_')))),
            ))),
            recognize(many1(terminated(digit1, many0(char('_'))))),
        )),
        |s| Token::new(TaskToken::Integer, s),
    )(i)
}

fn float<'a>(i: &'a str) -> TokenRes<'a> {
    map(
        alt((
            recognize(tuple((
                integer,
                preceded(char('.'), cut(digit1)),
                opt(tuple((one_of("eE"), integer))),
            ))),
            // even if there is no decimal 1e10 is float.
            recognize(tuple((
                integer,
                opt(preceded(char('.'), cut(digit1))),
                tuple((one_of("eE"), integer)),
            ))),
        )),
        |s| Token::new(TaskToken::Float, s),
    )(i)
}

fn date<'a>(i: &'a str) -> TokenRes<'a> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char('-')))), digit1))),
        |s| Token::new(TaskToken::Date, s),
    )(i)
}

fn time<'a>(i: &'a str) -> TokenRes<'a> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char(':')))), digit1))),
        |s| Token::new(TaskToken::Time, s),
    )(i)
}

fn datetime<'a>(i: &'a str) -> TokenRes<'a> {
    map(recognize(tuple((date, one_of(" T"), time))), |s| {
        Token::new(TaskToken::DateTime, s)
    })(i)
}

fn task_script<'a>(i: &'a str) -> VecTokenRes<'a> {
    context(
        "task script",
        many0(alt((
            whitespace, newline, comment, string, datetime, date, time, boolean, float, integer,
            variable, symbols,
        ))),
    )(i)
}

pub fn get_tokens(txt: &str) -> Result<Vec<Token>, TokenError> {
    let (res, tokens) = match task_script(txt) {
        Ok(v) => v,
        Err(e) => {
            let er = match e {
                nom::Err::Error(e) | nom::Err::Failure(e) => e,
                _ => panic!("incomplete error shouldn't happen"),
            };
            let pre = er.errors.iter().next().unwrap().0;
            let off = txt.len() - pre.len();
            // if pre.is_empty() {
            //     txt.len()
            // } else {
            // let s = txt.as_ptr();
            // let e = pre.as_ptr();
            // e as usize - s as usize
            // };
            let pre = &txt[..off];
            let res = &txt[off..];
            let line = pre.lines().count() - 1;
            let linestr = txt.lines().nth(line).unwrap_or_default().to_string();
            let col = linestr.len() - res.lines().next().unwrap_or_default().len() + 1;
            return Err(TokenError { line, col, linestr });
        }
    };
    if res.is_empty() {
        Ok(tokens)
    } else {
        let line = txt.lines().count() - res.lines().count();
        let linestr = txt.lines().nth(line).unwrap_or_default().to_string();
        let col = linestr.len() - res.lines().next().unwrap_or_default().len() + 1;
        Err(TokenError { line, col, linestr })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("1990-12-21", TaskToken::Date, "")]
    fn date_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = date(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }
}
