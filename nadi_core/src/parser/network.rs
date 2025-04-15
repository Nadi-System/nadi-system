use crate::parser::tokenizer::{TaskToken, Token, VecTokens};
use crate::parser::{ParseError, ParseErrorType};
use nadi_core::network::StrPath;

#[derive(Debug, PartialEq)]
pub enum State {
    PathSep(String),
    Output(String),
    Newline(String, String),
    None,
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<StrPath>, ParseError> {
    let mut tokens = VecTokens::new(tokens);
    let mut state = State::None;
    let mut paths = vec![];
    let mut token;
    loop {
        token = match tokens.next() {
            Some(t) => t,
            None => break,
        };
        match token.ty {
            TaskToken::WhiteSpace | TaskToken::Comment => (),
            TaskToken::NewLine => match state {
                State::Newline(s, e) => {
                    state = State::None;
                    paths.push(StrPath::new(s.into(), e.into()));
                }
                State::None => (),
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            // // TODO: connection attributes [key=val,...] format
            // TaskToken::BracketStart => {
            // 	match state {
            // 	    State::Newline(s, e) => {
            // 		state = State::None;
            // 		paths.push(StrPath::new(s.into(), e.into()));

            // 	    }
            // 	    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            // 	}
            // },
            TaskToken::Variable | TaskToken::Integer | TaskToken::Bool => match state {
                State::None => {
                    state = State::PathSep(token.content.to_string());
                }
                State::Output(s) => {
                    state = State::Newline(s, token.content.to_string());
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::String(s) => match state {
                State::None => {
                    state = State::PathSep(s);
                }
                State::Output(s2) => {
                    state = State::Newline(s2, s);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::PathSep => match state {
                State::PathSep(s) => {
                    state = State::Output(s);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            _ => return Err(tokens.parse_error(ParseErrorType::InvalidToken)),
        }
    }
    if state != State::None {
        Err(tokens.parse_error(ParseErrorType::Unclosed))
    } else {
        Ok(paths)
    }
}
