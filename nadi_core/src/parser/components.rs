use crate::expressions::InputVarIndex;
use crate::parser::{
    errors::{MatchErr, ParseErrorType},
    tokenizer::{TaskToken, Token},
};
use crate::tasks::TaskKeyword;
use crate::template::Template;
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use nom::{
    branch::alt,
    combinator::{map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
    IResult,
};
use std::str::FromStr;

pub type MatchRes<'a, 'b, T> = IResult<&'a [Token<'b>], T, MatchErr<'a, 'b>>;

pub fn string_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, String> {
    if let [first, rest @ ..] = inp {
        match &first.ty {
            TaskToken::String(s) => Ok((rest, s.clone())),
            _ => Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::TokenMismatch),
            )),
        }
    } else {
        Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::Incomplete),
        ))
    }
}

pub fn template_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Template> {
    if let [first, rest @ ..] = inp {
        match &first.ty {
            TaskToken::Template(s) => Template::from_str(s).map(|t| (rest, t)).map_err(|e| {
                nom::Err::Error(MatchErr::new(inp).ty(&ParseErrorType::InvalidTemplate(e)))
            }),
            _ => Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::TokenMismatch),
            )),
        }
    } else {
        Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::Incomplete),
        ))
    }
}

pub fn keyword_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, TaskKeyword> {
    if let [first, rest @ ..] = inp {
        match &first.ty {
            TaskToken::Keyword(s) => Ok((rest, s.clone())),
            _ => Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::TokenMismatch),
            )),
        }
    } else {
        Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::Incomplete),
        ))
    }
}

pub fn variable_name<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, String> {
    if let [first, rest @ ..] = inp {
        match &first.ty {
            TaskToken::Variable => Ok((rest, first.content.to_string())),
            _ => Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::TokenMismatch),
            )),
        }
    } else {
        Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::Incomplete),
        ))
    }
}

macro_rules! one_token {
    ($name:ident, $ty:pat) => {
        pub fn $name<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, &'a Token<'b>> {
            match inp {
                [first, rest @ ..] => match first.ty {
                    $ty => Ok((rest, first)),
                    _ => Err(nom::Err::Error(
                        MatchErr::new(inp).ty(&ParseErrorType::TokenMismatch),
                    )),
                },
                [] => Err(nom::Err::Error(
                    MatchErr::new(inp).ty(&ParseErrorType::Incomplete),
                )),
            }
        }
    };
}

one_token!(none, TaskToken::None);
one_token!(infinity, TaskToken::Infinity);
one_token!(nan, TaskToken::NaN);
one_token!(float, TaskToken::Float);
one_token!(integer, TaskToken::Integer);
one_token!(boolean, TaskToken::Bool);
one_token!(string, TaskToken::String(_));
one_token!(template, TaskToken::Template(_));
one_token!(datetime, TaskToken::DateTime);
one_token!(newline, TaskToken::NewLine);
one_token!(space, TaskToken::WhiteSpace);
one_token!(comment, TaskToken::Comment);
one_token!(keyword, TaskToken::Keyword(_));
one_token!(variable, TaskToken::Variable);
one_token!(function, TaskToken::Function);
one_token!(angle_start, TaskToken::AngleStart);
one_token!(paren_start, TaskToken::ParenStart);
one_token!(brace_start, TaskToken::BraceStart);
one_token!(bracket_start, TaskToken::BracketStart);
one_token!(path_sep, TaskToken::PathSep);
one_token!(comma, TaskToken::Comma);
one_token!(caret, TaskToken::Caret);
one_token!(dash, TaskToken::Dash);
one_token!(plus, TaskToken::Plus);
one_token!(star, TaskToken::Star);
one_token!(slash, TaskToken::Slash);
one_token!(percentage, TaskToken::Percentage);
one_token!(question, TaskToken::Question);
one_token!(colon, TaskToken::Colon);
one_token!(semicolon, TaskToken::Semicolon);
one_token!(dot, TaskToken::Dot);
one_token!(and, TaskToken::And);
one_token!(or, TaskToken::Or);
one_token!(not, TaskToken::Not);
one_token!(angle_end, TaskToken::AngleEnd);
one_token!(paren_end, TaskToken::ParenEnd);
one_token!(brace_end, TaskToken::BraceEnd);
one_token!(bracket_end, TaskToken::BracketEnd);
one_token!(assignment, TaskToken::Assignment);
one_token!(at, TaskToken::At);
one_token!(dollar, TaskToken::Dollar);
one_token!(invalid, TaskToken::Invalid(_));

one_token!(kw_network, TaskToken::Keyword(TaskKeyword::Network));
one_token!(kw_node, TaskToken::Keyword(TaskKeyword::Node));
one_token!(kw_env, TaskToken::Keyword(TaskKeyword::Env));
one_token!(kw_import, TaskToken::Keyword(TaskKeyword::Import));
one_token!(kw_exec, TaskToken::Keyword(TaskKeyword::Exec));
one_token!(kw_from, TaskToken::Keyword(TaskKeyword::From));
one_token!(kw_if, TaskToken::Keyword(TaskKeyword::If));
one_token!(kw_else, TaskToken::Keyword(TaskKeyword::Else));
one_token!(kw_while, TaskToken::Keyword(TaskKeyword::While));
one_token!(kw_for, TaskToken::Keyword(TaskKeyword::For));
one_token!(kw_try, TaskToken::Keyword(TaskKeyword::Try));
one_token!(kw_catch, TaskToken::Keyword(TaskKeyword::Catch));
one_token!(kw_func, TaskToken::Keyword(TaskKeyword::Function));
one_token!(kw_struct, TaskToken::Keyword(TaskKeyword::Struct));
one_token!(kw_error, TaskToken::Keyword(TaskKeyword::Error));
one_token!(kw_in, TaskToken::Keyword(TaskKeyword::In));
one_token!(kw_match, TaskToken::Keyword(TaskKeyword::Match));
one_token!(kw_hook, TaskToken::Keyword(TaskKeyword::Hook));
one_token!(kw_help, TaskToken::Keyword(TaskKeyword::Help));
one_token!(kw_end, TaskToken::Keyword(TaskKeyword::End));
one_token!(kw_clear, TaskToken::Keyword(TaskKeyword::Clear));
one_token!(kw_exit, TaskToken::Keyword(TaskKeyword::Exit));
one_token!(kw_return, TaskToken::Keyword(TaskKeyword::Return));

/// Matches the next one that might have spaces before it
pub fn err_ctx<'a, 'b: 'a, O, F>(
    ty: &'static ParseErrorType,
    mut f: F,
) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, O>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    move |i: &'a [Token<'b>]| match f.parse(i) {
        Ok(o) => Ok(o),
        Err(nom::Err::Incomplete(i)) => Err(nom::Err::Incomplete(i)),
        Err(nom::Err::Error(e)) => Err(nom::Err::Error(e.ty(ty))),
        Err(nom::Err::Failure(e)) => Err(nom::Err::Failure(e.ty(ty))),
    }
}

/// Matches the next one that might have spaces before it
pub fn maybe_space<'a, 'b: 'a, O, F>(f: F) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, O>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    preceded(many0(space), f)
}

/// Matches the next one that might have spaces before it
pub fn after_space<'a, 'b: 'a, O, F>(f: F) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, O>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    preceded(many1(space), f)
}

/// Matches the next one that might have spaces, newlines or comments before it
pub fn maybe_newline<'a, 'b: 'a, O, F>(f: F) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, O>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    preceded(many0_newlines, f)
}

/// Matches the next one that might have spaces, newlines or comments before it
pub fn newline_separated<'a, 'b: 'a, O, F>(
    f: F,
) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<O>>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    maybe_newline(separated_list0(many1_newlines, maybe_space(f)))
}

/// Matches the next one that might have spaces, newlines or comments before it
pub fn trailing_newlines<'a, 'b: 'a, O, F>(
    f: F,
) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, O>
where
    F: nom::Parser<&'a [Token<'b>], O, MatchErr<'a, 'b>>,
{
    terminated(f, many0_newlines)
}

pub fn many0_newlines<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    value((), many0(alt((space, newline, comment))))(inp)
}

pub fn many1_newlines<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    value((), many1(maybe_space(alt((newline, comment)))))(inp)
}

pub fn dash_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, String> {
    map(
        separated_list1(dash, map(variable, |v| v.content.to_string())),
        |v| v.join("-"),
    )(inp)
}

pub fn task_dot_variable<'a, 'b>(
    inp: &'a [Token<'b>],
) -> MatchRes<'a, 'b, (String, Vec<InputVarIndex>)> {
    alt((
        // to avoid it taking literal string as variable unless followed by a dot
        separated_pair(
            alt((map(variable, |v| v.content.to_string()), string_val)),
            dot,
            separated_list1(
                dot,
                alt((
                    map(variable, |v| InputVarIndex::Str(v.content.to_string())),
                    map(string_val, InputVarIndex::Str),
                    map(integer_usize, InputVarIndex::Int),
                )),
            ),
        ),
        map(variable, |v| (v.content.to_string(), vec![])),
    ))(inp)
}

pub fn dot_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<String>> {
    alt((
        // to avoid it taking literal string as variable unless followed by a dot
        map(
            separated_pair(
                alt((dash_variable, string_val)),
                dot,
                separated_list1(dot, alt((dash_variable, string_val))),
            ),
            |(v1, mut v2)| {
                let mut v = vec![v1];
                v.append(&mut v2);
                v
            },
        ),
        map(dash_variable, |v| vec![v]),
    ))(inp)
}

macro_rules! parse_err {
    ($inp:ident, $msg:literal) => {
        Err(nom::Err::Error(
            MatchErr::new($inp).ty(&ParseErrorType::ValueError($msg)),
        ))
    };
}

pub fn attr_bool<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, val) = boolean(inp)?;
    let val = match val.content {
        "true" => true,
        "false" => false,
        _ => {
            return parse_err!(inp, "Boolean should be true or false");
        }
    }
    .into();
    Ok((rest, val))
}

pub fn integer_usize<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, usize> {
    let (rest, val) = integer(inp)?;
    let val = match val.content.replace('_', "").parse::<usize>() {
        Ok(v) => v,
        _ => {
            return parse_err!(inp, "Error while parsing Integer");
        }
    };
    Ok((rest, val))
}

pub fn integer_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, i64> {
    let (rest, val) = integer(inp)?;
    let val = match val.content.replace('_', "").parse::<i64>() {
        Ok(v) => v,
        _ => {
            return parse_err!(inp, "Error while parsing Integer");
        }
    };
    Ok((rest, val))
}

pub fn float_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, f64> {
    let (rest, val) = float(inp)?;
    let val = match val.content.replace('_', "").parse::<f64>() {
        Ok(v) => v,
        _ => {
            return parse_err!(inp, "Error while parsing Float");
        }
    };
    Ok((rest, val))
}

pub fn neg_sign<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, bool> {
    let (rest, sign) = alt((plus, dash))(inp)?;
    Ok((rest, sign.content == "-"))
}

pub fn attr_integer<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, (neg, val)) = pair(opt(neg_sign), integer_val)(inp)?;
    Ok((rest, if let Some(true) = neg { -val } else { val }.into()))
}

pub fn all_float<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, f64> {
    alt((
        float_val,
        value(f64::NAN, nan),
        value(f64::INFINITY, infinity),
    ))(inp)
}

pub fn attr_float<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, (neg, val)) = pair(opt(neg_sign), all_float)(inp)?;
    Ok((rest, if let Some(true) = neg { -val } else { val }.into()))
}

/// Primitives do not have sign (that is unary operator)
pub fn primitives<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((
        attr_bool,
        map(all_float, Attribute::Float),
        map(integer_val, Attribute::Integer),
        map(string_val, |s| Attribute::String(s.into())),
        map(datetime, |t| {
            Attribute::DateTime(DateTime::from_str(t.content).unwrap())
        }),
    ))(inp)?;
    Ok((rest, var))
}

/// Sign is important in case of loading attributes
pub fn signed_primitives<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((
        attr_bool,
        map(pair(opt(neg_sign), all_float), |(s, f)| {
            Attribute::Float(if let Some(true) = s { -f } else { f })
        }),
        map(pair(opt(neg_sign), integer_val), |(s, i)| {
            Attribute::Integer(if let Some(true) = s { -i } else { i })
        }),
        map(string_val, |s| Attribute::String(s.into())),
        map(datetime, |t| {
            Attribute::DateTime(DateTime::from_str(t.content).unwrap())
        }),
    ))(inp)?;
    Ok((rest, var))
}

pub fn date<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Date> {
    let (rest, (year, _, month, _, day)) = tuple((integer, dash, integer, dash, integer))(inp)?;
    let year = match year.content.parse::<u16>() {
        Ok(y) => y,
        Err(_) => return parse_err!(inp, "Invalid year"),
    };
    let month = match month.content.parse::<u8>() {
        Ok(m) => m,
        Err(_) => return parse_err!(inp, "Invalid year"),
    };
    let day = match day.content.parse::<u8>() {
        Ok(d) => d,
        Err(_) => return parse_err!(inp, "Invalid year"),
    };
    Ok((rest, Date::new(year, month, day)))
}

pub fn time<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Time> {
    let (rest, (hour, _, min, second)) =
        tuple((integer, colon, integer, opt(pair(colon, integer))))(inp)?;
    let hour = match hour.content.parse::<u8>() {
        Ok(h) => h,
        Err(_) => return parse_err!(inp, "Invalid year"),
    };
    let min = match min.content.parse::<u8>() {
        Ok(m) => m,
        Err(_) => return parse_err!(inp, "Invalid year"),
    };
    let sec = if let Some((_, sec)) = second {
        match sec.content.parse::<u8>() {
            Ok(d) => d,
            Err(_) => return parse_err!(inp, "Invalid year"),
        }
    } else {
        0
    };
    Ok((rest, Time::new(hour, min, sec, 0)))
}

pub fn attribute_simple<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((
        map(date, Attribute::Date),
        map(time, Attribute::Time),
        signed_primitives,
    ))(inp)?;
    Ok((rest, var))
}

pub fn attribute<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((attribute_simple, array, table))(inp)?;
    Ok((rest, var))
}

pub fn attribute_inline<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((attribute_simple, array_inline, table_inline))(inp)?;
    Ok((rest, var))
}

pub fn key_val_dot<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (Vec<String>, Attribute)> {
    separated_pair(
        dot_variable,
        maybe_space(assignment),
        maybe_space(attribute_inline),
    )(inp)
}

pub fn key_val<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (String, Attribute)> {
    separated_pair(
        // no dot variable in keyval pair
        alt((map(variable, |t| t.content.to_string()), string_val)),
        maybe_space(assignment),
        maybe_space(attribute),
    )(inp)
}

pub fn array<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, vars) = delimited(
        bracket_start,
        maybe_newline(separated_list0(
            maybe_newline(comma),
            maybe_newline(attribute),
        )),
        maybe_newline(bracket_end),
    )(inp)?;
    Ok((rest, Attribute::Array(vars.into())))
}

pub fn table<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, vars) = delimited(
        brace_start,
        maybe_newline(separated_list0(
            maybe_newline(comma),
            maybe_newline(key_val),
        )),
        maybe_newline(brace_end),
    )(inp)?;
    Ok((
        rest,
        Attribute::Table(vars.into_iter().map(|(k, v)| (k.into(), v)).collect()),
    ))
}

pub fn array_inline<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, vars) = delimited(
        bracket_start,
        maybe_space(separated_list0(maybe_space(comma), maybe_space(attribute))),
        maybe_space(bracket_end),
    )(inp)?;
    Ok((rest, Attribute::Array(vars.into())))
}

pub fn table_inline<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, vars) = delimited(
        brace_start,
        maybe_space(separated_list0(maybe_space(comma), maybe_space(key_val))),
        maybe_space(brace_end),
    )(inp)?;
    Ok((
        rest,
        Attribute::Table(vars.into_iter().map(|(k, v)| (k.into(), v)).collect()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokenizer::get_tokens;
    use rstest::rstest;

    #[rstest]
    #[case("12.23", "12.23")]
    #[case(" 12.23", "12.23")]
    #[case("  12.23", "12.23")]
    #[case("         12.23", "12.23")]
    pub fn maybe_space_test(#[case] txt: &str, #[case] val: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tk) = maybe_space(float)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        assert_eq!(tk.content, val);
    }

    #[rstest]
    #[should_panic]
    #[case("12.23", "12.23")]
    #[case(" 12.23", "12.23")]
    #[case("  12.23", "12.23")]
    #[case("         12.23", "12.23")]
    pub fn after_space_test(#[case] txt: &str, #[case] val: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tk) = after_space(float)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        assert_eq!(tk.content, val);
    }

    #[rstest]
    #[case("12.23", "12.23")]
    #[case(" 12.23", "12.23")]
    #[case("\n  12.23", "12.23")]
    #[case("  \n  \n  12.23", "12.23")]
    #[case("  \n #comment \n  12.23", "12.23")]
    pub fn maybe_newline_test(#[case] txt: &str, #[case] val: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tk) = maybe_newline(float)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        assert_eq!(tk.content, val);
    }

    #[rstest]
    #[case("12.23\n1.12\n1.23", 3)]
    #[case("\n12.23\n1.12\n1.23", 3)]
    #[case("\n#comment \n12.23\n 1.12\n1.23", 3)]
    pub fn newline_separated_test(#[case] txt: &str, #[case] count: usize) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = newline_separated(float)(&tokens).unwrap();
        assert_eq!(tk.len(), count)
    }

    #[rstest]
    #[case("12.23", TaskToken::Float)]
    pub fn float_test(#[case] txt: &str, #[case] ty: TaskToken) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = float(&tokens).unwrap();
        assert!(tk.ty == ty)
    }

    #[rstest]
    #[case("12,23", TaskToken::Integer)]
    pub fn integer_test(#[case] txt: &str, #[case] ty: TaskToken) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = integer(&tokens).unwrap();
        assert!(tk.ty == ty)
    }

    #[rstest]
    #[case("val")]
    #[case("val2")]
    pub fn variable_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = variable(&tokens).unwrap();
        assert_eq!(tk.content, txt)
    }

    #[rstest]
    #[case("val", vec!["val"])]
    #[case("val.val2", vec!["val", "val2"])]
    #[case("val.\"val2\"", vec!["val", "val2"])]
    #[case("\"val\".val2", vec!["val", "val2"])]
    #[should_panic]
    #[case("1232", vec!["1232"])]
    pub fn dot_variable_test(#[case] txt: &str, #[case] vals: Vec<&str>) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = dot_variable(&tokens).unwrap();
        let vals: Vec<String> = vals.into_iter().map(String::from).collect();
        assert_eq!(tk, vals)
    }

    #[rstest]
    #[case("val = 12", vec!["val"], Attribute::Integer(12))]
    #[case("val.val2 = \"=val~2\"", vec!["val", "val2"], Attribute::String("=val~2".into()))]
    #[case("val.\"val2\" = -1.123", vec!["val", "val2"], Attribute::Float(-1.123))]
    #[case("\"val\".val2 = 1223-12-12", vec!["val", "val2"], Attribute::Date(Date::new(1223,12,12)))]
    #[should_panic]
    #[case("= 1232", vec![""], Attribute::Integer(1232))]
    #[should_panic]
    #[case("var =\n 1232", vec!["var"], Attribute::Integer(1232))]
    pub fn key_val_dot_test(#[case] txt: &str, #[case] key: Vec<&str>, #[case] val: Attribute) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = key_val_dot(&tokens).unwrap();
        let key: Vec<String> = key.into_iter().map(String::from).collect();
        assert_eq!(key, tk.0);
        assert_eq!(val, tk.1);
    }

    #[rstest]
    #[case("1223-12-23", Date::new(1223, 12, 23))]
    #[should_panic] // datetime
    #[case("1223-12-23 23:00", Date::new(1223, 12, 23))]
    // invalid month does not matter, leave it to the implementation
    #[case("1223-24-23", Date::new(1223, 24, 23))]
    pub fn date_test(#[case] txt: &str, #[case] value: Date) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = date(&tokens).unwrap();
        assert_eq!(tk, value)
    }

    #[rstest]
    #[case("1223-12-23 23:00", Date::new(1223, 12, 23).with_time(Time::new(23,0,0,0)))]
    // invalid month matters for datetime in iso format
    #[should_panic]
    #[case("1223-24-23T24:98:00", Date::new(1223, 24, 23).with_time(Time::new(24,98,0,0)))]
    #[case("1223-02-23T14:14:00", Date::new(1223, 2, 23).with_time(Time::new(14,14,0,0)))]
    pub fn datetime_test(#[case] txt: &str, #[case] value: DateTime) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, v) = datetime(&tokens).unwrap();
        assert_eq!(rest, []);
        assert_eq!(value, DateTime::from_str(v.content).unwrap());
    }

    #[rstest]
    #[case("1232", Attribute::Integer(1232))]
    #[case("12.32", Attribute::Float(12.32))]
    #[case("\"12.32\"", Attribute::String("12.32".into()))]
    #[case("true", Attribute::Bool(true))]
    #[case("false", Attribute::Bool(false))]
    #[should_panic]
    #[case("null", Attribute::Bool(false))]
    #[case("1223-12-23", Attribute::Date(Date::new(1223, 12, 23)))]
    #[case(
        "1223-12-23 23:00",
        Attribute::DateTime(DateTime::new(Date::new(1223, 12, 23), Time::new(23, 0, 0, 0), None))
    )]
    // invalid month does not matter, leave it to the implementation
    #[case("1223-24-23", Attribute::Date(Date::new(1223, 24, 23)))]
    pub fn attribute_test(#[case] txt: &str, #[case] value: Attribute) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = attribute(&tokens).unwrap();
        assert_eq!(tk, value)
    }

    #[rstest]
    #[case("exit")]
    #[should_panic]
    #[case("help")]
    pub fn kw_exit_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        kw_exit(&tokens).unwrap();
    }

    #[rstest]
    #[case("1232")]
    #[case("12.32")]
    #[case("\"12.32\"")]
    #[case("true")]
    #[case("false")]
    #[should_panic]
    #[case("null")]
    #[case("1223-12-23")]
    #[case("\"some complicated string ain't problem?\"")]
    #[case("1223-12-23 23:00:12")]
    // invalid month doesn't matter
    #[case("1223-24-23")]
    pub fn toml_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (_, tk) = attribute(&tokens).unwrap();
        let val = tk.to_string();
        assert_eq!(txt, val)
    }
}
