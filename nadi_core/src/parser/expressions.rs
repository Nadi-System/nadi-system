use crate::expressions::{BiOperator, Expression, FunctionCall, InputVar, UniOperator, VarType};
use crate::parser::{
    components::*,
    errors::{MatchErr, ParseErrorType},
    tokenizer::Token,
};
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::{many1, separated_list1},
    sequence::{delimited, pair, separated_pair, terminated, tuple},
};

pub fn expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    alt((
        map(input_variable, Expression::Variable),
        map(attribute, Expression::Literal),
        map(function_call, Expression::Function),
        uni_operator_expr,
        if_else_expr,
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

pub fn expression_block<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    delimited(
        brace_start,
        maybe_newline(complete_expression),
        cut(err_ctx(
            &ParseErrorType::Unclosed("}"),
            maybe_newline(brace_end),
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

macro_rules! bi_op_pred {
    ($name:ident, $inner:expr, $($itself:expr)?, $($kw: expr => $op: expr),*) => {
	fn $name<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
	    map(
		tuple((
		    alt(($inner, expression_group)),
		    maybe_newline(alt((
			$(value($op, $kw),)*
		    ))),
		    maybe_newline(cut(err_ctx(
			&ParseErrorType::IncompleteExpression,
			alt((expression_group, $($itself,)? $inner))
		    )))
		)),
		|(lhs, op, rhs)| Expression::BiOp(op, Box::new(lhs), Box::new(rhs)),
	    )(inp)
	}
    }
}

bi_op_pred!(bi_op_in_match, expression,,
            kw_in => BiOperator::In,
            kw_match => BiOperator::Match
);
bi_op_pred!(bi_op_mult, expression, bi_op_mult,
            star => BiOperator::Multiply,
            pair(slash, slash) => BiOperator::IntDivide,
            slash => BiOperator::Divide,
            percentage => BiOperator::Modulus
);
bi_op_pred!(bi_op_plusminus, alt((bi_op_mult, expression)), bi_op_plusminus,
            plus => BiOperator::Add,
            dash => BiOperator::Substract
);
bi_op_pred!(bi_op_and, alt((bi_op_in_match, bi_op_compare, expression)), bi_op_and,
            and => BiOperator::And
);
bi_op_pred!(bi_op_or, alt((bi_op_and, bi_op_in_match, bi_op_compare, expression)), bi_op_or,
            or => BiOperator::Or
);
bi_op_pred!(bi_op_compare, expression,,
            pair(assignment, assignment) => BiOperator::Equal,
            pair(not, assignment) => BiOperator::NotEqual,
            pair(angle_start, assignment) => BiOperator::LessThanEqual,
            pair(angle_end, assignment) => BiOperator::GreaterThanEqual,
            angle_start => BiOperator::LessThan,
            angle_end => BiOperator::GreaterThan
);

pub fn if_else_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (_, cond, iftrue, _, iffalse)) = tuple((
        kw_if,
        maybe_newline(expression_group),
        maybe_newline(expression_block),
        maybe_newline(kw_else),
        maybe_newline(expression_block),
    ))(inp)?;
    Ok((
        rest,
        Expression::IfElse(Box::new(cond), Box::new(iftrue), Box::new(iffalse)),
    ))
}

pub fn complete_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    alt((
        bi_op_or,
        bi_op_and,
        bi_op_in_match,
        bi_op_compare,
        bi_op_plusminus,
        bi_op_mult,
        expression_group,
        expression,
    ))(inp)
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
    map(
        tuple((
            opt(terminated(variable_type, dot)),
            dot_variable,
            opt(question),
        )),
        |(vt, mut v, q)| {
            let name = v.pop().expect("There should be at least one var");
            InputVar::new(vt, v, name, q.is_some())
        },
    )(inp)
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
    let (rest, (ty, name, (args, kwargs))) = tuple((
        opt(terminated(variable_type, dot)),
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
        FunctionCall::new(
            ty,
            None,
            name.content.to_string(),
            args,
            kwargs.into_iter().collect(),
        ),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{Attribute, HasAttributes};
    use crate::expressions::EvalError;
    use crate::functions::NadiFunctions;
    use crate::parser::tokenizer::get_tokens;
    use crate::tasks::FunctionType;
    use crate::tasks::TaskContext;
    use rstest::{fixture, rstest};
    use std::sync::OnceLock;

    static mut NADI_FUNCS: OnceLock<NadiFunctions> = OnceLock::new();

    #[fixture]
    fn context() -> TaskContext {
        // The static mut ref is for OnceLock, and it is immediately
        // cloned to be used, so it is safe. This just saves us from
        // loading the plugins over and over again for each test,
        // significantly improving the runtime speed.
        #[allow(static_mut_refs)]
        let functions = unsafe { NADI_FUNCS.get_or_init(NadiFunctions::new) }.clone();

        let mut ctx = TaskContext {
            functions,
            ..Default::default()
        };
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
    #[should_panic]
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
    #[case("inf - 2.12", (f64::INFINITY).into())]
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
    // testing
    #[case("1 + 2 * 2", 5.into())]
    #[case("1 + 2 * (2 - 5)", (-5).into())]
    #[case("(2 - 5) * 2 + 1", (-5).into())]
    #[case("(2 - 1)  + 1", 2.into())]
    #[case("10 // 5 + 2", 4.into())]
    pub fn compl_expr_eval_test(context: TaskContext, #[case] txt: &str, #[case] val: Attribute) {
        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr
            .resolve(&FunctionType::Env, &context, None)
            .unwrap()
            .eval(&FunctionType::Env, &context, None)
            .unwrap()
            .unwrap();
        assert_eq!(res, val);
    }

    #[rstest]
    #[case("1 + 2 * 2", (5).into())]
    #[case("1 + 2 * (2 - 5)", (-5).into())]
    #[case("(2 - 5) * 2 + 1", (-5).into())]
    #[case("(2 - 1) + 1", (2).into())]
    #[case("10 // 5 + 2", (4).into())]
    #[case("10 % 5 + 2", (2).into())]
    #[case("1 + 2 * (3 - 2)", (3).into())]
    #[case("(3 + 2) * 2 - 1", (9).into())]
    #[case("((10 - 2) / (2 + 1)) * 3", (8.0).into())]
    #[case("2 * (4 - 2) + 3", (7).into())]
    #[case("(8 - 6) / 2 + 1", (2.0).into())]
    #[case("(9 - 5) * (3 + 1)", (16).into())]
    #[case("(7 + 3) * (2 - 1)", (10).into())]
    #[case("10 // (2 * 2) + 1", (3).into())]
    #[case("(4 + 2) / (3 - 1) + 2", (5.0).into())]
    #[case("(8 - 5) * (2 + 1)", (9).into())]
    #[case("(6 - 3) / (3 + 1) + 1", (1.75).into())]
    #[case("12 // (4 * 2) + 2", (3).into())]
    #[case("(10 + 2) * (5 - 3)", (24).into())]
    #[case("(9 + 1) / (4 - 2) + 2", (7.0).into())]
    #[case("(8 + 6) // (2 * 2) + 1", (4).into())]
    #[case("(7 - 3) * (5 + 1)", (24).into())]
    #[case("3 * ((6 - 2) / (4 - 1) + 2)", (10.0).into())]
    #[case("18 // (9 * 2) + 3", (4).into())]
    #[case("(11 + 5) * (8 - 4)", (64).into())]
    #[case("(10 + 3) / (7 - 2) + 3", (5.6).into())]
    #[case("(10 + 3) % (7 - 2) + 3", (6).into())]
    #[case("(9 + 1) // (5 * 2) + 2", (3).into())]
    #[case("(8 + 2) * (7 - 1)", (60).into())]
    #[case("(7 - 1) / (6 - 2) + 3", (4.5).into())]
    #[case("20 // (10 * 2) + 5", (6).into())]
    #[case("(13 + 9) * (11 - 5)", (132).into())]
    #[case("(12 + 4) / (9 - 1) + 6", (8.0).into())]
    #[case("(11 - 3) * (10 + 2)", (96).into())]
    #[case("(10 - 2) // (8 - 4) + 5", (7).into())]
    #[case("(9 + 3) / (7 - 1) + 6", (8.0).into())]
    #[case("(8 + 1) * (7 - 2)", (45).into())]
    #[case("(7 + 1) / (6 - 2) + 5", (7.0).into())]
    #[case("25 // (13 * 2) + 7", (7).into())]
    #[case("(16 + 9) * (15 - 7)", (200).into())]
    #[case("(15 + 3) / (12 - 2) + 8", (9.8).into())]
    #[case("(14 - 4) * (13 + 1)", (140).into())]
    #[case("(13 - 3) // (11 - 1) + 9", (10).into())]
    #[case("(12 + 2) * (11 - 2)", (126).into())]
    #[case("(11 - 1) / (10 - 2) + 10", (11.25).into())]
    #[case("30 // (15 * 2) + 12", (13).into())]
    #[case("(19 + 13) * (18 - 9)", (288).into())]
    #[case("true & true", true.into())]
    #[case("true & (2 < 0.9)", false.into())]
    #[case("10 <=9 & true", false.into())]
    #[case("false & false", false.into())]
    #[case("true | true", true.into())]
    #[case("true | false", true.into())]
    // this here can be invalid if xyz is bool, but valid when it's
    // number, so > is evaluated before the boolean operation |
    #[case("false | xyz > -12", true.into())]
    #[case("false | false", false.into())]
    #[case("! true", false.into())]
    #[case("! false", true.into())]
    #[case("!(true & true)", false.into())]
    #[case("!(true & false)", true.into())]
    #[case("!!(false & true)", false.into())]
    pub fn compl_expr_eval_test_2(context: TaskContext, #[case] txt: &str, #[case] val: Attribute) {
        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr
            .resolve(&FunctionType::Env, &context, None)
            .unwrap()
            .eval(&FunctionType::Env, &context, None)
            .unwrap()
            .unwrap();
        assert_eq!(res, val);
    }
    // testing the simplify process
    #[rstest]
    #[case("12 + 2", "14")]
    #[case("true | false", "true")]
    #[case("false | false | true & false", "false")]
    #[case("5 + 12 - 2 + 0 * 100", "15")]
    #[case("false | (false & true)", "false")]
    // even though this is invalid, this short circuits after the
    // first true, so it doesn't fail
    #[case("true | 12", "true")]
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
    #[case("12 | true", EvalError::NotABool)]
    #[case("(xyz - 1) * (12 + true)", EvalError::InvalidOperation)]
    #[case("(xyz - 1) * (true + true)", EvalError::InvalidOperation)]
    #[case("(xyz * \"1\") * (12 + true)", EvalError::InvalidOperation)]
    pub fn compl_expr_error_test(context: TaskContext, #[case] txt: &str, #[case] err: EvalError) {
        let tokens = get_tokens(txt);
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr.simplify(&FunctionType::Env, &context);
        assert_eq!(res, Err(err));
    }
}
