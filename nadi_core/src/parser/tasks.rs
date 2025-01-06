use crate::functions::Propagation;
use crate::network::StrPath;
use crate::parser::tokenizer::{TaskToken, Token, VecTokens};
use crate::parser::{ParseError, ParseErrorType};
use crate::prelude::*;
use crate::tasks::{FunctionCall, Task, TaskInput, TaskKeyword, TaskType};
use abi_stable::std_types::{RString, RVec};
use std::collections::HashMap;

#[derive(Clone, PartialEq, Debug)]
enum State {
    None,
    Help(Option<TaskKeyword>),
    Propagation,
    PropagationList,
    PropagationPath,
    Attribute,
    Assignment,
    Rhs,
    Function(String),
    FuncArgs(FunctionCall),
    FuncKeyArgs(Option<String>, FunctionCall),
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Task>, ParseError> {
    let mut tokens = VecTokens::new(tokens);
    let mut curr_keyword = None;
    let mut data: Vec<String> = vec![];
    let mut propagation: Option<Propagation> = None;
    let mut output: Option<String> = None;
    let mut state = State::None;
    let mut tasks: Vec<Task> = vec![];
    let mut token;
    loop {
        token = match tokens.next() {
            Some(t) => t,
            None => break,
        };
        // println!("{token:?} {state:?}");
        match token.ty {
            TaskToken::NewLine | TaskToken::Comment | TaskToken::WhiteSpace => (),
            TaskToken::Keyword(kw) => {
                match state {
                    State::None => (),
                    State::Attribute | State::Assignment => {
                        let last_kw = match curr_keyword.replace(kw.clone()) {
                            Some(k) => k,
                            None => panic!(
                                "Last kw can't be none if it's in Attribute or Assignment state"
                            ),
                        };
                        match last_kw {
                            TaskKeyword::Exit => (), // last one shouldn't be exit as it'd exit earlier
                            TaskKeyword::Node => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                tasks.push(Task {
                                    ty: TaskType::Node(prop),
                                    attribute: output.take(),
                                    input: TaskInput::None,
                                });
                            }
                            TaskKeyword::Network => {
                                tasks.push(Task {
                                    ty: TaskType::Network,
                                    attribute: output.take(),
                                    input: TaskInput::None,
                                });
                            }
                            TaskKeyword::Env => {
                                tasks.push(Task {
                                    ty: TaskType::Env,
                                    attribute: output.take(),
                                    input: TaskInput::None,
                                });
                            }
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        }
                    }
                    State::Help(None) => {
                        state = State::Help(Some(kw));
                        continue;
                    }
                    State::Help(Some(hkw)) => {
                        tasks.push(Task::help(Some(hkw), None));
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
                match kw {
                    TaskKeyword::Node => {
                        state = State::Propagation;
                    }
                    TaskKeyword::Network | TaskKeyword::Env => {
                        state = State::Attribute;
                    }
                    TaskKeyword::Help => {
                        state = State::Help(None);
                    }
                    TaskKeyword::Exit => {
                        tasks.push(Task::exit());
                        return Ok(tasks);
                    }
                }
                curr_keyword = Some(kw.clone());
            }
            TaskToken::ParenStart => match state {
                State::Propagation => {
                    match read_propagation(&mut tokens)? {
                        Some(p) => {
                            propagation.replace(p);
                        }
                        None => break,
                    }
                    state = State::Attribute;
                }
                State::Function(ref s) => {
                    let fc = FunctionCall {
                        name: s.to_string(),
                        args: vec![],
                        kwargs: HashMap::new(),
                    };
                    state = State::FuncArgs(fc);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::BraceStart => match state {
		State::Rhs => {
                    let inp = match read_attribute(Some(token.clone()), &mut tokens, true)? {
			Some(a) => a,
			None => return Err(tokens.parse_error(ParseErrorType::SyntaxError))
		    };
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => TaskType::Network,
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    tasks.push(Task {
                        ty,
                        attribute: output.take(),
                        input: TaskInput::Literal(inp),
                    });
                    state = State::None;
		}
                State::FuncArgs(ref mut fc) => {
                    let inp = read_input(Some(token.clone()), &mut tokens)?;
                    fc.args.push(inp);
                    if let Some(t) = tokens.peek_next_no_ws(true) {
                        match t.ty {
                            TaskToken::Comma | TaskToken::ParenEnd => (),
                            TaskToken::Assignment => {
                                return Err(tokens.parse_error(ParseErrorType::SyntaxError))
                            }
                            _ => (), // is an error anyway
                        }
                    }
                }
                State::FuncKeyArgs(None, _) => {
                    return Err(tokens.parse_error(ParseErrorType::SyntaxError))
                }

                State::FuncKeyArgs(ref mut name, mut fc) => {
                    let name = name.take().expect("should be Some based on prev pattern");
                    let inp = read_input(Some(token.clone()), &mut tokens)?;
                    fc.kwargs.insert(name, inp);

                    state = State::FuncKeyArgs(None, fc);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::BracketStart => match state {
                State::Propagation => {
                    state = State::PropagationList;
                }
		State::Rhs => {
                    let inp = match read_attribute(Some(token.clone()), &mut tokens, true)? {
			Some(a) => a,
			None => return Err(tokens.parse_error(ParseErrorType::SyntaxError))
		    };
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => TaskType::Network,
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    tasks.push(Task {
                        ty,
                        attribute: output.take(),
                        input: TaskInput::Literal(inp),
                    });
                    state = State::None;
		}
                State::FuncArgs(ref mut fc) => {
                    let inp = read_input(Some(token.clone()), &mut tokens)?;
                    fc.args.push(inp);
                    if let Some(t) = tokens.peek_next_no_ws(true) {
                        match t.ty {
                            TaskToken::Comma | TaskToken::ParenEnd => (),
                            // dictionary can't be key
                            TaskToken::Assignment => {
                                return Err(tokens.parse_error(ParseErrorType::SyntaxError))
                            }
                            _ => (), // is an error anyway
                        }
                    }
                }
                State::FuncKeyArgs(None, _) => {
                    return Err(tokens.parse_error(ParseErrorType::SyntaxError))
                }

                State::FuncKeyArgs(ref mut name, mut fc) => {
                    let name = name.take().expect("should be Some based on prev pattern");
                    let inp = read_input(Some(token.clone()), &mut tokens)?;
                    fc.kwargs.insert(name, inp);

                    state = State::FuncKeyArgs(None, fc);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::PathSep => match state {
                State::PropagationList => {
                    state = State::PropagationPath;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Comma => match state {
                State::PropagationList | State::FuncArgs(_) | State::FuncKeyArgs(_, _) => (),
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Dot => match state {
                State::Propagation => {
		    state = State::Attribute;
		},
                State::Attribute => (),
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::ParenEnd => {
                // propagation ParenEnd doesn't reach here
                match state {
                    State::FuncArgs(ref fc) | State::FuncKeyArgs(None, ref fc) => {
                        let ty = match curr_keyword.take().expect("current keyword can't be empty")
                        {
                            TaskKeyword::Node => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                TaskType::Node(prop)
                            }
                            TaskKeyword::Network => TaskType::Network,
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        tasks.push(Task {
                            ty,
                            attribute: output.take(),
                            input: TaskInput::Function(fc.clone()),
                        });
                        state = State::None;
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
            TaskToken::BraceEnd => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            TaskToken::BracketEnd => {
                match state {
                    State::PropagationList => {
                        let mut lst = RVec::with_capacity(data.len());
                        data.drain(..).for_each(|s| {
                            lst.push(RString::from(s));
                        });
                        propagation = Some(Propagation::List(lst));
                        state = State::Attribute;
                    }
                    State::PropagationPath => {
                        // since we only change to PropagationPath after encountering -> it should have at least 1 member
                        let start = data[0].to_string();
                        let end = match data.get(1) {
                            Some(e) => e.to_string(),
                            None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
			data = vec![];
                        propagation =
                            Some(Propagation::Path(StrPath::new(start.into(), end.into())));
                        state = State::Attribute;
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
            TaskToken::Variable => {
                match state {
                    State::Help(hkw) => {
                        tasks.push(Task::help(hkw, Some(token.content.to_string())));
                        state = State::None;
                    }
                    State::PropagationList | State::PropagationPath => {
                        data.push(token.content.to_string());
                    }
                    State::Attribute => {
                        output = Some(token.content.to_string());
                        state = State::Assignment;
                    }
                    State::Rhs => {
                        let ty = match curr_keyword {
                            Some(TaskKeyword::Node) => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                TaskType::Node(prop)
                            }
                            Some(TaskKeyword::Network) => TaskType::Network,
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        tasks.push(Task {
                            ty,
                            attribute: output.take(),
                            input: TaskInput::Variable(token.content.to_string()),
                        });
                        state = State::None;
                    }
                    State::FuncArgs(ref mut fc) => {
                        if let Some(t) = tokens.peek_next_no_ws(true) {
                            match t.ty {
                                TaskToken::Comma | TaskToken::ParenEnd => {
                                    fc.args.push(TaskInput::Variable(token.content.to_string()));
                                }
                                TaskToken::Assignment => {
                                    state = State::FuncKeyArgs(
                                        Some(token.content.to_string()),
                                        fc.clone(),
                                    )
                                }
                                _ => (), // is an error anyway
                            }
                        }
                    }
                    State::FuncKeyArgs(None, fc) => {
                        state = State::FuncKeyArgs(Some(token.content.to_string()), fc)
                    }
                    State::FuncKeyArgs(ref mut name, ref mut fc) => {
                        let name = name
                            .take()
                            .expect("has to be Some based on the pattern above");
                        fc.kwargs
                            .insert(name.into(), TaskInput::Variable(token.content.to_string()));
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
            TaskToken::Function => match state {
                State::Attribute | State::Propagation | State::Rhs => {
                    if let Some(TaskKeyword::Env) = curr_keyword {
                        // env rhs can only be literal values
                        return Err(tokens.parse_error(ParseErrorType::ValueError));
                    } else {
                        state = State::Function(token.content.to_string());
                    }
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Assignment => match state {
                State::Assignment => {
                    state = State::Rhs;
                }
                State::FuncKeyArgs(Some(_), _) => (),
                State::FuncKeyArgs(None, _) => {
                    return Err(tokens.parse_error(ParseErrorType::SyntaxError))
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::String(ref s) => match state {
                State::PropagationList | State::PropagationPath => {
                    data.push(s.to_string());
                }
                State::Rhs => {
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => TaskType::Network,
                        Some(TaskKeyword::Env) => TaskType::Env,
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    tasks.push(Task {
                        ty,
                        attribute: output.take(),
                        input: TaskInput::Literal(s.to_string().into()),
                    });
                    state = State::None;
                }
                State::FuncArgs(ref mut fc) => {
                    if let Some(t) = tokens.peek_next_no_ws(true) {
                        match t.ty {
                            TaskToken::Comma | TaskToken::ParenEnd => {
                                fc.args
                                    .push(TaskInput::Literal(Attribute::String(s.as_str().into())));
                            }
                            TaskToken::Assignment => {
                                state = State::FuncKeyArgs(Some(s.clone()), fc.clone())
                            }
                            _ => (), // is an error anyway
                        }
                    }
                }
                State::FuncKeyArgs(ref mut out, ref mut fc) => {
                    let out2 = out.take();
                    match out2 {
                        Some(o) => {
                            fc.kwargs
                                .insert(o.into(), TaskInput::Literal(s.to_string().into()));
                        }
                        None => {
                            out.replace(s.to_string());
                        }
                    }
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            _ => match state {
                State::PropagationList | State::PropagationPath => match token.ty {
                    TaskToken::Bool | TaskToken::Integer => {
                        data.push(token.content.to_string());
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::ValueError)),
                },
                State::Rhs => {
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => TaskType::Network,
                        Some(TaskKeyword::Env) => TaskType::Env,
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    match token.attribute() {
                        Some(v) => {
                            tasks.push(Task {
                                ty,
                                attribute: output.take(),
                                input: TaskInput::Literal(v),
                            });
                        }
                        None => return Err(tokens.parse_error(ParseErrorType::ValueError)),
                    }
                    state = State::None;
                }
                State::FuncArgs(ref mut fc) => match token.attribute() {
                    Some(v) => {
                        fc.args.push(TaskInput::Literal(v));
                    }
                    None => return Err(tokens.parse_error(ParseErrorType::ValueError)),
                },
                State::FuncKeyArgs(ref mut var, ref mut fc) => match token.attribute() {
                    Some(v) => match var.take() {
                        Some(var) => {
                            fc.kwargs.insert(var.into(), TaskInput::Literal(v));
                        }
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    },
                    None => return Err(tokens.parse_error(ParseErrorType::ValueError)),
                },
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
        }
    }
    match (state, curr_keyword) {
        (State::None, _) => Ok(tasks),
        (State::Assignment | State::Attribute, Some(kw)) => {
	    let ty = match kw {
		TaskKeyword::Env => TaskType::Env,
		TaskKeyword::Node => TaskType::Node(propagation.take().unwrap_or_default()),
		TaskKeyword::Network => TaskType::Network,
		TaskKeyword::Help => TaskType::Help(None, None),
		TaskKeyword::Exit => TaskType::Exit,
	    };
            tasks.push(Task {
                ty,
                attribute: output.take(),
                input: TaskInput::None,
            });
            Ok(tasks)
        }
        _ => Err(tokens.parse_error(ParseErrorType::Unclosed)),
    }
}

fn read_input(start: Option<Token>, tokens: &mut VecTokens) -> Result<TaskInput, ParseError> {
    let tk = match start.or_else(|| tokens.next_no_ws(true)) {
        None => return Ok(TaskInput::None),
        Some(t) => t,
    };
    match tk.ty {
        TaskToken::Variable => Ok(TaskInput::Variable(tk.content.to_string())),
        _ => Ok(match read_attribute(Some(tk), tokens, true)? {
            Some(a) => TaskInput::Literal(a),
            None => TaskInput::None,
        }),
    }
}

pub fn read_attribute(
    start: Option<Token>,
    tokens: &mut VecTokens,
    newline: bool,
) -> Result<Option<Attribute>, ParseError> {
    let tk = match start.or_else(|| tokens.next_no_ws(newline)) {
        None => return Ok(None),
        Some(t) => t,
    };
    match tk.ty {
        // todo better error here
        TaskToken::NewLine if !newline => return Err(tokens.parse_error(ParseErrorType::Unclosed)),
        TaskToken::BracketStart => {
            let mut vals = vec![];
            let mut want_comma = false;
            while let Some(t) = tokens.next_no_ws(newline) {
                if want_comma {
                    match t.ty {
                        TaskToken::Comma => {
                            want_comma = false;
                            continue;
                        }
                        TaskToken::BracketEnd => return Ok(Some(Attribute::Array(vals.into()))),
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    }
                }
                if let Some(a) = read_attribute(Some(t), tokens, newline)? {
                    vals.push(a);
                    want_comma = true;
                }
                // else it could be empty [], or error if the next tag is not `]`
            }
            Err(tokens.parse_error(ParseErrorType::Unclosed))
        }
        TaskToken::BraceStart => {
            let mut vals = HashMap::new();
            let mut want_eq = false;
            let mut want_comma = false;
            let mut name = None;
            while let Some(t) = tokens.next_no_ws(newline) {
                if want_eq {
                    match t.ty {
                        TaskToken::Assignment => {
                            want_eq = false;
                            continue;
                        }
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    }
                }
                if want_comma {
                    match t.ty {
                        TaskToken::Comma => {
                            want_comma = false;
                            continue;
                        }
                        TaskToken::BraceEnd => return Ok(Some(Attribute::Table(vals.into()))),
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    }
                }
                let val = name.take();
                if let Some(val) = val {
                    // has name needs value
                    match read_attribute(Some(t), tokens, newline)? {
                        Some(a) => {
                            vals.insert(val, a);
                            want_eq = false;
                            want_comma = true;
                        }
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    }
                } else {
                    // needs name/key
                    want_eq = true;
                    match t.ty {
                        TaskToken::Variable => {
                            name = Some(t.content.to_string().into());
                        }
                        TaskToken::String(ref s) => {
                            name = Some(s.to_string().into());
                        }
                        _ => return Err(tokens.parse_error(ParseErrorType::ValueError)),
                    }
                }
            }
            Err(tokens.parse_error(ParseErrorType::Unclosed))
        }
        _ => match tk.attribute() {
            Some(a) => Ok(Some(a)),
            None => Err(tokens.parse_error(ParseErrorType::ValueError)),
        },
    }
}

fn read_propagation(tokens: &mut VecTokens) -> Result<Option<Propagation>, ParseError> {
    let tk = match tokens.next_no_ws(true) {
        None => return Ok(None),
        Some(t) => t,
    };
    let prop = match tk.ty {
        TaskToken::Variable => tk.content.to_string(),
        TaskToken::String(s) => s,
        _ => return Err(tokens.parse_error(ParseErrorType::InvalidPropagation)),
    };
    let prop = match prop.as_str() {
        "sequential" => Propagation::Sequential,
        "inverse" => Propagation::Inverse,
        "inputsfirst" => Propagation::InputsFirst,
        "outputfirst" => Propagation::OutputFirst,
        _ => return Err(tokens.parse_error(ParseErrorType::InvalidPropagation)),
    };
    let tk = match tokens.next_no_ws(true) {
        None => return Ok(None),
        Some(t) => t,
    };
    match tk.ty {
        TaskToken::ParenEnd => (),
        _ => return Err(tokens.parse_error(ParseErrorType::Unclosed)),
    };
    Ok(Some(prop))
}
