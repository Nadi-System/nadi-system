use crate::parser::string::parse_string;
use crate::parser::{ParseError as TaskParseError, ParseErrorType};
use crate::tasks::TaskKeyword;
use colored::Colorize;
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, anychar, char, digit1, one_of},
    combinator::{map, opt, recognize},
    error::{context, VerboseError},
    multi::{many0, many1},
    sequence::{pair, preceded, terminated, tuple},
    IResult,
};
use std::str::FromStr;

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

pub fn check_tokens(tokens: &[Token]) -> Result<(), TaskParseError> {
    let mut data = tokens.split(|t| !t.ty.is_valid());
    let valid = data.next().unwrap();
    match data.next() {
        Some(_) => Err(TaskParseError::new(
            tokens,
            &tokens[valid.len()..],
            ParseErrorType::InvalidToken,
        )),
        None => Ok(()),
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
    Comma,        // ,
    Caret,        // ^
    Dash,         // -
    Plus,         // +
    Star,         // *
    Slash,        // /
    Percentage,   // %
    Question,     // ?
    Semicolon,    // ;
    Dot,          // .
    And,          // &
    Or,           // |
    Not,          // !
    AngleEnd,
    ParenEnd,
    BraceEnd,
    BracketEnd,
    Variable,
    Function,
    Assignment,
    None, // <None>
    Bool,
    String(String), // might need new value instead of slice (think escape seq)
    Integer,
    Float,
    Date,
    Time,
    DateTime,
    NaN,
    Infinity,
    Invalid(char), // any invalid characters
}

impl TaskToken {
    pub fn is_valid(&self) -> bool {
        match self {
            Self::Invalid(_) => false,
            // none is only there for highlight of output
            Self::None => false,
            _ => true,
        }
    }

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
            TaskToken::Caret => "blue",
            TaskToken::Dot => "blue",
            TaskToken::Dash => "white",
            TaskToken::Plus => "white",
            TaskToken::Star => "white",
            TaskToken::Slash => "white",
            TaskToken::Percentage => "white",
            TaskToken::Question => "yellow",
            TaskToken::Semicolon => "yellow",
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
            TaskToken::None => "gray",
            TaskToken::String(_) => "yellow",
            TaskToken::Integer => "yellow",
            TaskToken::Float => "yellow",
            TaskToken::Date => "cyan",
            TaskToken::Time => "cyan",
            TaskToken::DateTime => "cyan",
            TaskToken::NaN => "yellow",
            TaskToken::Infinity => "yellow",
            TaskToken::Invalid(_) => "red",
        }
    }
}

impl Token<'_> {
    pub fn colored_print(&self) {
        print!("{}", self.colored());
    }

    pub fn colored(&self) -> String {
        match self.ty {
            TaskToken::NewLine | TaskToken::WhiteSpace => self.content.to_string(),
            TaskToken::Comment => format!("{}", self.content.truecolor(100, 100, 100)),
            TaskToken::Keyword(_) => format!("{}", self.content.red()),
            TaskToken::AngleStart => format!("{}", self.content.blue()),
            TaskToken::ParenStart => format!("{}", self.content.blue()),
            TaskToken::BraceStart => format!("{}", self.content.blue()),
            TaskToken::BracketStart => format!("{}", self.content.blue()),
            TaskToken::PathSep => format!("{}", self.content.blue()),
            TaskToken::Comma => format!("{}", self.content.blue()),
            TaskToken::Caret => format!("{}", self.content.blue()),
            TaskToken::Dot => format!("{}", self.content.blue()),
            TaskToken::Dash => format!("{}", self.content.blue()),
            TaskToken::Plus => self.content.to_string(),
            TaskToken::Star => self.content.to_string(),
            TaskToken::Slash => self.content.to_string(),
            TaskToken::Percentage => self.content.to_string(),
            TaskToken::Question => format!("{}", self.content.yellow()),
            TaskToken::Semicolon => format!("{}", self.content.yellow()),
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
            TaskToken::None => format!("{}", self.content.truecolor(100, 100, 100)),
            TaskToken::Bool => format!("{}", self.content.yellow()),
            TaskToken::String(_) => format!("{}", self.content.yellow()),
            TaskToken::Integer => format!("{}", self.content.yellow()),
            TaskToken::Float => format!("{}", self.content.yellow()),
            TaskToken::Date => format!("{}", self.content.cyan()),
            TaskToken::Time => format!("{}", self.content.cyan()),
            TaskToken::DateTime => format!("{}", self.content.cyan()),
            TaskToken::NaN => format!("{}", self.content.yellow()),
            TaskToken::Infinity => format!("{}", self.content.yellow()),
            TaskToken::Invalid(_) => format!("{}", self.content.red()),
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
            TaskToken::NaN => Attribute::Float(f64::NAN),
            TaskToken::Infinity => Attribute::Float(f64::INFINITY),
            _ => return Ok(None),
        };
        Ok(Some(val))
    }
}

pub(crate) type TokenRes<'a> = IResult<&'a str, Token<'a>, VerboseError<&'a str>>;

pub(crate) type VecTokenRes<'a> = IResult<&'a str, Vec<Token<'a>>, VerboseError<&'a str>>;

// catch all one char token
fn invalid(i: &str) -> TokenRes<'_> {
    map(anychar, |c| {
        Token::new(TaskToken::Invalid(c), {
            // so that we split at utf-8 boundary, since the utf-8
            // char is invalid we don't care about the actual char
            match i.char_indices().nth(1) {
                Some((ind, _)) => &i[..ind],
                None => i,
            }
        })
    })(i)
}

fn whitespace(i: &str) -> TokenRes<'_> {
    map(recognize(many1(alt((tag("\t"), tag(" "))))), |s| {
        Token::new(TaskToken::WhiteSpace, s)
    })(i)
}

fn newline(i: &str) -> TokenRes<'_> {
    // only unix, mac and windows line end supported for now
    map(alt((tag("\n\r"), tag("\r\n"), tag("\n"))), |s| {
        Token::new(TaskToken::NewLine, s)
    })(i)
}

fn comment(i: &str) -> TokenRes<'_> {
    map(recognize(pair(tag("#"), many0(is_not("\n\r")))), |s| {
        Token::new(TaskToken::Comment, s)
    })(i)
}

fn none(i: &str) -> TokenRes<'_> {
    map(tag("<None>"), |s| Token::new(TaskToken::Caret, s))(i)
}

fn operators(i: &str) -> TokenRes<'_> {
    alt((
        map(tag("^"), |s| Token::new(TaskToken::Caret, s)),
        map(tag("-"), |s| Token::new(TaskToken::Dash, s)),
        map(tag("+"), |s| Token::new(TaskToken::Plus, s)),
        map(tag("*"), |s| Token::new(TaskToken::Star, s)),
        map(tag("/"), |s| Token::new(TaskToken::Slash, s)),
        map(tag("%"), |s| Token::new(TaskToken::Percentage, s)),
        map(tag("="), |s| Token::new(TaskToken::Assignment, s)),
        map(tag("&"), |s| Token::new(TaskToken::And, s)),
        map(tag("|"), |s| Token::new(TaskToken::Or, s)),
        map(tag("!"), |s| Token::new(TaskToken::Not, s)),
    ))(i)
}

fn symbols(i: &str) -> TokenRes<'_> {
    alt((
        map(tag("->"), |s| Token::new(TaskToken::PathSep, s)),
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
        map(tag("?"), |s| Token::new(TaskToken::Question, s)),
        map(tag(";"), |s| Token::new(TaskToken::Semicolon, s)),
    ))(i)
}

pub fn valid_variable_name(txt: &str) -> bool {
    match variable(txt) {
        Ok((res, _)) => res.trim().is_empty(),
        _ => false,
    }
}

fn variable(i: &str) -> TokenRes<'_> {
    let mut get_var = recognize(pair(
        alt((alpha1, tag("_"))),
        many0(alt((alphanumeric1, tag("_")))),
    ));
    let (mut rest, mut var) = get_var(i)?;
    let ty = match TaskKeyword::from_str(var) {
        Ok(kw) => TaskToken::Keyword(kw),
        Err(_) => {
            if rest.trim_start().starts_with('(') {
                TaskToken::Function
            } else if let Some(re) = rest.trim_start().strip_prefix('.') {
                // .var or ."var" is supported
                if re.trim_start().starts_with('"') {
                    TaskToken::Variable
                } else {
                    let (r, _) = get_var(re)?;
                    if r.trim_start().starts_with('(') {
                        rest = r;
                        var = &i[..(i.len() - r.len())];
                        TaskToken::Function
                    } else {
                        TaskToken::Variable
                    }
                }
            } else {
                match var {
                    "nan" => TaskToken::NaN,
                    "inf" => TaskToken::Infinity,
                    _ => TaskToken::Variable,
                }
            }
        }
    };
    Ok((rest, Token::new(ty, var)))
}

fn string(i: &str) -> TokenRes<'_> {
    let (rest, s) = context("string", parse_string)(i)?;
    Ok((
        rest,
        Token::new(TaskToken::String(s), &i[..(i.len() - rest.len())]),
    ))
}

fn boolean(i: &str) -> TokenRes<'_> {
    map(alt((tag("true"), tag("false"))), |s| {
        Token::new(TaskToken::Bool, s)
    })(i)
}

fn integer(i: &str) -> TokenRes<'_> {
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

fn float(i: &str) -> TokenRes<'_> {
    map(
        alt((
            recognize(tuple((
                integer,
                preceded(char('.'), digit1),
                opt(tuple((one_of("eE"), integer))),
            ))),
            // even if there is no decimal 1e10 is float.
            recognize(tuple((
                integer,
                opt(preceded(char('.'), digit1)),
                tuple((one_of("eE"), integer)),
            ))),
        )),
        |s| Token::new(TaskToken::Float, s),
    )(i)
}

fn date(i: &str) -> TokenRes<'_> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char('-')))), digit1))),
        |s| Token::new(TaskToken::Date, s),
    )(i)
}

fn time(i: &str) -> TokenRes<'_> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char(':')))), digit1))),
        |s| Token::new(TaskToken::Time, s),
    )(i)
}

fn datetime(i: &str) -> TokenRes<'_> {
    map(recognize(tuple((date, one_of(" T"), time))), |s| {
        Token::new(TaskToken::DateTime, s)
    })(i)
}

fn task_token(i: &str) -> TokenRes<'_> {
    alt((
        whitespace, newline, comment, string, datetime, date, time, boolean, float, integer,
        variable, none, symbols, operators, invalid,
    ))(i)
}

fn task_script(i: &str) -> VecTokenRes<'_> {
    context("task script", many0(task_token))(i)
}

pub fn get_tokens(txt: &str) -> Vec<Token> {
    let (res, tokens) = task_script(txt).expect("Parser shouldn't error out");
    if !res.is_empty() {
        panic!("Logic Error on Parser, there shouldn't be anything left")
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest] // whitespace
    #[case(" ", TaskToken::WhiteSpace, "")]
    #[case("\t", TaskToken::WhiteSpace, "")] // tab whitespace
    #[case("   \n", TaskToken::WhiteSpace, "\n")] // multiple spaces
    fn whitespace_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = whitespace(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest]
    #[case("# comment", TaskToken::Comment, "")]
    #[case("# comment\n", TaskToken::Comment, "\n")]
    #[case("# comment\n123", TaskToken::Comment, "\n123")]
    fn comment_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = comment(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest]
    #[case("->", TaskToken::PathSep, "")]
    fn symbols_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = symbols(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest]
    #[case("var", TaskToken::Variable, "")]
    #[case("x", TaskToken::Variable, "")]
    #[case("_xyz12", TaskToken::Variable, "")]
    #[case("xyz_12_z", TaskToken::Variable, "")]
    #[case("var()", TaskToken::Function, "()")]
    #[case("x.var(ab)", TaskToken::Function, "(ab)")]
    #[case("node", TaskToken::Keyword(TaskKeyword::Node), "")]
    #[should_panic]
    #[case("12_z", TaskToken::Variable, "")]
    fn variable_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = variable(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // newline
    #[case("\n", TaskToken::NewLine, "")]
    #[should_panic]
    #[case("\\\n", TaskToken::NewLine, "")] // escaped newline should be escaped
    #[case("\n   ", TaskToken::NewLine, "   ")]
    fn newline_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = newline(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // string
    #[case("\"hello world\"", TaskToken::String(String::from("hello world")), "")]
    #[case("\"\"'", TaskToken::String(String::from("")), "'")]

    fn string_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = string(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // boolean
    #[case("true", TaskToken::Bool, "")]
    #[case("false", TaskToken::Bool, "")]
    #[should_panic]
    #[case("nil", TaskToken::Bool, "")]
    fn boolean_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = boolean(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // integer
    #[case("12_300", TaskToken::Integer, "")]
    #[case("123", TaskToken::Integer, "")]
    #[case("-123", TaskToken::Integer, "")]
    fn integer_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = integer(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // float
    #[case("12_000.34", TaskToken::Float, "")]
    #[case("12.34", TaskToken::Float, "")]
    #[case("-123.45", TaskToken::Float, "")]
    fn float_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = float(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // date
    #[case("1990-12-21", TaskToken::Date, "")]
    fn date_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = date(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // time
    #[case("14:30", TaskToken::Time, "")]
    #[case("14:30:32", TaskToken::Time, "")]
    fn time_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = time(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest] // datetime
    #[case("1990-12-21 14:30", TaskToken::DateTime, "")]
    #[case("1990-12-21T14:30", TaskToken::DateTime, "")]
    #[case("1990-12-21 14:30:32", TaskToken::DateTime, "")]
    #[case("1990-12-21T14:30:32", TaskToken::DateTime, "")]
    fn datetime_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = datetime(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    #[rstest]
    #[case("~", TaskToken::Invalid('~'), "")]
    #[case("~12", TaskToken::Invalid('~'), "12")]
    #[case("@~", TaskToken::Invalid('@'), "~")]
    fn invalid_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = invalid(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }

    // combined all the test cases from above into a single one
    #[rstest]
    #[case(" ", TaskToken::WhiteSpace, "")]
    #[case("\t", TaskToken::WhiteSpace, "")]
    #[case("   \n", TaskToken::WhiteSpace, "\n")]
    #[case("# comment", TaskToken::Comment, "")]
    #[case("# comment\n", TaskToken::Comment, "\n")]
    #[case("# comment\n123", TaskToken::Comment, "\n123")]
    #[case("->", TaskToken::PathSep, "")]
    #[case("var", TaskToken::Variable, "")]
    #[case("x", TaskToken::Variable, "")]
    #[case("_xyz12", TaskToken::Variable, "")]
    #[case("xyz_12_z", TaskToken::Variable, "")]
    #[case("var()", TaskToken::Function, "()")]
    #[case("x.var(ab)", TaskToken::Function, "(ab)")]
    #[case("node", TaskToken::Keyword(TaskKeyword::Node), "")]
    #[case("12_z", TaskToken::Integer, "z")] // coz _ are ignored in numbers
    #[case("\n", TaskToken::NewLine, "")]
    #[case("\\\n", TaskToken::Invalid('\\'), "\n")]
    #[case("\n   ", TaskToken::NewLine, "   ")]
    #[case("\"hello world\"", TaskToken::String(String::from("hello world")), "")]
    #[case("\"\"'", TaskToken::String(String::from("")), "'")]
    #[case("true", TaskToken::Bool, "")]
    #[case("false", TaskToken::Bool, "")]
    #[case("nil", TaskToken::Variable, "")]
    #[case("12_300", TaskToken::Integer, "")]
    #[case("123", TaskToken::Integer, "")]
    #[case("-123", TaskToken::Integer, "")]
    #[case("12_000.34", TaskToken::Float, "")]
    #[case("12.34", TaskToken::Float, "")]
    #[case("-123.45", TaskToken::Float, "")]
    #[case("1990-12-21", TaskToken::Date, "")]
    #[case("14:30", TaskToken::Time, "")]
    #[case("14:30:32", TaskToken::Time, "")]
    #[case("1990-12-21 14:30", TaskToken::DateTime, "")]
    #[case("1990-12-21T14:30", TaskToken::DateTime, "")]
    #[case("1990-12-21 14:30:32", TaskToken::DateTime, "")]
    #[case("1990-12-21T14:30:32", TaskToken::DateTime, "")]
    #[case("~", TaskToken::Invalid('~'), "")]
    #[case("~12", TaskToken::Invalid('~'), "12")]
    #[case("@~", TaskToken::Invalid('@'), "~")]
    fn task_token_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = task_token(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }
}
