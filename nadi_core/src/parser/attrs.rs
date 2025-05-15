use crate::attrs::{AttrMap, Attribute, HasAttributes};
use crate::parser::{
    components::*,
    errors::ParseError,
    tokenizer::{check_tokens, Token},
};
use nom::{branch::alt, combinator::map, sequence::delimited, Finish};

pub fn attr_group<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<String>> {
    delimited(
        bracket_start,
        maybe_space(dot_variable),
        maybe_space(bracket_end),
    )(inp)
}

pub enum Line {
    Group(Vec<String>),
    KeyVal((Vec<String>, Attribute)),
}

pub fn attr_file_line<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Line> {
    alt((map(attr_group, Line::Group), map(key_val_dot, Line::KeyVal)))(inp)
}

pub fn attr_file<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Line>> {
    trailing_newlines(newline_separated(attr_file_line))(inp)
}

pub fn parse(tokens: Vec<Token>) -> Result<AttrMap, ParseError> {
    check_tokens(&tokens)?;
    let lines = match attr_file(&tokens).finish() {
        Ok((rest, lines)) => {
            if rest.is_empty() {
                lines
            } else {
                let err = maybe_newline(attr_file_line)(rest)
                    .finish()
                    .err()
                    .expect("Rest should be empty if network parse is complete");
                return Err(ParseError::new(&tokens, err.internal.input, err.ty));
            }
        }
        Err(e) => return Err(ParseError::new(&tokens, e.internal.input, e.ty)),
    };

    let mut attrmap = AttrMap::new();
    let mut curr_var = &mut attrmap;

    for line in lines {
        match line {
            Line::Group(grp) => {
                curr_var = move_in(&grp, curr_var)?;
            }
            Line::KeyVal((keys, val)) => {
                let old = match keys.as_slice() {
                    [] => return Err(ParseError::custom("Empty attribute group".into())),
                    [name] => curr_var.set_attr(name, val.clone()),
                    [pre @ .., name] => {
                        let map = move_in(pre, curr_var)?;
                        map.set_attr(name, val.clone())
                    }
                };
                if let Some(oval) = old {
                    return Err(ParseError::custom(format!(
                        "Key {} already set to value {} (new: {})",
                        keys.join("."),
                        val.to_string(),
                        oval.to_string()
                    )));
                }
            }
        };
    }
    Ok(attrmap)
}

fn move_in<'a>(keys: &[String], table: &'a mut AttrMap) -> Result<&'a mut AttrMap, ParseError> {
    let mut map = table;
    for k in keys {
        map = match map
            .entry(k.to_string().into())
            .or_insert(Attribute::Table(AttrMap::new()))
        {
            Attribute::Table(ref mut mp) => mp,
            val => {
                return Err(ParseError::custom(format!(
                    "Key {k} in {} is not a table (value: {})",
                    keys.join("."),
                    val.to_string(),
                )))
            }
        };
    }
    Ok(map)
}
