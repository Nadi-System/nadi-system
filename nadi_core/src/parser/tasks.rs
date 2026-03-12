use crate::{
    expressions::{MapFunction, SeriesExpression, TaskPosition},
    network::{PropCondition, PropNodes, PropOrder, Propagation},
    structs::NadiAttrType,
    tasks::{AttrTask, CondTask, EvalTask, FunctionType, ImportTask, Task, WhileTask},
};
use crate::{
    parser::{
        components::*,
        errors::MatchErr,
        expressions::{complete_expression, expression_group, function_def, variable_type},
        network::{node_name, str_path},
        tokenizer::{RawToken, Token},
        ParseError, ParseErrorType,
    },
    tasks::{GetSeriesTask, SetSeriesTask},
};
use abi_stable::std_types::{RString, RVec};
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::separated_list1,
    sequence::{delimited, pair, preceded, tuple},
    Finish,
};
use std::path::PathBuf;
use std::str::FromStr;

pub fn prop_order<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, PropOrder> {
    let (rest, var) = delimited(
        angle_start,
        maybe_newline(cut(err_ctx(&ParseErrorType::Incomplete, variable))),
        maybe_newline(cut(err_ctx(&ParseErrorType::Unclosed(">"), angle_end))),
    )(inp)?;
    let prop = match var.content {
        "sequential" | "seq" => PropOrder::Sequential,
        "inverse" | "inv" => PropOrder::Inverse,
        "inputsfirst" | "inp" => PropOrder::InputsFirst,
        "outputfirst" | "out" => PropOrder::OutputFirst,
        _ => {
            return Err(nom::Err::Failure(
                MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
            ));
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

pub fn prop_nodes<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, PropNodes> {
    delimited(
        bracket_start,
        cut(alt((
            map(maybe_newline(str_path), PropNodes::Path),
            map(maybe_newline(node_list), PropNodes::List),
        ))),
        maybe_newline(cut(err_ctx(&ParseErrorType::Unclosed("]"), bracket_end))),
    )(inp)
}

pub fn propagation<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Option<Propagation>> {
    let (rest, (order, nodes, cond)) = tuple((
        opt(prop_order),
        opt(prop_nodes),
        opt(map(
            delimited(
                paren_start,
                maybe_newline(complete_expression),
                maybe_newline(paren_end),
            ),
            PropCondition::Expr,
        )),
    ))(inp)?;
    if order.is_none() && nodes.is_none() && cond.is_none() {
        Ok((rest, None))
    } else {
        Ok((
            rest,
            Some(Propagation {
                order: order.unwrap_or_default(),
                nodes: nodes.unwrap_or_default(),
                condition: cond.unwrap_or_default(),
                start: inp.position(),
            }),
        ))
    }
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

pub fn attr_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, AttrTask> {
    let (rest, (ty, propagation, mut attr_pre)) =
        tuple((function_type, propagation, preceded(opt(dot), dot_variable)))(inp)?;

    match (&ty, propagation.is_some()) {
        (FunctionType::Node, true) => (),
        (_, true) => {
            return Err(nom::Err::Error(
                MatchErr::new(function_type(inp)?.0).ty(&ParseErrorType::PropagationNotSupported),
            ));
        }
        _ => (),
    }
    let attr = attr_pre.pop().expect("should have at least one component");
    Ok((
        rest,
        AttrTask {
            ty,
            attr_pre,
            attr,
            propagation,
            start: inp.position(),
        },
    ))
}

// todo add support for user types as well (structs)
pub fn attr_type<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, NadiAttrType> {
    let (rest, var) = variable(inp)?;
    match NadiAttrType::from_str(var.content) {
        Ok(v) => Ok((rest, v)),
        Err(_) => Err(nom::Err::Failure(
            MatchErr::new(inp).ty(&ParseErrorType::InvalidType),
        )),
    }
}

pub fn typed_var<'a, 'b>(
    inp: &'a [Token<'b>],
) -> MatchRes<'a, 'b, (Vec<String>, Option<NadiAttrType>)> {
    pair(
        dot_variable,
        opt(preceded(maybe_space(colon), maybe_space(attr_type))),
    )(inp)
}

pub fn eval_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, EvalTask> {
    let (rest, (ty, propagation, attr, input, sc)) = tuple((
        function_type,
        propagation,
        opt(delimited(opt(dot), typed_var, maybe_space(assignment))),
        maybe_newline(complete_expression),
        opt(semicolon),
    ))(inp)?;
    match (&ty, propagation.is_some()) {
        (FunctionType::Node, true) => (),
        (_, true) => {
            return Err(nom::Err::Error(
                MatchErr::new(function_type(inp)?.0).ty(&ParseErrorType::PropagationNotSupported),
            ));
        }
        _ => (),
    }
    let (attr_pre, attr) = match attr {
        None => (vec![], None),
        Some((mut v, aty)) => {
            let name = v.pop().map(|v| (v, aty));
            (v, name)
        }
    };
    Ok((
        rest,
        EvalTask {
            ty,
            attr_pre,
            attr,
            propagation,
            input,
            silent: sc.is_some(),
            start: inp.position(),
        },
    ))
}

pub fn help_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    map(
        tuple((
            kw_help,
            opt(after_space(keyword_val)),
            opt(after_space(alt((
                map(variable, |v| v.content.to_string()),
                string_val,
            )))),
        )),
        |(_, kw, st)| Task::Help(kw, st),
    )(inp)
}

pub fn cond_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, CondTask> {
    let (rest, (cond, iftrue, iffalse)) = tuple((
        preceded(kw_if, maybe_space(expression_group)),
        maybe_newline(tasks_block),
        opt(maybe_newline(preceded(kw_else, maybe_newline(tasks_block)))),
    ))(inp)?;
    Ok((
        rest,
        CondTask {
            cond,
            iftrue,
            iffalse: iffalse.unwrap_or_default(),
            start: inp.position(),
        },
    ))
}

pub fn hook_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    let (rest, tasks) = preceded(kw_hook, maybe_space(tasks_block))(inp)?;
    Ok((rest, tasks))
}

pub fn while_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, WhileTask> {
    let (rest, (cond, tasks)) = tuple((
        preceded(kw_while, maybe_space(expression_group)),
        maybe_newline(tasks_block),
    ))(inp)?;
    Ok((
        rest,
        WhileTask {
            cond,
            tasks,
            start: inp.position(),
        },
    ))
}

pub fn import_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ImportTask> {
    let (rest, (tasks, name, path)) = tuple((
        alt((value(true, kw_exec), value(false, kw_import))),
        after_space(variable),
        opt(preceded(
            after_space(kw_from),
            after_space(map(string_val, PathBuf::from)),
        )),
    ))(inp)?;
    Ok((
        rest,
        ImportTask {
            name: name.content.to_string(),
            path,
            tasks,
        },
    ))
}

pub fn get_series_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, GetSeriesTask> {
    let (rest, (ty, propagation, (_, ts, name))) = tuple((
        function_type,
        propagation,
        tuple((dollar, opt(dollar), variable)),
    ))(inp)?;

    match (&ty, propagation.is_some()) {
        (FunctionType::Node, true) => (),
        (_, true) => {
            return Err(nom::Err::Error(
                MatchErr::new(function_type(inp)?.0).ty(&ParseErrorType::PropagationNotSupported),
            ));
        }
        _ => (),
    }
    Ok((
        rest,
        GetSeriesTask {
            ty,
            timeseries: ts.is_some(),
            name: name.content.to_string(),
            propagation,
            start: inp.position(),
        },
    ))
}

pub fn series_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SeriesExpression> {
    alt((
        map(
            tuple((
                opt(variable_type),
                dollar,
                opt(dollar),
                variable,
                // optionally the function mapping
                opt(after_space(preceded(
                    path_sep,
                    cut(after_space(alt((
                        map(function_def, MapFunction::Defn),
                        map(preceded(at, variable), |v| {
                            MapFunction::Pointer(v.content.to_string())
                        }),
                    )))),
                ))),
            )),
            |(vt, _, ts, name, func)| match func {
                Some(func) => {
                    SeriesExpression::SeriesMap(vt, ts.is_some(), name.content.to_string(), func)
                }
                None => SeriesExpression::Series(vt, ts.is_some(), name.content.to_string()),
            },
        ),
        map(complete_expression, SeriesExpression::AttrExpr),
    ))(inp)
}
pub fn set_series_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SetSeriesTask> {
    let (rest, (ty, propagation, (_, ts, name), _, expression)) = tuple((
        function_type,
        propagation,
        tuple((dollar, opt(dollar), variable)),
        maybe_space(assignment),
        maybe_space(series_expression),
    ))(inp)?;

    match (&ty, propagation.is_some()) {
        (FunctionType::Node, true) => (),
        (_, true) => {
            return Err(nom::Err::Error(
                MatchErr::new(function_type(inp)?.0).ty(&ParseErrorType::PropagationNotSupported),
            ));
        }
        _ => (),
    }
    Ok((
        rest,
        SetSeriesTask {
            ty,
            timeseries: ts.is_some(),
            name: name.content.to_string(),
            propagation,
            expression,
            start: inp.position(),
        },
    ))
}

pub fn task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    alt((
        // set should be before get
        map(set_series_task, Task::SetSeries),
        map(get_series_task, Task::GetSeries),
        // eval task should come after series, otherwise series gets
        // interpreted as attribute expression
        map(eval_task, Task::Eval),
        map(attr_task, Task::Attr),
        map(cond_task, Task::Conditional),
        map(while_task, Task::WhileLoop),
        map(hook_task, Task::Hook),
        map(function_def, Task::Function),
        map(import_task, Task::Import),
        map(complete_expression, Task::Expr),
        help_task,
        value(Task::Clear, kw_clear),
        value(Task::Exit, kw_exit),
    ))(inp)
}

pub fn tasks<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    trailing_newlines(newline_separated(task))(inp)
}

pub fn tasks_block<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    delimited(brace_start, maybe_newline(tasks), maybe_newline(brace_end))(inp)
}

pub fn parse(tokens: Vec<RawToken>) -> Result<Vec<Task>, ParseError> {
    let tokens = Token::validate(tokens)?;
    match tasks(&tokens).finish() {
        Ok((rest, tasks)) => {
            if rest.is_empty() {
                Ok(tasks)
            } else {
                match trailing_newlines(maybe_newline(task))(rest).finish() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokenizer::get_tokens;
    use rstest::rstest;

    #[rstest]
    #[case("exit")]
    #[case("help")]
    #[case("help node")]
    #[case("help variable")]
    #[case("help network var")]
    #[case("env x")]
    #[case("env x + 1")]
    #[case("env call_sth(x + 1);")]
    #[case("env.x")]
    #[case("node.x")]
    #[case("network.x")]
    #[case("inputs.x")]
    #[case("env (x + 1) != 5")]
    #[case("env \"val\" in selected_vals")]
    #[case("env echo(x)")]
    #[case("network load_file(test)")]
    #[case("network gis.load_file(12)")]
    #[case("node call_sth(x + 1);")]
    #[case("node some_func()")]
    #[case("node<inverse> some_func()")]
    #[case("node<outputfirst>[a] some_func()")]
    #[case("node<inputsfirst>[a](cond) some_func()")]
    #[case("node[a](cond) some_func()")]
    #[case("node(cond) (some_func() + 12) > 12")]
    #[case("while (true) {\n\tenv echo(x)\n}")]
    #[case("if (true) {\n\tenv echo(x)\n} else {\n\tenv echo(y)\n}")]
    #[case("while (true) {\n\tenv echo(x)\n}")]
    pub fn task_valid_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tasks) = task(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let tsk = tasks.to_string();
        assert_eq!(txt, tsk);
    }

    #[rstest]
    #[case("\n # test\nexit")]
    #[case("help")]
    #[case("help node")]
    #[case("help variable")]
    #[case("help network var")]
    #[case("env x")]
    pub fn parse_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        parse(tokens).unwrap();
    }

    /// Testing the codes in mdbook
    #[rstest]
    #[case(
        "network load_file(\"./data/mississippi.net\")\nnode[ohio] render(\"{_NAME:case(title)} River\")"
    )]
    pub fn parse_valid_mdbook_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        parse(tokens).unwrap();
    }
}
