use crate::parser::{
    components::*,
    errors::MatchErr,
    expressions::complete_expression,
    network::{node_name, str_path},
    tokenizer::{TaskToken, Token},
    ParseError, ParseErrorType,
};
use crate::{
    functions::Propagation,
    network::StrPath,
    prelude::*,
    tasks::{FunctionType, Task},
};
use abi_stable::std_types::{RString, RVec};
use nom::{
    branch::alt,
    combinator::{all_consuming, map},
    multi::separated_list1,
    sequence::{delimited, separated_pair},
    Finish,
};

pub fn prop_seq<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Propagation> {
    let (rest, var) = delimited(
        angle_start,
        maybe_newline(variable),
        maybe_newline(angle_end),
    )(inp)?;
    let prop = match var.content {
        "sequential" => Propagation::Sequential,
        "inverse" => Propagation::Inverse,
        "inputsfirst" => Propagation::InputsFirst,
        "outputfirst" => Propagation::OutputFirst,
        _ => {
            return Err(nom::Err::Failure(
                MatchErr::new(inp).ty(&ParseErrorType::ValueError("Invalid Propagation name")),
            ))
        }
    };
    Ok((rest, prop))
}

pub fn node_list<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, RVec<RString>> {
    map(
        separated_list1(maybe_newline(comma), maybe_newline(node_name)),
        |v| v.into_iter().map(RString::from).collect(),
    )(inp)
}

pub fn propagation<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Propagation> {
    alt((
        prop_seq,
        delimited(
            bracket_start,
            alt((
                map(maybe_newline(node_list), Propagation::List),
                map(maybe_newline(str_path), Propagation::Path),
            )),
            maybe_newline(bracket_end),
        ),
        map(
            delimited(
                paren_start,
                maybe_newline(complete_expression),
                maybe_newline(paren_end),
            ),
            Propagation::Conditional,
        ),
    ))(inp)
}

pub fn function_type<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FunctionType> {
    let (rest, kw) = keyword_val(inp)?;
    match FunctionType::from_keyword(&kw) {
        Some(v) => Ok((rest, v)),
        None => Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::InvalidKeyword),
        )),
    }
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Task>, ParseError> {
    todo!()
}
