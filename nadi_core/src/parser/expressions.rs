use crate::attrs::{AttrMap, Attribute, HasAttributes};
use crate::expressions::{BiOperator, Expression, FunctionCall, InputVar, UniOperator, VarType};
use crate::parser::{
    components::*,
    errors::{MatchErr, ParseError, ParseErrorType},
    tokenizer::{check_tokens, TaskToken, Token},
};
use crate::tasks::TaskKeyword;
use abi_stable::std_types::{map::REntry, RString};
use nadi_core::network::StrPath;
use nom::{
    branch::alt,
    combinator::{all_consuming, cut, fail, map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, separated_pair, terminated, tuple},
    Finish,
};

pub fn expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    alt((
        uni_operator_expr,
        map(input_variable, Expression::Variable),
        map(attribute, Expression::Literal),
        map(function_call, Expression::Function),
    ))(inp)
}

pub fn expression_group<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    delimited(
        paren_start,
        maybe_newline(complete_expression),
        cut(err_ctx(
            &ParseErrorType::Unclosed(")"),
            maybe_newline(paren_end),
        )),
    )(inp)
}

pub fn uni_operator_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (op, expr)) = pair(
        alt((
            value(UniOperator::Not, not),
            value(UniOperator::Negative, dash),
        )),
        maybe_newline(alt((expression_group, expression))),
    )(inp)?;
    Ok((rest, Expression::UniOp(op, Box::new(expr))))
}

pub fn bi_operator<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, BiOperator> {
    alt((
        value(BiOperator::Add, plus),
        value(BiOperator::Substract, dash),
        value(BiOperator::Multiply, star),
        value(BiOperator::Divide, slash),
        value(BiOperator::Modulus, percentage),
        value(BiOperator::Equal, pair(assignment, assignment)),
        value(BiOperator::LessThanEqual, pair(angle_start, assignment)),
        value(BiOperator::GreaterThanEqual, pair(angle_end, assignment)),
        value(BiOperator::LessThan, angle_start),
        value(BiOperator::GreaterThan, angle_end),
        value(BiOperator::And, and),
        value(BiOperator::Or, or),
    ))(inp)
}

pub fn complete_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (first, others)) = pair(
        alt((expression_group, expression)),
        many0(pair(
            maybe_newline(bi_operator),
            cut(err_ctx(
                &ParseErrorType::IncompleteExpression,
                maybe_newline(alt((expression_group, expression))),
            )),
        )),
    )(inp)?;
    // TODO left-first expression evaluation; redo later
    let mut lhs = first;
    match others.as_slice() {
        [] => Ok((rest, lhs)),
        [others @ .., last] => {
            // TODO a way to do pattern match without cloning would be nice
            for (o, v) in others {
                lhs = Expression::BiOp(o.clone(), Box::new(lhs), Box::new(v.clone()));
            }
            Ok((
                rest,
                Expression::BiOp(last.0.clone(), Box::new(lhs), Box::new(last.1.clone())),
            ))
        }
    }
}

pub fn variable_type<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, VarType> {
    let (rest, kw) = keyword_val(inp)?;
    match VarType::from_keyword(&kw) {
        Some(v) => Ok((rest, v)),
        None => Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::InvalidKeyword),
        )),
    }
}

pub fn input_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, InputVar> {
    alt((
        map(
            pair(
                separated_pair(
                    variable_type,
                    // example showing how to make it return error on
                    // partial matches
                    cut(err_ctx(&ParseErrorType::Incomplete, dot)),
                    cut(err_ctx(&ParseErrorType::Incomplete, dot_variable)),
                ),
                opt(question),
            ),
            |((vt, mut v), q)| {
                let name = v.pop().expect("There should be at least one var");
                InputVar::new(Some(vt), v, name, q.is_some())
            },
        ),
        map(pair(dot_variable, opt(question)), |(mut v, q)| {
            let name = v.pop().expect("There should be at least one var");
            InputVar::new(None, v, name, q.is_some())
        }),
    ))(inp)
}

pub fn kw_arg<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (String, Expression)> {
    separated_pair(
        // no dot variable in kwargs pair
        map(variable, |t| t.content.to_string()),
        maybe_space(assignment),
        cut(err_ctx(
            &ParseErrorType::MissingValue,
            maybe_space(complete_expression),
        )),
    )(inp)
}

pub fn kw_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<(String, Expression)>> {
    separated_list1(comma, maybe_newline(kw_arg))(inp)
}

pub fn pos_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Expression>> {
    separated_list1(comma, maybe_newline(complete_expression))(inp)
}

pub fn function_call<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FunctionCall> {
    let (rest, (name, (args, kwargs))) = tuple((
        function,
        cut(err_ctx(
            &ParseErrorType::InvalidFunctionParameters,
            alt((
                value(
                    (vec![], vec![]),
                    pair(paren_start, maybe_newline(paren_end)),
                ),
                delimited(
                    paren_start,
                    map(pos_args, |a| (a, vec![])),
                    maybe_newline(paren_end),
                ),
                delimited(
                    paren_start,
                    map(kw_args, |a| (vec![], a)),
                    maybe_newline(paren_end),
                ),
                delimited(
                    paren_start,
                    pair(
                        many1(terminated(
                            maybe_newline(complete_expression),
                            maybe_newline(comma),
                        )),
                        maybe_newline(kw_args),
                    ),
                    maybe_newline(paren_end),
                ),
            )),
        )),
    ))(inp)?;
    Ok((
        rest,
        FunctionCall::new(name.content.to_string(), args, kwargs.into_iter().collect()),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expressions::EvalError;
    use crate::parser::tokenizer::get_tokens;
    use crate::tasks::FunctionType;
    use crate::tasks::TaskContext;
    use rstest::{fixture, rstest};

    #[fixture]
    fn context() -> TaskContext {
        // Since TaskContext is not thread safe, we cannot share references between tests
        let mut ctx = TaskContext::new(None);
        ctx.env.set_attr("xyz", 12.into());
        ctx
    }

    #[rstest]
    #[case("12")]
    #[case("2.12")]
    #[case("- 2.12")]
    #[case("xyz")]
    #[case("!(xyz)")]
    #[case("!(-xyz)")]
    pub fn expression_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        let (rest, _) = expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
    }

    #[rstest]
    #[case("12")]
    #[case("2.12")]
    #[case("- 2.12")]
    #[case("xyz")]
    #[case("!(xyz)")]
    #[case("!(-xyz)")]
    #[case("xyz + 12")]
    // since it doesn't eval, anything is valid
    #[case("xyz | yzx * 12 + true % func(call)")]
    #[case("(xyz | yzx) * (12 + true)")]
    #[should_panic]
    #[case("(xyz |* yzx) * (12 + true)")]
    pub fn compl_expr_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        let (rest, _) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
    }

    #[rstest]
    #[case("sth()")]
    #[case("sth.sth()")]
    #[case("sth.sth(12)")]
    #[case("sth.sth(-zyx2)")]
    #[case("sth.sth(y=12)")]
    #[case("sth.sth(2.12, y=12, y2=43)")]
    #[case("sth.sth(2.12, y=12, y2=43 + values * 1.23)")]
    #[should_panic]
    #[case("sth.sth(2.12, y=12, 43)")]
    pub fn function_call_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        let (rest, _) = function_call(&tokens).unwrap();
        assert_eq!(rest, vec![]);
    }

    // testing the evaluation is easier than testing if it got all the
    // components correct, xyz=12 from fixure above
    #[rstest]
    #[case("12", 12.into())]
    #[case("2.12", 2.12.into())]
    #[case("- 2.12", (-2.12).into())]
    #[case("xyz", 12.into())]
    #[should_panic]
    #[case("!(xyz)", 12.into())]
    #[should_panic]
    #[case("(xyz2)", 12.into())]
    #[case("(-xyz)", (-12).into())]
    #[case("xyz + 12", 24.into())]
    #[case("2*(xyz - 10) + 12", 16.into())]
    #[case("(xyz >= 10) | false", true.into())]
    #[should_panic]
    #[case("(xyz - 1) * (12 + true)", 143.into())]
    pub fn compl_expr_eval_test(
        mut context: TaskContext,
        #[case] txt: &str,
        #[case] val: Attribute,
    ) {
        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr
            .resolve(&FunctionType::Env, &context, None)
            .unwrap()
            .eval(&FunctionType::Env, &mut context, None)
            .unwrap()
            .unwrap();
        assert_eq!(res, val);
    }

    // testing the simplify process
    #[rstest]
    #[case("12 + 2", "14")]
    #[case("true | false", "true")]
    #[case("12 > 12", "false")]
    #[case("(xyz >= 10) | false", "(xyz >= 10) | false")]
    #[should_panic]
    #[case("(xyz - 1) * (12 + true)", "(xyz - 1) * 13")]
    pub fn compl_expr_simplify_test(context: TaskContext, #[case] txt: &str, #[case] simpl: &str) {
        // let context = task_context();

        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr.simplify(&FunctionType::Env, &context).unwrap();

        let tokens2 = get_tokens(simpl);
        let (rest2, expr2) = complete_expression(&tokens2).unwrap();
        assert_eq!(rest2, vec![]);

        assert_eq!(res, expr2);
    }

    // testing the simplify process
    #[rstest]
    #[case("- true", EvalError::NotANumber)]
    #[case("true | 12", EvalError::NotABool)]
    #[case("(xyz - 1) * (12 + true)", EvalError::InvalidOperation)]
    #[case("(xyz - 1) * (true + true)", EvalError::InvalidOperation)]
    #[case("(xyz * \"1\") * (12 + true)", EvalError::InvalidOperation)]
    pub fn compl_expr_error_test(context: TaskContext, #[case] txt: &str, #[case] err: EvalError) {
        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr.simplify(&FunctionType::Env, &context).err().unwrap();
        assert_eq!(res, err);
    }
}
