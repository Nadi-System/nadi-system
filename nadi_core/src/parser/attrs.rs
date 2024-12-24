use crate::attrs::{AttrMap, Attribute};
use crate::parser::tasks::read_attribute;
use crate::parser::tokenizer::{TaskToken, Token, VecTokens};
use crate::parser::{ParseError, ParseErrorType};
use abi_stable::std_types::{map::REntry, RString};

#[derive(Debug, PartialEq)]
pub enum State {
    AttrGroup(Option<String>),
    Assignment(String),
    Variable(String),
    Newline,
    None,
}

pub fn parse(tokens: Vec<Token>) -> Result<AttrMap, ParseError> {
    let mut tokens = VecTokens::new(tokens);
    let mut attrmap = AttrMap::new();
    let mut curr_grp: Vec<String> = vec![];
    let mut curr_var = &mut attrmap;
    let mut state = State::None;
    let mut token;
    loop {
        token = match tokens.next() {
            Some(t) => t,
            None => break,
        };
        match token.ty {
            TaskToken::WhiteSpace | TaskToken::Comment => (),
            TaskToken::NewLine => match state {
                State::Newline => {
                    state = State::None;
                    curr_var = &mut attrmap;
                    for g in &curr_grp {
                        curr_var = move_in(g, curr_var, &tokens)?;
                    }
                }
                State::None => (),
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::BracketStart => {
                match state {
                    State::None => {
                        // if it's start of `[` for group then reset
                        // the current group to root
                        curr_var = &mut attrmap;
                        curr_grp = vec![];
                        state = State::AttrGroup(None);
                    }
                    State::Variable(s) => {
                        let inp = match read_attribute(Some(token.clone()), &mut tokens, false)? {
                            Some(i) => i,
                            None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        curr_var.insert(s.into(), inp);
                        state = State::Newline;
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
            TaskToken::BraceStart => match state {
                State::Variable(s) => {
                    let inp = match read_attribute(Some(token.clone()), &mut tokens, false)? {
                        Some(i) => i,
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    curr_var.insert(s.into(), inp);
                    state = State::Newline;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Dot => match state {
                State::AttrGroup(Some(s)) => {
                    curr_var = move_in(&s, curr_var, &tokens)?;
                    curr_grp.push(s);
                    state = State::AttrGroup(None);
                }
                State::Assignment(s) => {
                    curr_var = move_in(&s, curr_var, &tokens)?;
                    state = State::None;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::BracketEnd => match state {
                State::AttrGroup(Some(n)) => {
                    curr_var = move_in(&n, curr_var, &tokens)?;
                    curr_grp.push(n);
                    state = State::None;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Variable => match state {
                State::None => {
                    state = State::Assignment(token.content.to_string());
                }
                State::AttrGroup(None) => {
                    state = State::AttrGroup(Some(token.content.to_string()));
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Assignment => match state {
                State::Assignment(s) => {
                    let inp = match read_attribute(None, &mut tokens, false)? {
                        Some(i) => i,
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    curr_var.insert(s.into(), inp);
                    state = State::Newline;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Bool => (),
            TaskToken::String(s) => match state {
                State::None => {
                    state = State::Assignment(s);
                }
                State::AttrGroup(None) => {
                    state = State::AttrGroup(Some(s));
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Integer => (),
            TaskToken::Date => (),
            TaskToken::Float => (),
            TaskToken::DateTime => (),
            TaskToken::Time => (),
            _ => return Err(tokens.parse_error(ParseErrorType::InvalidToken)),
        }
    }
    match state {
	State::None | State::Newline => Err(tokens.parse_error(ParseErrorType::Unclosed)),
	_ => Ok(attrmap),
    }
}

fn move_in<'a>(
    key: &str,
    table: &'a mut AttrMap,
    tokens: &VecTokens,
) -> Result<&'a mut AttrMap, ParseError> {
    let key: RString = key.into();
    let tb = match table.entry(key) {
        REntry::Occupied(o) => o.into_mut(),
        REntry::Vacant(v) => v.insert(Attribute::Table(AttrMap::new())),
    };
    match tb {
        Attribute::Table(t) => Ok(t),
        // TODO better error message (say value already present and not table)
        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
    }
}
