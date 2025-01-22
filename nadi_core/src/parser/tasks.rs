use crate::functions::{Condition, Propagation};
use crate::network::StrPath;
use crate::parser::tokenizer::{TaskToken, Token, VecTokens};
use crate::parser::{ParseError, ParseErrorType};
use crate::prelude::*;
use crate::tasks::{FunctionCall, Task, TaskInput, TaskKeyword, TaskType};
use abi_stable::std_types::{RBox, RString, RVec};
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
    let mut prev_states = vec![];
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
                            None => return Err(tokens.parse_error(ParseErrorType::LogicalError(
                                "Last kw can't be none if it's in Attribute or Assignment state",
                            ))),
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
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                tasks.push(Task {
                                    ty: TaskType::Network(prop),
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
                    TaskKeyword::Node | TaskKeyword::Network => {
                        state = State::Propagation;
                    }
                    TaskKeyword::Env => {
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
            TaskToken::AngleStart => match state {
                State::Propagation => {
                    match read_propagation(&mut tokens)? {
                        Some(p) => {
                            propagation.replace(p);
                        }
                        None => break,
                    }
                    state = State::Attribute;
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::ParenStart => match state {
                State::Propagation => {
                    match read_conditional(&mut tokens)? {
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
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Network(prop)
                        }
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
                    let name =
                        name.take()
                            .ok_or(tokens.parse_error(ParseErrorType::LogicalError(
                                "should be Some based on prev pattern",
                            )))?;
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
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Network(prop)
                        }
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
                    let name =
                        name.take()
                            .ok_or(tokens.parse_error(ParseErrorType::LogicalError(
                                "should be Some based on prev pattern",
                            )))?;
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
                }
                State::Attribute => (),
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::ParenEnd => {
                // propagation ParenEnd doesn't reach here
                match state {
                    State::FuncArgs(fc) | State::FuncKeyArgs(None, fc) => {
                        let kw = if prev_states.is_empty() {
                            curr_keyword.take()
                        } else {
                            curr_keyword.clone()
                        }
                        .ok_or(tokens.parse_error(
                            ParseErrorType::LogicalError("current keyword can't be empty"),
                        ))?;
                        let ty = match kw {
                            TaskKeyword::Node => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                TaskType::Node(prop)
                            }
                            TaskKeyword::Network => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                TaskType::Network(prop)
                            }
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        match prev_states.pop() {
                            Some(s) => match s {
                                State::FuncArgs(mut pfc) => {
                                    pfc.args.push(TaskInput::Function(fc));
                                    state = State::FuncArgs(pfc);
                                }
                                State::FuncKeyArgs(Some(a), mut pfc) => {
                                    pfc.kwargs.insert(a.into(), TaskInput::Function(fc));
                                    state = State::FuncKeyArgs(None, pfc);
                                }
                                _ => {
                                    return Err(tokens.parse_error(ParseErrorType::LogicalError(
                                        "Prev State should be function arg to get here",
                                    )))
                                }
                            },
                            None => {
                                let tsk = Task {
                                    ty,
                                    attribute: output.take(),
                                    input: TaskInput::Function(fc),
                                };
                                tasks.push(tsk);
                                state = State::None;
                            }
                        }
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
                let var = read_variable(token.content, &mut tokens)?;
                match state {
                    State::Help(hkw) => {
                        tasks.push(Task::help(hkw, Some(var)));
                        state = State::None;
                    }
                    State::PropagationList | State::PropagationPath => {
                        data.push(var);
                    }
                    State::Attribute => {
                        output = Some(var);
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
                            Some(TaskKeyword::Network) => {
                                let prop = propagation
                                    .replace(Propagation::default())
                                    .unwrap_or_default();
                                TaskType::Network(prop)
                            }
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        tasks.push(Task {
                            ty,
                            attribute: output.take(),
                            input: TaskInput::Variable(var),
                        });
                        state = State::None;
                    }
                    State::FuncArgs(ref mut fc) => {
                        if let Some(t) = tokens.peek_next_no_ws(true) {
                            match t.ty {
                                TaskToken::Comma | TaskToken::ParenEnd => {
                                    fc.args.push(TaskInput::Variable(var));
                                }
                                TaskToken::Assignment => {
                                    state = State::FuncKeyArgs(Some(var), fc.clone())
                                }
                                _ => (), // is an error anyway
                            }
                        }
                    }
                    State::FuncKeyArgs(None, fc) => state = State::FuncKeyArgs(Some(var), fc),
                    State::FuncKeyArgs(ref mut name, ref mut fc) => {
                        let name = name
                            .take()
                            .expect("has to be Some based on the pattern above");
                        fc.kwargs.insert(name.into(), TaskInput::Variable(var));
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
            TaskToken::Function => match state {
                State::Attribute | State::Propagation | State::Rhs => {
                    if let Some(TaskKeyword::Env) = curr_keyword {
                        return Err(tokens.parse_error(ParseErrorType::ValueError(
                            "Env Rhs can only be literal for now",
                        )));
                    } else {
                        state = State::Function(token.content.to_string());
                    }
                }
                State::FuncArgs(_) | State::FuncKeyArgs(Some(_), _) => {
                    prev_states.push(state);
                    state = State::Function(token.content.to_string());
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
                        Some(TaskKeyword::Network) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Network(prop)
                        }
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
                    _ => {
                        return Err(
                            tokens.parse_error(ParseErrorType::ValueError("Invalid Node Name"))
                        )
                    }
                },
                State::Rhs => {
                    let ty = match curr_keyword {
                        Some(TaskKeyword::Node) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Node(prop)
                        }
                        Some(TaskKeyword::Network) => {
                            let prop = propagation
                                .replace(Propagation::default())
                                .unwrap_or_default();
                            TaskType::Network(prop)
                        }
                        Some(TaskKeyword::Env) => TaskType::Env,
                        _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    };
                    match token.attribute() {
                        Ok(Some(v)) => {
                            tasks.push(Task {
                                ty,
                                attribute: output.take(),
                                input: TaskInput::Literal(v),
                            });
                        }
                        Ok(None) => {
                            return Err(
                                tokens.parse_error(ParseErrorType::ValueError("Not an Attribute"))
                            )
                        }
                        Err(e) => return Err(tokens.parse_error(ParseErrorType::ValueError(e))),
                    }
                    state = State::None;
                }
                State::FuncArgs(ref mut fc) => match token.attribute() {
                    Ok(Some(v)) => {
                        fc.args.push(TaskInput::Literal(v));
                    }
                    Ok(None) => {
                        return Err(
                            tokens.parse_error(ParseErrorType::ValueError("Not an Attribute"))
                        )
                    }
                    Err(e) => return Err(tokens.parse_error(ParseErrorType::ValueError(e))),
                },
                State::FuncKeyArgs(ref mut var, ref mut fc) => match token.attribute() {
                    Ok(Some(v)) => match var.take() {
                        Some(var) => {
                            fc.kwargs.insert(var.into(), TaskInput::Literal(v));
                        }
                        None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                    },
                    Ok(None) => {
                        return Err(
                            tokens.parse_error(ParseErrorType::ValueError("Not an Attribute"))
                        )
                    }
                    Err(e) => return Err(tokens.parse_error(ParseErrorType::ValueError(e))),
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
                TaskKeyword::Network => TaskType::Network(propagation.take().unwrap_or_default()),
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
                        _ => {
                            return Err(tokens
                                .parse_error(ParseErrorType::ValueError("Key can only be string")))
                        }
                    }
                }
            }
            Err(tokens.parse_error(ParseErrorType::Unclosed))
        }
        _ => match tk.attribute() {
            Ok(Some(v)) => Ok(Some(v)),
            Ok(None) => Err(tokens.parse_error(ParseErrorType::ValueError("Not an Attribute"))),
            Err(e) => Err(tokens.parse_error(ParseErrorType::ValueError(e))),
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
    match tokens.next_no_ws(true) {
        None => Ok(None),
        Some(tk) => match tk.ty {
            TaskToken::AngleEnd => Ok(Some(prop)),
            _ => Err(tokens.parse_error(ParseErrorType::Unclosed)),
        },
    }
}

enum CompType {
    Eq,
    Lt,
    Gt,
}

enum CondState {
    FirstVar(i64),
    Not,
    Cond(Condition),
    SecondVar(Condition, bool),
    SecondValue(Condition, CompType),
}

fn read_conditional(tokens: &mut VecTokens) -> Result<Option<Propagation>, ParseError> {
    let mut state = CondState::FirstVar(0);
    let mut strict = 0;
    let cond = loop {
        let tk = match tokens.next_no_ws(true) {
            None => return Ok(None),
            Some(t) => t,
        };
        match tk.ty {
            TaskToken::Assignment => match state {
                CondState::FirstVar(i) => {
                    if i > 1 {
                        return Err(tokens.parse_error(ParseErrorType::SyntaxError));
                    }
                    state = CondState::FirstVar(i + 1);
                }
                CondState::Cond(s) => {
                    state = CondState::SecondValue(s, CompType::Eq);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::AngleStart => match state {
                CondState::Cond(s) => {
                    state = CondState::SecondValue(s, CompType::Lt);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::AngleEnd => match state {
                CondState::Cond(s) => {
                    state = CondState::SecondValue(s, CompType::Gt);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::And => match state {
                CondState::Cond(s) => {
                    state = CondState::SecondVar(s, true);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Or => match state {
                CondState::Cond(s) => {
                    state = CondState::SecondVar(s, false);
                }
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::Not => match state {
                CondState::FirstVar(i) => {
                    strict = i;
                    state = CondState::Not;
                }
                CondState::SecondVar(f, a) => match tokens.next_no_ws(true) {
                    Some(t) => {
                        let var = match t.ty {
                            TaskToken::Variable => tk.content.to_string(),
                            TaskToken::String(s) => s,
                            _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                        };
                        let cv = Condition::Not(RBox::new(Condition::Variable(var.into())));
                        let cond = if a {
                            Condition::And(RBox::new(f), RBox::new(cv))
                        } else {
                            Condition::Or(RBox::new(f), RBox::new(cv))
                        };
                        state = CondState::Cond(cond);
                    }
                    None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                },
                _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            },
            TaskToken::ParenEnd => {
                if let CondState::Cond(s) = state {
                    break s;
                } else {
                    return Err(tokens.parse_error(ParseErrorType::Unclosed));
                }
            }
            ref ty => {
                let cd = match ty {
                    TaskToken::Variable => {
                        Condition::Variable(read_variable(tk.content, tokens)?.into())
                    }
                    _ => match tk.attribute() {
                        Ok(Some(a)) => Condition::Literal(a),
                        Ok(None) => {
                            return Err(tokens.parse_error(ParseErrorType::InvalidPropagation))
                        }
                        Err(e) => return Err(tokens.parse_error(ParseErrorType::ValueError(e))),
                    },
                };
                match state {
                    CondState::FirstVar(i) => {
                        strict = i;
                        state = CondState::Cond(cd);
                    }
                    CondState::Not => {
                        state = CondState::Cond(Condition::Not(RBox::new(cd)));
                    }
                    CondState::SecondVar(f, a) => {
                        let cond = if a {
                            Condition::And(RBox::new(f), RBox::new(cd))
                        } else {
                            Condition::Or(RBox::new(f), RBox::new(cd))
                        };
                        state = CondState::Cond(cond);
                    }
                    CondState::SecondValue(s, ct) => {
                        let var = match s {
                            Condition::Variable(v) => v,
                            _ => return Err(tokens.parse_error(ParseErrorType::InvalidPropagation)),
                        };
                        let cond = match ct {
                            CompType::Eq => Condition::Eq(var, RBox::new(cd)),
                            CompType::Lt => Condition::Lt(var, RBox::new(cd)),
                            CompType::Gt => Condition::Gt(var, RBox::new(cd)),
                        };
                        state = CondState::Cond(cond);
                    }
                    _ => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
                }
            }
        }
    };
    let prop = match strict {
        0 => Propagation::Conditional(cond),
        1 => Propagation::ConditionalStrict(cond),
        _ => Propagation::ConditionalSuperStrict(cond),
    };
    Ok(Some(prop))
}

fn read_variable(pre: &str, tokens: &mut VecTokens) -> Result<String, ParseError> {
    let mut names = vec![pre];
    loop {
        if tokens.next_no_ws_if(true, TaskToken::Dot).is_some() {
            match tokens.next_no_ws_if(true, TaskToken::Variable) {
                Some(t) => names.push(&t.content),
                None => return Err(tokens.parse_error(ParseErrorType::SyntaxError)),
            }
        } else {
            break;
        }
    }
    Ok(names.join("."))
}
