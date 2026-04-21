use crate::expressions::{
    BiOperator, ExprProgress, ExprType, ExprWithContext, FunctionCall, ImportExpr, InputVar,
    Position, RawExpr, SetVariable, UniOperator, VarType,
};
use crate::network::{PropCondition, PropNodes, PropOrder};
use crate::parser::{
    components::*,
    errors::{MatchErr, ParseErrorType},
    tasks::propagation,
    tokenizer::Token,
};
use crate::structs::NadiStructExpr;
use crate::tasks::TaskKeyword;
use crate::udf::UserFunction;
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::{many0, many1, separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
};
use std::path::PathBuf;

fn set_pos(ty: ExprType<RawExpr>, tk: &[Token<'_>]) -> RawExpr {
    RawExpr::new(ty, tk.position())
}

/// Matches the next one that might have spaces, newlines or comments before it
pub fn raw_expr<'a, 'b: 'a, F>(mut f: F) -> impl FnMut(&'a [Token<'b>]) -> MatchRes<'a, 'b, RawExpr>
where
    F: nom::Parser<&'a [Token<'b>], ExprType<RawExpr>, MatchErr<'a, 'b>>,
{
    move |i: &'a [Token<'b>]| match f.parse(i) {
        Ok((rest, o)) => Ok((
            rest,
            RawExpr::new(
                o,
                // it could crash things
                i.position(),
            ),
        )),
        Err(e) => Err(e),
    }
}

/// expressions that can potentially return value
pub fn value_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    alt((
        expr_with_context,
        expr_maybe_range,
        map(function_call, ExprType::Function),
        map(template_val, ExprType::Render),
        array_expr,
        table_expr,
        uni_operator_expr,
        if_else_expr,
        try_catch_expr,
        for_each_expr,
        while_expr,
        loop_expr,
    ))(inp)
}

pub fn expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    alt((
        // map(nadi_struct_expr, Expression::StructExpr),
        expr_set_variable,
        value_expression,
        import_expr,
        value(ExprType::Continue, kw_continue),
        value(ExprType::None, none),
        progress_expr,
        map(
            preceded(kw_error, after_space(string_val)),
            ExprType::UserError,
        ),
        map(
            preceded(
                kw_return,
                opt(after_space(raw_expr(maybe_silent_expression))),
            ),
            |e| ExprType::Return(e.map(Box::new)),
        ),
        map(
            preceded(
                kw_break,
                opt(after_space(raw_expr(maybe_silent_expression))),
            ),
            |e| ExprType::Break(e.map(Box::new)),
        ),
        series,
    ))(inp)
}

pub fn import_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
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
        ExprType::Import(ImportExpr {
            name: name.content.to_string(),
            path,
            tasks,
        }),
    ))
}

pub fn expr_with_context<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (vt, expr)) = pair(
        variable_type,
        alt((
            maybe_space(raw_expr(expression_block)),
            // this is for backward compatibility
            after_space(raw_expr(complete_expression)),
        )),
    )(inp)?;
    Ok((rest, ExprType::WithContext(ExprWithContext::new(vt, expr))))
}

pub fn expr_set_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    map(
        tuple((
            opt(terminated(variable_type, dot)),
            task_dot_variable,
            maybe_space(assignment),
            maybe_space(raw_expr(complete_expression)),
            opt(semicolon),
        )),
        |(vt, (var, indices), _, expr, silent)| {
            ExprType::SetVar(SetVariable::new(
                InputVar::new(vt.clone(), var.clone(), indices.clone(), inp.position()),
                expr,
                silent.is_some(),
            ))
        },
    )(inp)
}

pub fn expr_literal<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    map(primitives, ExprType::Value)(inp)
}

pub fn expr_maybe_range<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (start, step, end)) = tuple((
        alt((input_variable, expr_literal)),
        opt(preceded(
            colon,
            maybe_space(raw_expr(alt((input_variable, expr_literal)))),
        )),
        opt(preceded(
            colon,
            maybe_space(raw_expr(alt((input_variable, expr_literal)))),
        )),
    ))(inp)?;
    if let Some(step) = step {
        if let Some(end) = end {
            Ok((
                rest,
                ExprType::Range(
                    Box::new(set_pos(start, inp)),
                    Some(Box::new(step)),
                    Box::new(end),
                ),
            ))
        } else {
            Ok((
                rest,
                ExprType::Range(Box::new(set_pos(start, inp)), None, Box::new(step)),
            ))
        }
    } else {
        // just an expression
        Ok((rest, start))
    }
}

pub fn array_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    delimited(
        bracket_start,
        alt((
            map(
                tuple((
                    maybe_newline(raw_expr(alt((expression_block, value_expression)))),
                    delimited(
                        maybe_space(kw_for),
                        after_space(map(variable, |v| v.content.to_string())),
                        after_space(kw_in),
                    ),
                    after_space(raw_expr(value_expression)),
                )),
                |(expr, var, parent)| ExprType::ArrayGen(Box::new(expr), var, Box::new(parent)),
            ),
            map(
                separated_list0(
                    maybe_space(comma),
                    maybe_newline(raw_expr(complete_value_expression)),
                ),
                ExprType::Array,
            ),
        )),
        maybe_newline(bracket_end),
    )(inp)
}

pub fn table_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    delimited(
        brace_start,
        alt((
            map(
                tuple((
                    maybe_newline(raw_expr(alt((expression_block, value_expression)))),
                    maybe_space(assignment),
                    maybe_newline(raw_expr(alt((expression_block, value_expression)))),
                    delimited(
                        maybe_space(kw_for),
                        after_space(separated_pair(
                            map(variable, |v| v.content.to_string()),
                            after_space(comma),
                            after_space(map(variable, |v| v.content.to_string())),
                        )),
                        after_space(kw_in),
                    ),
                    after_space(raw_expr(expression)),
                )),
                |(key, _, expr, (var1, var2), parent)| {
                    ExprType::MapGen(Box::new(key), Box::new(expr), var1, var2, Box::new(parent))
                },
            ),
            map(opt(maybe_newline(kw_args)), |exprs| {
                ExprType::Map(exprs.unwrap_or_default())
            }),
        )),
        maybe_newline(brace_end),
    )(inp)
}

pub fn expression_group<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    delimited(
        paren_start,
        maybe_newline(multi_expression),
        cut(err_ctx(
            &ParseErrorType::Unclosed(")"),
            maybe_newline(paren_end),
        )),
    )(inp)
}

pub fn expression_block<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    delimited(
        brace_start,
        maybe_newline(multi_expression),
        cut(err_ctx(
            &ParseErrorType::Unclosed("}"),
            maybe_newline(brace_end),
        )),
    )(inp)
}

pub fn progress_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    map(
        tuple((
            kw_progress,
            maybe_space(raw_expr(alt((expression, expression_group)))),
            // somehow it wants a space here
            maybe_space(assignment),
            maybe_space(raw_expr(alt((expression, expression_group)))),
            maybe_space(kw_in),
            maybe_space(raw_expr(alt((expression, expression_group)))),
        )),
        |(_, label, _, prog, _, total)| ExprType::Progress(ExprProgress::new(label, prog, total)),
    )(inp)
}

pub fn uni_operator_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (op, expr)) = pair(
        alt((
            value(UniOperator::Not, not),
            value(UniOperator::Negative, dash),
            value(UniOperator::Positive, plus),
        )),
        maybe_newline(raw_expr(alt((expression_group, expression)))),
    )(inp)?;
    Ok((rest, ExprType::UniOp(op, Box::new(expr))))
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
    first: RawExpr,
    args: Vec<(BiOperator, RawExpr)>,
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
    fn parse(self) -> Option<ExprType<RawExpr>> {
        // Two stacks used by the shunting-yard algorithm.
        let mut operand_stack: Vec<RawExpr> = Vec::new();
        let mut operator_stack: Vec<BiOperator> = Vec::new();

        // we don't have position for each operand, maybe we need to store it somewhere
        // let position = self.first.position();
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

        Some(ExprType::Multi(operand_stack))
    }
}

pub fn complete_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (first, args)) = pair(
        alt((expression, expression_group)),
        many0(pair(
            maybe_space(bi_operator),
            maybe_newline(cut(err_ctx(
                &ParseErrorType::IncompleteExpression,
                raw_expr(alt((expression, expression_group))),
            ))),
        )),
    )(inp)?;
    let expr = if args.is_empty() {
        first
    } else {
        BiOpExpr {
            first: set_pos(first, inp),
            args,
        }
        .parse()
        .ok_or(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::IncompleteExpression),
        ))?
    };
    Ok((rest, expr))
}

pub fn complete_value_expression<'a, 'b>(
    inp: &'a [Token<'b>],
) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (first, args)) = pair(
        alt((value_expression, expression_group)),
        many0(pair(
            maybe_newline(bi_operator),
            maybe_newline(cut(err_ctx(
                &ParseErrorType::IncompleteExpression,
                raw_expr(alt((value_expression, expression_group))),
            ))),
        )),
    )(inp)?;
    let expr = if args.is_empty() {
        first
    } else {
        BiOpExpr {
            first: set_pos(first, inp),
            args,
        }
        .parse()
        .ok_or(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::IncompleteExpression),
        ))?
    };
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

pub fn nadi_struct_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, NadiStructExpr<RawExpr>> {
    let (rest, (name, fields)) = tuple((
        variable_name,
        delimited(
            maybe_space(brace_start),
            maybe_newline(newline_separated(tuple((
                variable_name,
                preceded(
                    maybe_space(assignment),
                    maybe_space(raw_expr(complete_expression)),
                ),
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

pub fn if_else_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (_, cond, iftrue, iffalse)) = tuple((
        kw_if,
        maybe_newline(raw_expr(expression_group)),
        maybe_newline(raw_expr(expression_block)),
        opt(preceded(
            maybe_newline(kw_else),
            alt((
                after_space(raw_expr(if_else_expr)),
                maybe_newline(raw_expr(expression_block)),
            )),
        )),
    ))(inp)?;
    Ok((
        rest,
        ExprType::IfElse(Box::new(cond), Box::new(iftrue), iffalse.map(Box::new)),
    ))
}

pub fn try_catch_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (_, try_blk, _, catch_blk)) = tuple((
        kw_try,
        maybe_newline(raw_expr(expression_block)),
        maybe_newline(kw_catch),
        maybe_newline(raw_expr(expression_block)),
    ))(inp)?;
    Ok((
        rest,
        ExprType::TryCatch(Box::new(try_blk), Box::new(catch_blk)),
    ))
}

pub fn for_each_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (var, parent, expr)) = tuple((
        delimited(
            kw_for,
            after_space(map(variable, |v| v.content.to_string())),
            after_space(kw_in),
        ),
        after_space(raw_expr(expression)),
        maybe_newline(raw_expr(expression_block)),
    ))(inp)?;
    Ok((
        rest,
        ExprType::ForEach(var, Box::new(parent), Box::new(expr)),
    ))
}

pub fn while_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (cond, expr)) = tuple((
        preceded(kw_while, after_space(raw_expr(expression_group))),
        maybe_newline(raw_expr(expression_block)),
    ))(inp)?;
    Ok((rest, ExprType::While(Box::new(cond), Box::new(expr))))
}

pub fn loop_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, expr) = preceded(kw_loop, after_space(raw_expr(expression_block)))(inp)?;
    Ok((rest, ExprType::Loop(Box::new(expr))))
}

pub fn maybe_silent_expression<'a, 'b>(
    inp: &'a [Token<'b>],
) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    let (rest, (expr, silent)) = pair(complete_expression, opt(maybe_space(semicolon)))(inp)?;
    Ok((
        rest,
        if silent.is_some() {
            ExprType::Silent(Box::new(set_pos(expr, inp)))
        } else {
            expr
        },
    ))
}

pub fn multi_expression<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    map(
        newline_separated(raw_expr(maybe_silent_expression)),
        ExprType::Multi,
    )(inp)
}

pub fn variable_type<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, VarType> {
    let (rest, (kw, prop)) = pair(keyword_val, propagation)(inp)?;
    let node = match (&kw, &prop) {
        // make sure node variable type has only one node name instead
        // of any other propagation
        (TaskKeyword::Node, Some(p)) => {
            if p.order != PropOrder::default() || p.condition != PropCondition::default() {
                return Err(nom::Err::Failure(
                    MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
                ));
            };
            match &p.nodes {
                PropNodes::All => None,
                PropNodes::List(lst) => {
                    if let [n] = lst.as_slice() {
                        Some(n.to_string())
                    } else {
                        return Err(nom::Err::Failure(
                            MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
                        ));
                    }
                }
                PropNodes::Path(_) => {
                    return Err(nom::Err::Failure(
                        MatchErr::new(inp).ty(&ParseErrorType::InvalidPropagation),
                    ));
                }
            }
        }
        _ => None,
    };
    match VarType::from_keyword(&kw, prop, node) {
        Some(v) => Ok((rest, v)),
        None => Err(nom::Err::Error(
            MatchErr::new(inp).ty(&ParseErrorType::InvalidKeyword),
        )),
    }
}

pub fn input_variable<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
    map(
        tuple((
            opt(terminated(variable_type, dot)),
            task_dot_variable,
            opt(maybe_space(pair(
                question,
                opt(maybe_space(raw_expr(alt((expression, expression_group))))),
            ))),
        )),
        |(vt, (var, indices), q)| {
            let var = ExprType::Var(InputVar::new(vt, var, indices, inp.position()));
            if let Some((_, val)) = q {
                let var = set_pos(var, inp);
                if let Some(val) = val {
                    ExprType::IfElse(
                        Box::new(set_pos(ExprType::Check(Box::new(var.clone())), inp)),
                        Box::new(var),
                        Some(Box::new(val)),
                    )
                } else {
                    ExprType::Check(Box::new(var))
                }
            } else {
                var
            }
        },
    )(inp)
}

pub fn kw_arg<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (String, RawExpr)> {
    separated_pair(
        // no dot variable in kwargs pair
        map(variable, |t| t.content.to_string()),
        maybe_space(assignment),
        cut(err_ctx(
            &ParseErrorType::MissingValue,
            maybe_space(raw_expr(complete_value_expression)),
        )),
    )(inp)
}

pub fn kw_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<(String, RawExpr)>> {
    separated_list1(comma, maybe_newline(kw_arg))(inp)
}

pub fn pos_args<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<RawExpr>> {
    // complete value expr doesn't have the SetVar that could consume keyword arg
    separated_list1(comma, maybe_newline(raw_expr(complete_value_expression)))(inp)
}

pub fn pos_vars<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<String>> {
    separated_list1(
        comma,
        maybe_newline(map(variable, |v| v.content.to_string())),
    )(inp)
}

type FuncDefArgKwarg = (Vec<String>, Vec<(String, RawExpr)>);
type FuncCallArgKwarg = (Vec<RawExpr>, Vec<(String, RawExpr)>);

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
                        maybe_newline(raw_expr(complete_value_expression)),
                        maybe_newline(comma),
                    )),
                    maybe_newline(kw_args),
                ),
                maybe_newline(paren_end),
            ),
        )),
    )(inp)
}

pub fn function_call<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, FunctionCall<RawExpr>> {
    let (rest, (ty, name, (args, kwargs))) = tuple((
        opt(terminated(variable_type, dot)),
        function,
        cut(func_args),
    ))(inp)?;
    Ok((
        rest,
        FunctionCall::new(
            ty,
            name.content.to_string(),
            args,
            kwargs.into_iter().collect(),
            inp.position(),
        ),
    ))
}

pub fn function_body<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<RawExpr>> {
    delimited(
        brace_start,
        newline_separated(raw_expr(complete_expression)),
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
        maybe_newline(raw_expr(map(function_body, ExprType::Multi))),
    ))(inp)?;
    Ok((
        rest,
        UserFunction::new(name.map(|n| n.content.to_string()), args, kwargs, exprs),
    ))
}

pub fn series<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, ExprType<RawExpr>> {
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
        Some(ind) => ExprType::SeriesValue(vt, ts.is_some(), name, ind),
        None => ExprType::Series(vt, ts.is_some(), name),
    };
    Ok((rest, sr))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::{Attribute, HasAttributes};
    use crate::eval::{Eval, EvalCtx, EvalErrorType};
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
        let (rest, expr) = raw_expr(complete_expression)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let ectx = EvalCtx::default();
        let res = expr.eval_value(&context, &ectx).unwrap();
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
        let (rest, expr) = raw_expr(complete_expression)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let ectx = EvalCtx::default();
        let res = expr.eval_value(&context, &ectx).unwrap();
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
        let (rest, expr) = raw_expr(complete_expression)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let ectx = EvalCtx::default();
        let res1 = expr.eval_value(&context, &ectx).unwrap();

        let tokens = Token::validate(get_tokens(simpl)).unwrap();
        let (rest, expr) = raw_expr(complete_expression)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let ectx = EvalCtx::default();
        let res2 = expr.eval_value(&context, &ectx).unwrap();

        assert_eq!(res1, res2);
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
        let (rest, expr) = raw_expr(complete_expression)(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let ectx = EvalCtx::default();
        let res = expr.eval(&context, &ectx).err().unwrap();
        assert_eq!(res.ty, err);
    }
}
