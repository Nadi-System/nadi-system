use crate::parser::{
    errors::{MatchErr, ParseErrorType},
    tokenizer::{TaskToken, Token},
};
use crate::tasks::TaskKeyword;
use nadi_core::attrs::{Attribute, Date, DateTime, Time};
use nom::{
    branch::alt,
    combinator::{map, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, preceded, separated_pair, terminated},
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

one_token!(infinity, TaskToken::Infinity);
one_token!(nan, TaskToken::NaN);
one_token!(float, TaskToken::Float);
one_token!(integer, TaskToken::Integer);
one_token!(boolean, TaskToken::Bool);
one_token!(string, TaskToken::String(_));
one_token!(date, TaskToken::Date);
one_token!(time, TaskToken::Time);
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
one_token!(invalid, TaskToken::Invalid(_));

one_token!(kw_if, TaskToken::Keyword(TaskKeyword::If));
one_token!(kw_else, TaskToken::Keyword(TaskKeyword::Else));
one_token!(kw_while, TaskToken::Keyword(TaskKeyword::While));
one_token!(kw_in, TaskToken::Keyword(TaskKeyword::In));
one_token!(kw_match, TaskToken::Keyword(TaskKeyword::Match));
one_token!(kw_help, TaskToken::Keyword(TaskKeyword::Help));
one_token!(kw_end, TaskToken::Keyword(TaskKeyword::End));
one_token!(kw_exit, TaskToken::Keyword(TaskKeyword::Exit));

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

pub fn attr_bool<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, val) = boolean(inp)?;
    let val = match val.content {
        "true" => true,
        "false" => false,
        _ => {
            return Err(nom::Err::Error(MatchErr::new(inp).ty(
                &ParseErrorType::ValueError("Boolean should be true or false"),
            )))
        }
    }
    .into();
    Ok((rest, val))
}

pub fn attr_integer<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, val) = integer(inp)?;
    let val = match val.content.replace('_', "").parse::<i64>() {
        Ok(v) => v,
        _ => {
            return Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::ValueError("Error while parsing Integer")),
            ))
        }
    }
    .into();
    Ok((rest, val))
}

pub fn attr_float<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    alt((
        attr_float_number,
        value(Attribute::Float(f64::NAN), nan),
        value(Attribute::Float(f64::INFINITY), infinity),
    ))(inp)
}

pub fn attr_float_number<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, val) = float(inp)?;
    let val = match val.content.replace('_', "").parse::<f64>() {
        Ok(v) => v,
        _ => {
            return Err(nom::Err::Error(
                MatchErr::new(inp).ty(&ParseErrorType::ValueError("Error while parsing Float")),
            ))
        }
    }
    .into();
    Ok((rest, val))
}

pub fn attribute_simple<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Attribute> {
    let (rest, var) = alt((
        attr_bool,
        attr_float,
        attr_integer,
        map(string_val, |s| Attribute::String(s.into())),
        map(datetime, |t| {
            Attribute::DateTime(DateTime::from_str(t.content).unwrap())
        }),
        map(time, |t| {
            Attribute::Time(Time::from_str(t.content).unwrap())
        }),
        map(date, |t| {
            Attribute::Date(Date::from_str(t.content).unwrap())
        }),
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
        let tokens = get_tokens(txt);
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
        let tokens = get_tokens(txt);
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
        let tokens = get_tokens(txt);
        let (rest, tk) = maybe_newline(float)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        assert_eq!(tk.content, val);
    }

    #[rstest]
    #[case("12.23\n1.12\n1.23", 3)]
    #[case("\n12.23\n1.12\n1.23", 3)]
    #[case("\n#comment \n12.23\n 1.12\n1.23", 3)]
    pub fn newline_separated_test(#[case] txt: &str, #[case] count: usize) {
        let tokens = get_tokens(txt);
        let (_, tk) = newline_separated(float)(&tokens).unwrap();
        assert_eq!(tk.len(), count)
    }

    #[rstest]
    #[case("12.23", TaskToken::Float)]
    pub fn float_test(#[case] txt: &str, #[case] ty: TaskToken) {
        let tokens = get_tokens(txt);
        let (_, tk) = float(&tokens).unwrap();
        assert!(tk.ty == ty)
    }

    #[rstest]
    #[case("12,23", TaskToken::Integer)]
    pub fn integer_test(#[case] txt: &str, #[case] ty: TaskToken) {
        let tokens = get_tokens(txt);
        let (_, tk) = integer(&tokens).unwrap();
        assert!(tk.ty == ty)
    }

    #[rstest]
    #[case("val")]
    #[case("val2")]
    pub fn variable_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
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
        let tokens = get_tokens(txt);
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
        let tokens = get_tokens(txt);
        let (_, tk) = key_val_dot(&tokens).unwrap();
        let key: Vec<String> = key.into_iter().map(String::from).collect();
        assert_eq!(key, tk.0);
        assert_eq!(val, tk.1);
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
    #[should_panic] // invalid month
    #[case("1223-24-23", Attribute::Date(Date::new(1223, 24, 23)))]
    pub fn attribute_test(#[case] txt: &str, #[case] value: Attribute) {
        let tokens = get_tokens(txt);
        let (_, tk) = attribute(&tokens).unwrap();
        assert!(tk == value)
    }

    #[rstest]
    #[case("exit")]
    #[should_panic]
    #[case("help")]
    pub fn kw_exit_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
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
    #[should_panic] // invalid month
    #[case("1223-24-23")]
    pub fn toml_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        let (_, tk) = attribute(&tokens).unwrap();
        let val = tk.to_toml_string();
        assert_eq!(txt, val)
    }
}
