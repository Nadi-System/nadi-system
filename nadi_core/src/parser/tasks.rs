use crate::parser::{
    components::*,
    errors::MatchErr,
    expressions::{complete_expression, expression_group},
    network::{node_name, str_path},
    tokenizer::{check_tokens, Token},
    ParseError, ParseErrorType,
};
use crate::{
    network::{PropCondition, PropNodes, PropOrder, Propagation},
    tasks::{AttrTask, CondTask, EvalTask, FunctionType, Task, WhileTask},
};
use abi_stable::std_types::{RString, RVec};
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::separated_list1,
    sequence::{delimited, preceded, tuple},
    Finish,
};

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
            ))
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
        },
    ))
}

pub fn eval_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, EvalTask> {
    let (rest, (ty, propagation, attr, input, sc)) = tuple((
        function_type,
        propagation,
        opt(delimited(opt(dot), dot_variable, maybe_space(assignment))),
        maybe_newline(complete_expression),
        opt(semicolon),
    ))(inp)?;
    match (&ty, propagation.is_some()) {
        (FunctionType::Node, true) => (),
        (_, true) => {
            return Err(nom::Err::Error(
                MatchErr::new(function_type(inp)?.0).ty(&ParseErrorType::PropagationNotSupported),
            ))
        }
        _ => (),
    }
    let (attr_pre, attr) = match attr {
        None => (vec![], None),
        Some(mut v) => {
            let name = v.pop();
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
        },
    ))
}

pub fn while_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, WhileTask> {
    let (rest, (cond, tasks)) = tuple((
        preceded(kw_while, maybe_space(expression_group)),
        maybe_newline(tasks_block),
    ))(inp)?;
    Ok((rest, WhileTask { cond, tasks }))
}

pub fn task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    alt((
        map(cond_task, Task::Conditional),
        map(while_task, Task::WhileLoop),
        map(eval_task, Task::Eval),
        map(attr_task, Task::Attr),
        help_task,
        value(Task::Exit, kw_exit),
    ))(inp)
}

pub fn tasks<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    trailing_newlines(newline_separated(task))(inp)
}

pub fn tasks_block<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    delimited(brace_start, maybe_newline(tasks), maybe_newline(brace_end))(inp)
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<Task>, ParseError> {
    check_tokens(&tokens)?;
    match tasks(&tokens).finish() {
        Ok((rest, tasks)) => {
            if rest.is_empty() {
                Ok(tasks)
            } else {
                let err = maybe_newline(task)(rest) // need this to fail
                    .finish()
                    .expect_err("Rest should be empty if tasks parse is complete");
                Err(ParseError::new(&tokens, err.internal.input, err.ty))
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
    pub fn task_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        let (rest, _) = task(&tokens).unwrap();
        assert_eq!(rest, vec![]);
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
    #[case("network load_file(\"./data/mississippi.net\")\nnode[ohio] render(\"{_NAME:case(title)} River\")")]
    pub fn parse_valid_mdbook_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        parse(tokens).unwrap();
    }
}
