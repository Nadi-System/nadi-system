use crate::network::StrPath;
use crate::parser::{
    components::*,
    errors::{ParseError, ParseErrorType},
    network::*,
    tokenizer::{RawToken, Token},
};
use crate::prelude::AttrMap;
use nom::{
    branch::alt,
    combinator::map,
    multi::separated_list0,
    sequence::{delimited, pair, tuple},
    Finish,
};
use std::collections::HashMap;

#[derive(Default)]
pub struct GvNetwork {
    pub nodes: HashMap<String, AttrMap>,
    pub edges: HashMap<StrPath, AttrMap>,
}

pub enum GvLine {
    Node((String, AttrMap)),
    Edge((StrPath, AttrMap)),
}

pub fn gv_file<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, GvNetwork> {
    let (rest, lines) = trailing_newlines(delimited(
        tuple((variable, maybe_space(variable), maybe_space(brace_start))),
        maybe_newline(gv_network),
        maybe_newline(brace_end),
    ))(inp)?;
    let mut net = GvNetwork::default();
    for line in lines {
        match line {
            GvLine::Node((k, v)) => net.nodes.insert(k, v),
            GvLine::Edge((k, v)) => net.edges.insert(k, v),
        };
    }
    Ok((rest, net))
}

pub fn gv_network<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<GvLine>> {
    newline_separated(alt((
        map(maybe_space(gv_node), GvLine::Node),
        map(maybe_space(gv_edge), GvLine::Edge),
    )))(inp)
}

pub fn gv_node<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (String, AttrMap)> {
    pair(
        node_name,
        delimited(
            maybe_space(bracket_start),
            map(
                separated_list0(maybe_space(comma), maybe_newline(key_val)),
                |kv| kv.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ),
            maybe_space(pair(bracket_end, semicolon)),
        ),
    )(inp)
}

pub fn gv_edge<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (StrPath, AttrMap)> {
    pair(
        str_path,
        delimited(
            maybe_space(bracket_start),
            map(
                separated_list0(maybe_space(comma), maybe_newline(key_val)),
                |kv| kv.into_iter().map(|(k, v)| (k.into(), v)).collect(),
            ),
            maybe_space(pair(bracket_end, semicolon)),
        ),
    )(inp)
}

pub fn parse(tokens: Vec<RawToken>) -> Result<GvNetwork, ParseError> {
    let tokens = Token::validate(tokens)?;
    match gv_file(&tokens).finish() {
        Ok((rest, paths)) => {
            if rest.is_empty() {
                Ok(paths)
            } else {
                match maybe_newline(gv_network)(rest).finish() {
                    Ok((rest, _)) => {
                        Err(ParseError::new(&tokens, rest, ParseErrorType::SyntaxError))
                    }
                    Err(err) => Err(ParseError::new(&tokens, err.internal.input, err.ty)),
                }
            }
        }
        Err(e) => Err(ParseError::new(&tokens, e.internal.input, e.ty)),
    }
}
