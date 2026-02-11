use crate::expressions::TaskPosition;
use crate::parser::highlight::{Highlight, NadiFileType};
use crate::parser::string::parse_string;
use crate::parser::{ParseError as TaskParseError, ParseErrorType};
use crate::tasks::TaskKeyword;
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    character::complete::{alpha1, alphanumeric1, anychar, char, digit1, one_of},
    combinator::{map, opt, recognize},
    error::{context, VerboseError},
    multi::{many0, many1},
    sequence::{delimited, pair, preceded, terminated, tuple},
    IResult,
};
use std::str::FromStr;

#[derive(Clone, PartialEq, Debug)]
pub struct Token<'a> {
    /// Token Type
    pub ty: TaskToken,
    /// Contents of the token
    pub content: &'a str,
    /// start position of token (line, col)
    pub start: (usize, usize),
}

impl<'a> TaskPosition for Token<'a> {
    fn position(&self) -> (usize, usize) {
        self.start
    }
}

impl<'a, 'b> TaskPosition for &'b [Token<'a>] {
    fn position(&self) -> (usize, usize) {
        if let Some(t) = self.first() {
            t.position()
        } else {
            (0, 0)
        }
    }
}

impl<'a> Token<'a> {
    fn new(raw: RawToken<'a>, start: (usize, usize)) -> Self {
        Self {
            ty: raw.ty,
            content: raw.content,
            start,
        }
    }
    pub fn validate(tokens: Vec<RawToken<'a>>) -> Result<Vec<Self>, TaskParseError> {
        let mut data = tokens.split(|t| !t.ty.is_valid());
        let valid = data.next().unwrap();
        if data.next().is_some() {
            return Err(TaskParseError::raw(
                &tokens,
                &tokens[valid.len()..],
                ParseErrorType::InvalidToken,
            ));
        }
        let mut line = 1;
        let mut col = 1;
        let tokens = tokens
            .into_iter()
            .map(|t| {
                let start = (line, col);
                if t.ty == TaskToken::NewLine {
                    line += 1;
                    col = 1;
                } else {
                    col += t.content.len();
                }
                Token::new(t, start)
            })
            .collect();
        Ok(tokens)
    }

    /// returns the function that is part of the current input
    pub fn last_function<'b>(_tokens: &'b [Token<'a>]) -> Option<&'b Token<'a>> {
        todo!()
    }
}

/// Checks if the parens/brackets/braces are properly paired up
pub enum ParenCheck<'a> {
    Paired,
    Unpaired(Vec<Token<'a>>),
    Invalid(Token<'a>, TaskToken),
    Extra(Token<'a>),
}

impl<'a> ParenCheck<'a> {
    /// checks if there is any unclosed brackets
    pub fn scan<'b>(tokens: &'b [Token<'a>]) -> Self {
        let mut stack = Vec::new();
        for tk in tokens {
            match tk.ty {
                TaskToken::ParenStart => stack.push(tk),
                TaskToken::BraceStart => stack.push(tk),
                TaskToken::BracketStart => stack.push(tk),
                TaskToken::ParenEnd => match stack.pop() {
                    Some(prev) => {
                        if !matches!(prev.ty, TaskToken::ParenStart) {
                            return Self::Invalid(tk.clone(), TaskToken::ParenEnd);
                        }
                    }
                    None => return Self::Extra(tk.clone()),
                },
                TaskToken::BraceEnd => match stack.pop() {
                    Some(prev) => {
                        if !matches!(prev.ty, TaskToken::BraceStart) {
                            return Self::Invalid(tk.clone(), TaskToken::BraceEnd);
                        }
                    }
                    None => return Self::Extra(tk.clone()),
                },
                TaskToken::BracketEnd => match stack.pop() {
                    Some(prev) => {
                        if !matches!(prev.ty, TaskToken::BracketStart) {
                            return Self::Invalid(tk.clone(), TaskToken::BracketEnd);
                        }
                    }
                    None => return Self::Extra(tk.clone()),
                },
                _ => (),
            }
        }
        if stack.is_empty() {
            Self::Paired
        } else {
            Self::Unpaired(stack.into_iter().cloned().collect())
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct RawToken<'a> {
    /// Token Type
    pub ty: TaskToken,
    /// Contents of the token
    pub content: &'a str,
}

impl<'a> RawToken<'a> {
    fn new(ty: TaskToken, content: &'a str) -> Self {
        Self { ty, content }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskToken {
    NewLine,
    WhiteSpace,
    Comment,
    Keyword(TaskKeyword),
    AngleStart,   // <
    ParenStart,   // (
    BraceStart,   // {
    BracketStart, // [
    PathSep,      // ->
    Comma,        // ,
    Caret,        // ^
    Dash,         // -
    Plus,         // +
    Star,         // *
    Slash,        // /
    Percentage,   // %
    Question,     // ?
    Colon,        // :
    Semicolon,    // ;
    Dot,          // .
    And,          // &
    Or,           // |
    Not,          // !
    AngleEnd,     // >
    ParenEnd,     // )
    BraceEnd,     // }
    BracketEnd,   // ]
    At,           // @
    Dollar,       // $
    Variable,
    Function,
    Assignment,
    None, // <None>
    Bool,
    String(String), // might need new value instead of slice (think escape seq)
    Template(String),
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

    pub fn highlight(&self) -> Highlight {
        Highlight::from_token(self, &NadiFileType::Tasks)
    }

    #[deprecated(note = "use .highlight().color()")]
    pub fn syntax_color(&self) -> &'static str {
        self.highlight().color()
    }
}

impl Token<'_> {
    pub fn colored_print(&self) {
        print!("{}", self.colored());
    }

    pub fn colored(&self) -> String {
        self.ty.highlight().colored(self.content)
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

pub(crate) type TokenRes<'a> = IResult<&'a str, RawToken<'a>, VerboseError<&'a str>>;

pub(crate) type VecTokenRes<'a> = IResult<&'a str, Vec<RawToken<'a>>, VerboseError<&'a str>>;

// catch all one char token
fn invalid(i: &str) -> TokenRes<'_> {
    map(anychar, |c| {
        RawToken::new(TaskToken::Invalid(c), {
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
        RawToken::new(TaskToken::WhiteSpace, s)
    })(i)
}

fn newline(i: &str) -> TokenRes<'_> {
    // only unix, mac and windows line end supported for now
    map(alt((tag("\n\r"), tag("\r\n"), tag("\n"))), |s| {
        RawToken::new(TaskToken::NewLine, s)
    })(i)
}

fn comment(i: &str) -> TokenRes<'_> {
    map(recognize(pair(tag("#"), many0(is_not("\n\r")))), |s| {
        RawToken::new(TaskToken::Comment, s)
    })(i)
}

fn none(i: &str) -> TokenRes<'_> {
    map(tag("<None>"), |s| RawToken::new(TaskToken::Caret, s))(i)
}

fn operators(i: &str) -> TokenRes<'_> {
    alt((
        map(tag("^"), |s| RawToken::new(TaskToken::Caret, s)),
        map(tag("-"), |s| RawToken::new(TaskToken::Dash, s)),
        map(tag("+"), |s| RawToken::new(TaskToken::Plus, s)),
        map(tag("*"), |s| RawToken::new(TaskToken::Star, s)),
        map(tag("/"), |s| RawToken::new(TaskToken::Slash, s)),
        map(tag("%"), |s| RawToken::new(TaskToken::Percentage, s)),
        map(tag("="), |s| RawToken::new(TaskToken::Assignment, s)),
        map(tag("&"), |s| RawToken::new(TaskToken::And, s)),
        map(tag("|"), |s| RawToken::new(TaskToken::Or, s)),
        map(tag("!"), |s| RawToken::new(TaskToken::Not, s)),
    ))(i)
}

fn symbols(i: &str) -> TokenRes<'_> {
    alt((
        map(tag("->"), |s| RawToken::new(TaskToken::PathSep, s)),
        map(tag("<"), |s| RawToken::new(TaskToken::AngleStart, s)),
        map(tag(">"), |s| RawToken::new(TaskToken::AngleEnd, s)),
        map(tag("("), |s| RawToken::new(TaskToken::ParenStart, s)),
        map(tag(")"), |s| RawToken::new(TaskToken::ParenEnd, s)),
        map(tag("["), |s| RawToken::new(TaskToken::BracketStart, s)),
        map(tag("]"), |s| RawToken::new(TaskToken::BracketEnd, s)),
        map(tag("{"), |s| RawToken::new(TaskToken::BraceStart, s)),
        map(tag("}"), |s| RawToken::new(TaskToken::BraceEnd, s)),
        map(tag("."), |s| RawToken::new(TaskToken::Dot, s)),
        map(tag(","), |s| RawToken::new(TaskToken::Comma, s)),
        map(tag("?"), |s| RawToken::new(TaskToken::Question, s)),
        map(tag(":"), |s| RawToken::new(TaskToken::Colon, s)),
        map(tag(";"), |s| RawToken::new(TaskToken::Semicolon, s)),
        map(tag("@"), |s| RawToken::new(TaskToken::At, s)),
        map(tag("$"), |s| RawToken::new(TaskToken::Dollar, s)),
    ))(i)
}

/// Check if the given string is a valid variable name
///
/// This checks the name with the variable parser. Use the
/// [`attrs::valid_var_manual`] if `parser` feature is not activated.
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
                // .0 etc are supported for array indexing
                if re.trim_start().starts_with('"')
                    || re
                        .trim_start()
                        .chars()
                        .next()
                        .map(|c| c.is_numeric())
                        .unwrap_or_default()
                {
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
    Ok((rest, RawToken::new(ty, var)))
}

fn template(i: &str) -> TokenRes<'_> {
    let (rest, s) = context("template", preceded(tag("r"), parse_string))(i)?;
    Ok((
        rest,
        RawToken::new(TaskToken::Template(s), &i[..(i.len() - rest.len())]),
    ))
}

fn string(i: &str) -> TokenRes<'_> {
    let (rest, s) = context("string", parse_string)(i)?;
    Ok((
        rest,
        RawToken::new(TaskToken::String(s), &i[..(i.len() - rest.len())]),
    ))
}

fn lit_string(i: &str) -> TokenRes<'_> {
    let (rest, s) = context(
        "string",
        delimited(tag("'"), recognize(many0(is_not("'"))), tag("'")),
    )(i)?;
    Ok((
        rest,
        RawToken::new(
            TaskToken::String(s.to_string()),
            &i[..(i.len() - rest.len())],
        ),
    ))
}

fn boolean(i: &str) -> TokenRes<'_> {
    map(alt((tag("true"), tag("false"))), |s| {
        RawToken::new(TaskToken::Bool, s)
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
        |s| RawToken::new(TaskToken::Integer, s),
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
        |s| RawToken::new(TaskToken::Float, s),
    )(i)
}

fn date(i: &str) -> TokenRes<'_> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char('-')))), digit1))),
        |s| RawToken::new(TaskToken::Date, s),
    )(i)
}

fn time(i: &str) -> TokenRes<'_> {
    map(
        recognize(tuple((many1(terminated(digit1, many1(char(':')))), digit1))),
        |s| RawToken::new(TaskToken::Time, s),
    )(i)
}

fn datetime(i: &str) -> TokenRes<'_> {
    map(recognize(tuple((date, one_of(" T"), time))), |s| {
        RawToken::new(TaskToken::DateTime, s)
    })(i)
}

fn task_token(i: &str) -> TokenRes<'_> {
    alt((
        whitespace, newline, comment, template, string, lit_string, datetime, date, time, boolean,
        float, integer, variable, none, symbols, operators, invalid,
    ))(i)
}

fn task_script(i: &str) -> VecTokenRes<'_> {
    context("task script", many0(task_token))(i)
}

pub fn get_tokens<'a>(txt: &'a str) -> Vec<RawToken<'a>> {
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
    fn parencheck_paired_test(
        #[values(" ", "()", "{}", "[]", "[{()}]", "{[]}", "([], {})")] txt: &str,
    ) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let check = ParenCheck::scan(&tokens);
        assert!(matches!(check, ParenCheck::Paired))
    }

    #[rstest] // whitespace
    fn parencheck_extra_test(
        #[values(" )", "())", "{}]", "[])", "[{()}])", "{[]}}", "([], {})}")] txt: &str,
    ) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let check = ParenCheck::scan(&tokens);
        assert!(matches!(check, ParenCheck::Extra(_)))
    }

    #[rstest] // whitespace
    fn parencheck_invalid_test(#[values(" (]", "({)", "[{}]()[}")] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let check = ParenCheck::scan(&tokens);
        assert!(matches!(check, ParenCheck::Invalid(_, _)))
    }

    #[rstest] // whitespace
    fn parencheck_unpaired_test(#[values(" (", "({", "[{}]()[")] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let check = ParenCheck::scan(&tokens);
        assert!(matches!(check, ParenCheck::Unpaired(_)))
    }

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
    #[case("~", TaskToken::Invalid('~'), "")]
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
    #[case("@~", TaskToken::At, "~")]
    #[case("~@", TaskToken::Invalid('~'), "@")]
    fn task_token_test(#[case] txt: &str, #[case] value: TaskToken, #[case] reminder: &str) {
        let (rest, n) = task_token(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n.ty, value);
    }
}
