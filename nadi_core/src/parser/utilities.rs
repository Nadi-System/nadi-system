/// The plan is to use this to group the tokens first before parsing
/// them within the group, so that we don't have to parse a large
/// amount of nested expressions
use crate::expressions::{
    BiOperator, Expression, FunctionCall, InputVar, TaskPosition, UniOperator, VarType,
};
use crate::network::PropNodes;
use crate::parser::{
    components::*,
    errors::{MatchErr, ParseErrorType},
    tasks::propagation,
    tokenizer::Token,
};
use crate::tasks::TaskKeyword;
use crate::udf::UserFunction;
use nom::{
    branch::alt,
    combinator::{self, cut, map, opt, value},
    multi::{many1, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
};

pub fn inner_paren_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(paren_start, combinator::not(paren_start), paren_end)(inp)
}

pub fn inner_brace_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(brace_start, combinator::not(brace_start), brace_end)(inp)
}

pub fn inner_bracket_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(bracket_start, combinator::not(bracket_start), bracket_end)(inp)
}

pub fn inner_any_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    alt((inner_paren_pair, inner_brace_pair, inner_bracket_pair))(inp)
}

pub fn any_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    alt((inner_paren_pair, inner_brace_pair, inner_bracket_pair))(inp)
}

pub fn paren_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(
        paren_start,
        alt((any_pair, many0(combinator::not(paren_start)))),
        paren_end,
    )
}

pub fn brace_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(
        brace_start,
        alt((any_pair, many0(combinator::not(brace_start)))),
        brace_end,
    )
}

pub fn bracket_pair<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ()> {
    delimited(
        bracket_start,
        alt((any_pair, many0(combinator::not(bracket_start)))),
        bracket_end,
    )
}
