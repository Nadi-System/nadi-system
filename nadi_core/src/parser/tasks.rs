use crate::parser::{
    components::*,
    errors::MatchErr,
    expressions::complete_expression,
    network::{node_name, str_path},
    tokenizer::{check_tokens, TaskToken, Token},
    ParseError, ParseErrorType,
};
use crate::{
    functions::Propagation,
    network::StrPath,
    prelude::*,
    tasks::{AttrTask, EvalTask, FunctionType, Task},
};
use abi_stable::std_types::{RString, RVec};
use nom::{
    branch::alt,
    combinator::{all_consuming, map, opt, value},
    multi::separated_list1,
    sequence::{delimited, preceded, separated_pair, tuple},
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

pub fn attr_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, AttrTask> {
    map(
        tuple((
            function_type,
            opt(maybe_space(propagation)),
            preceded(opt(dot), dot_variable),
        )),
        |(ty, propagation, attribute)| AttrTask {
            ty,
            attribute,
            propagation,
        },
    )(inp)
}

pub fn eval_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, EvalTask> {
    map(
        tuple((
            function_type,
            opt(maybe_space(propagation)),
            opt(delimited(opt(dot), dot_variable, maybe_space(assignment))),
            maybe_newline(complete_expression),
            opt(semicolon),
        )),
        |(ty, propagation, attr, input, sc)| EvalTask {
            ty,
            attribute: attr.unwrap_or_default(),
            propagation,
            input,
            silent: sc.is_some(),
        },
    )(inp)
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

pub fn task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    alt((
        map(eval_task, Task::Eval),
        map(attr_task, Task::Attr),
        help_task,
        value(Task::Exit, kw_exit),
    ))(inp)
}

pub fn tasks<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    trailing_newlines(newline_separated(task))(inp)
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
                    .err()
                    .expect("Rest should be empty if network parse is complete");
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
}
