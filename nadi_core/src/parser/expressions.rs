use crate::expressions::{
    BiOperator, ExprWithContext, Expression, FunctionCall, InputVar, TaskPosition, UniOperator,
    VarType,
};
use crate::network::PropNodes;
use crate::parser::{
    components::*,
    errors::{MatchErr, ParseErrorType},
    tasks::propagation,
    tokenizer::Token,
};
use crate::structs::NadiStructExpr;
use crate::tasks::TaskKeyword;
use crate::udf::{LocalExpr, UserFunction};
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::{many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
};

pub fn expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    alt((
        map(nadi_struct_expr, Expression::StructExpr),
        expr_with_context,
        expr_set_variable,
        expr_maybe_range,
        map(function_call, Expression::Function),
        map(template_val, Expression::Render),
        array_expr,
        table_expr,
        uni_operator_expr,
        if_else_expr,
        try_catch_expr,
        for_each_if_expr,
        map(
            preceded(kw_error, after_space(string_val)),
            Expression::UserError,
        ),
        map(
            preceded(kw_return, opt(after_space(complete_expression))),
            |e| Expression::Return(e.map(Box::new)),
        ),
        series,
    ))(inp)
}

pub fn expr_with_context<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (vt, expr)) = pair(variable_type, maybe_space(expression_block))(inp)?;
    Ok((
        rest,
        Expression::WithContext(ExprWithContext::new(vt, expr)),
    ))
}

pub fn expr_set_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    map(
        tuple((
            opt(terminated(variable_type, dot)),
            task_dot_variable,
            maybe_space(assignment),
            maybe_space(complete_expression),
        )),
        |(vt, (var, indices), _, expr)| {
            Expression::SetVariable(
                InputVar::new(
                    vt.clone(),
                    var.clone(),
                    indices.clone(),
                    false,
                    inp.position(),
                ),
                Box::new(expr),
            )
        },
    )(inp)
}

pub fn expr_maybe_range<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (start, step, end)) = tuple((
        alt((input_variable, map(primitives, Expression::Literal))),
        opt(preceded(
            colon,
            maybe_space(alt((input_variable, map(primitives, Expression::Literal)))),
        )),
        opt(preceded(
            colon,
            maybe_space(alt((input_variable, map(primitives, Expression::Literal)))),
        )),
    ))(inp)?;
    if let Some(step) = step {
        if let Some(end) = end {
            Ok((
                rest,
                Expression::Range(Box::new(start), Some(Box::new(step)), Box::new(end)),
            ))
        } else {
            Ok((
                rest,
                Expression::Range(Box::new(start), None, Box::new(step)),
            ))
        }
    } else {
        // just an expression
        Ok((rest, start))
    }
}

pub fn array_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, exprs) = delimited(
        bracket_start,
        separated_list0(maybe_space(comma), maybe_newline(complete_expression)),
        maybe_newline(bracket_end),
    )(inp)?;

    Ok((rest, Expression::Array(exprs)))
}

pub fn table_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, exprs) = delimited(
        brace_start,
        maybe_newline(kw_args),
        maybe_newline(brace_end),
    )(inp)?;

    Ok((rest, Expression::Table(exprs.into_iter().collect())))
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
            value(UniOperator::Positive, plus),
        )),
        maybe_newline(alt((expression_group, expression))),
    )(inp)?;
    Ok((rest, Expression::UniOp(op, Box::new(expr))))
}

/// Helper structure that contains a *flat* representation of a binary operator
/// expression.  It is produced by the lexer / parser before the tree is
/// actually built.
///
/// `first`: the left-most operand  
/// `args` : a vector of `(operator, right-hand-side)` pairs.
///
/// Example: `a + b * c - d`  ->  `first = a` and
/// `args = [(Add, b), (Multiply, c), (Substract, d)]`
struct BiOpExpr {
    first: Expression,
    args: Vec<(BiOperator, Expression)>,
}

impl BiOpExpr {
    /// Parse the given BiOpExpr into a proper Expression
    ///
    /// The precedance order
    /// - Airthmatic
    ///   - Divide/Multiply
    ///   - plus/minus
    /// - Comparision operations
    ///   - greater than, less than, equal, etc
    ///   - Match statement
    /// - Logical operations
    ///   - or
    ///   - and
    ///
    fn parse(self) -> Option<Expression> {
        // Two stacks used by the shunting-yard algorithm.
        let mut operand_stack: Vec<Expression> = Vec::new();
        let mut operator_stack: Vec<BiOperator> = Vec::new();

        operand_stack.push(self.first);
        for (op, expr) in self.args {
            // While the top of the operator stack has higher or equal
            // precedence, pop it and apply it.
            while let Some(top_op) = operator_stack.last() {
                if top_op.precedence() >= op.precedence() {
                    let top = operator_stack.pop()?;
                    let res = top.expr_from_stack(&mut operand_stack)?;
                    operand_stack.push(res);
                } else {
                    break;
                }
            }

            // Push the current operator and the following operand.
            operator_stack.push(op);
            operand_stack.push(expr);
        }
        // Drain the remaining operators.
        while let Some(op) = operator_stack.pop() {
            let res = op.expr_from_stack(&mut operand_stack)?;
            operand_stack.push(res);
        }

        operand_stack.pop()
    }
}

fn bi_operator_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (first, args)) = pair(
        alt((expression, expression_group)),
        many1(pair(
            maybe_newline(bi_operator),
            maybe_newline(cut(err_ctx(
                &ParseErrorType::IncompleteExpression,
                alt((expression, expression_group)),
            ))),
        )),
    )(inp)?;
    let expr = BiOpExpr { first, args };
    let expr = expr.parse().ok_or(nom::Err::Error(
        MatchErr::new(inp).ty(&ParseErrorType::IncompleteExpression),
    ))?;
    Ok((rest, expr))
}

pub fn bi_operator<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, BiOperator> {
    alt((
        value(BiOperator::In, kw_in),
        value(BiOperator::Match, kw_match),
        value(BiOperator::Multiply, star),
        value(BiOperator::IntDivide, pair(slash, slash)),
        value(BiOperator::Divide, slash),
        value(BiOperator::Modulus, percentage),
        value(BiOperator::Add, plus),
        value(BiOperator::Substract, dash),
        value(BiOperator::And, and),
        value(BiOperator::Or, or),
        value(BiOperator::Equal, pair(assignment, assignment)),
        value(BiOperator::NotEqual, pair(not, assignment)),
        value(BiOperator::LessThanEqual, pair(angle_start, assignment)),
        value(BiOperator::GreaterThanEqual, pair(angle_end, assignment)),
        value(BiOperator::LessThan, angle_start),
        value(BiOperator::GreaterThan, angle_end),
    ))(inp)
}

pub fn nadi_struct_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, NadiStructExpr> {
    let (rest, (name, fields)) = tuple((
        variable_name,
        delimited(
            maybe_space(brace_start),
            maybe_newline(newline_separated(tuple((
                variable_name,
                preceded(maybe_space(assignment), maybe_space(complete_expression)),
                maybe_space(comma),
            )))),
            maybe_newline(brace_end),
        ),
    ))(inp)?;
    let mut nstr = NadiStructExpr::with_name(name);
    for (fd, val, _) in fields {
        nstr.values.insert(fd.into(), val);
    }
    Ok((rest, nstr))
}

pub fn if_else_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (_, cond, iftrue, _, iffalse)) = tuple((
        kw_if,
        maybe_newline(expression_group),
        maybe_newline(expression_block),
        maybe_newline(kw_else),
        alt((after_space(if_else_expr), maybe_newline(expression_block))),
    ))(inp)?;
    Ok((
        rest,
        Expression::IfElse(Box::new(cond), Box::new(iftrue), Box::new(iffalse)),
    ))
}

pub fn try_catch_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (_, try_blk, _, catch_blk)) = tuple((
        kw_try,
        maybe_newline(expression_block),
        maybe_newline(kw_catch),
        maybe_newline(expression_block),
    ))(inp)?;
    Ok((
        rest,
        Expression::TryCatch(Box::new(try_blk), Box::new(catch_blk)),
    ))
}

pub fn for_each_if_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (var, expr1, expr2, cond)) = tuple((
        delimited(
            kw_for,
            after_space(map(variable, |v| v.content.to_string())),
            after_space(kw_in),
        ),
        after_space(expression),
        maybe_newline(expression_block),
        maybe_newline(opt(preceded(
            kw_if,
            after_space(alt((expression_block, expression_group))),
        ))),
    ))(inp)?;
    Ok((
        rest,
        Expression::ForEachIf(var, Box::new(expr1), Box::new(expr2), cond.map(Box::new)),
    ))
}

pub fn complete_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    alt((bi_operator_expr, expression_group, expression))(inp)
}

pub fn variable_type<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, VarType> {
    let (rest, (kw, prop)) = pair(keyword_val, propagation)(inp)?;
    let node = match (&kw, &prop) {
        // make sure node variable type has only one node name instead
        // of any other propagation
        (TaskKeyword::Node, Some(p)) => match &p.nodes {
            PropNodes::All => None,
            PropNodes::List(lst) => {
                if let [n] = lst.as_slice() {
                    Some(n.to_string())
                } else {
                    return Err(nom::Err::Error(
                        MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
                    ));
                }
            }
            PropNodes::Path(_) => {
                return Err(nom::Err::Error(
                    MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
                ));
            }
        },
        _ => None,
    };
    match VarType::from_keyword(&kw, prop, node) {
        Some(v) => Ok((rest, v)),
        None => Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::InvalidKeyword),
        )),
    }
}

pub fn input_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    map(
        tuple((
            opt(terminated(variable_type, dot)),
            task_dot_variable,
            opt(maybe_space(pair(
                question,
                opt(maybe_space(alt((expression, expression_group)))),
            ))),
        )),
        |(vt, (var, indices), q)| {
            if let Some((_, val)) = q {
                if let Some(val) = val {
                    let cond = Expression::Variable(InputVar::new(
                        vt.clone(),
                        var.clone(),
                        indices.clone(),
                        true,
                        inp.position(),
                    ));
                    let var = Expression::Variable(InputVar::new(
                        vt,
                        var,
                        indices,
                        false,
                        inp.position(),
                    ));
                    Expression::IfElse(Box::new(cond), Box::new(var), Box::new(val))
                } else {
                    Expression::Variable(InputVar::new(vt, var, indices, true, inp.position()))
                }
            } else {
                Expression::Variable(InputVar::new(vt, var, indices, false, inp.position()))
            }
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

pub fn pos_vars<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<String>> {
    separated_list1(
        comma,
        maybe_newline(map(variable, |v| v.content.to_string())),
    )(inp)
}

type FuncDefArgKwarg = (Vec<String>, Vec<(String, Expression)>);
type FuncCallArgKwarg = (Vec<Expression>, Vec<(String, Expression)>);

pub fn funcdef_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FuncDefArgKwarg> {
    err_ctx(
        &ParseErrorType::InvalidFunctionParameters,
        alt((
            value(
                (vec![], vec![]),
                pair(paren_start, maybe_newline(paren_end)),
            ),
            delimited(
                paren_start,
                map(pos_vars, |a| (a, vec![])),
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
                        maybe_newline(map(variable, |v| v.content.to_string())),
                        maybe_newline(comma),
                    )),
                    maybe_newline(kw_args),
                ),
                maybe_newline(paren_end),
            ),
        )),
    )(inp)
}

pub fn func_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FuncCallArgKwarg> {
    err_ctx(
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
    )(inp)
}

pub fn function_call<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FunctionCall> {
    let (rest, (ty, name, (args, kwargs))) = tuple((
        opt(terminated(variable_type, dot)),
        function,
        cut(func_args),
    ))(inp)?;
    Ok((
        rest,
        FunctionCall::new(
            ty,
            None,
            name.content.to_string(),
            args,
            kwargs.into_iter().collect(),
            inp.position(),
        ),
    ))
}

pub fn local_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, LocalExpr> {
    let (rest, (var, expr, sc)) = tuple((
        opt(terminated(variable, maybe_space(assignment))),
        maybe_space(complete_expression),
        opt(semicolon),
    ))(inp)?;
    Ok((
        rest,
        match var {
            Some(v) => LocalExpr::Assign(v.content.to_string(), expr, sc.is_some()),
            None => LocalExpr::Expr(expr, sc.is_some()),
        },
    ))
}

pub fn function_body<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<LocalExpr>> {
    delimited(
        brace_start,
        newline_separated(local_expr),
        cut(err_ctx(
            &ParseErrorType::Unclosed("}"),
            maybe_newline(brace_end),
        )),
    )(inp)
}

pub fn function_def<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, UserFunction> {
    let (rest, (_, name, (args, kwargs), exprs)) = tuple((
        kw_func,
        maybe_space(opt(function)),
        maybe_space(cut(funcdef_args)),
        maybe_newline(function_body),
    ))(inp)?;
    Ok((
        rest,
        UserFunction::new(name.map(|n| n.content.to_string()), args, kwargs, exprs),
    ))
}

pub fn series<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Expression> {
    let (rest, (vt, (ts, name), ind)) = tuple((
        opt(variable_type),
        preceded(
            dollar,
            pair(opt(dollar), map(variable, |v| v.content.to_string())),
        ),
        opt(delimited(
            bracket_start,
            maybe_space(integer_usize),
            maybe_space(bracket_end),
        )),
    ))(inp)?;
    let sr = match ind {
        Some(ind) => Expression::SeriesValue(vt, ts.is_some(), name, ind),
        None => Expression::Series(vt, ts.is_some(), name),
    };
    Ok((rest, sr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{Attribute, HasAttributes};
    use crate::expressions::EvalErrorType;
    use crate::functions::NadiFunctions;
    use crate::network::Network;
    use crate::parser::tokenizer::get_tokens;
    use crate::tasks::FunctionType;
    use crate::tasks::{TaskContext, TaskContextEnv};
    use rstest::{fixture, rstest};
    use std::collections::HashMap;
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

        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut ctx = TaskContext {
            network: Network::default(),
            functions,
            udf: HashMap::new(),
            structs: HashMap::new(),
            env: TaskContextEnv::new(),
            hook: Vec::new(),
            channel: sender,
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
        let tokens = Token::validate(get_tokens(txt)).unwrap();
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
        let tokens = Token::validate(get_tokens(txt)).unwrap();
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
        let tokens = Token::validate(get_tokens(txt)).unwrap();
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
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr
            .resolve(&FunctionType::Env, &context, None, None)
            .unwrap()
            .eval(&FunctionType::Env, &context, None, None)
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
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr
            .resolve(&FunctionType::Env, &context, None, None)
            .unwrap()
            .eval(&FunctionType::Env, &context, None, None)
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

        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr.simplify(&FunctionType::Env, &context).unwrap();

        let tokens2 = Token::validate(get_tokens(simpl)).unwrap();
        let (rest2, expr2) = complete_expression(&tokens2).unwrap();
        assert_eq!(rest2, vec![]);

        assert_eq!(res, expr2);
    }

    // testing the simplify process
    #[rstest]
    #[case("- true", EvalErrorType::NotANumber)]
    #[case("12 | true", EvalErrorType::NotABool)]
    #[case("(xyz - 1) * (12 + true)", EvalErrorType::InvalidOperation)]
    #[case("(xyz - 1) * (true + true)", EvalErrorType::InvalidOperation)]
    #[case("(xyz * \"1\") * (12 + true)", EvalErrorType::InvalidOperation)]
    pub fn compl_expr_error_test(
        context: TaskContext,
        #[case] txt: &str,
        #[case] err: EvalErrorType,
    ) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, expr) = complete_expression(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let res = expr.simplify(&FunctionType::Env, &context);
        assert_eq!(res, Err(err.no_pos()));
    }
}
