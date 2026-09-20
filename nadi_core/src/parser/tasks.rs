use crate::parser::{
    components::*,
    errors::MatchErr,
    expressions::{
        complete_expression, function_def, input_variable_only, maybe_silent_expression, raw_expr,
    },
    network::node_name,
    tokenizer::{RawToken, Token},
    ParseError, ParseErrorType,
};
use crate::{
    expressions::Position,
    network::{PropOrder, Propagation, SelectEdgeFromTo, SelectEdges, SelectNodes},
    structs::{NadiAttrType, NadiStruct},
    tasks::{FunctionType, Task},
};
use abi_stable::std_types::{RString, RVec};
use nom::{
    branch::alt,
    combinator::{cut, map, opt, value},
    multi::{separated_list0, separated_list1},
    sequence::{delimited, pair, preceded, separated_pair, tuple},
    Finish,
};
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
        |v| v.into_iter().collect(),
    )(inp)
}

pub fn prop_nodes<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SelectNodes> {
    alt((prop_nodes_list, prop_nodes_expr))(inp)
}

pub fn prop_nodes_list<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SelectNodes> {
    delimited(
        bracket_start,
        cut(alt((
            map(maybe_newline(node_list), SelectNodes::List),
            map(
                maybe_newline(preceded(star, input_variable_only)),
                SelectNodes::Var,
            ),
        ))),
        maybe_newline(cut(err_ctx(&ParseErrorType::Unclosed("]"), bracket_end))),
    )(inp)
}

pub fn select_edge_node<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SelectEdgeFromTo> {
    alt((
        value(SelectEdgeFromTo::NodeCtx, maybe_newline(kw_node)),
        map(maybe_newline(node_name), |n| {
            SelectEdgeFromTo::Node(n.to_string())
        }),
        map(
            maybe_newline(separated_list1(
                maybe_newline(comma),
                maybe_newline(map(node_name, |s| s.to_string())),
            )),
            SelectEdgeFromTo::Nodes,
        ),
    ))(inp)
}

pub fn select_edges<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SelectEdges> {
    alt((
        delimited(
            bracket_start,
            cut(alt((
                map(
                    maybe_newline(separated_pair(
                        maybe_space(opt(select_edge_node)),
                        maybe_space(path_sep),
                        maybe_space(opt(select_edge_node)),
                    )),
                    |(a, b)| SelectEdges::One(a.unwrap_or_default(), b.unwrap_or_default()),
                ),
                map(
                    maybe_newline(preceded(star, input_variable_only)),
                    SelectEdges::Var,
                ),
            ))),
            maybe_newline(cut(err_ctx(&ParseErrorType::Unclosed("]"), bracket_end))),
        ),
        delimited(
            paren_start,
            map(
                maybe_newline(raw_expr(complete_expression)),
                SelectEdges::Expr,
            ),
            maybe_newline(cut(err_ctx(&ParseErrorType::Unclosed(")"), paren_end))),
        ),
    ))(inp)
}

pub fn prop_nodes_expr<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, SelectNodes> {
    delimited(
        paren_start,
        map(
            maybe_newline(raw_expr(complete_expression)),
            SelectNodes::Expr,
        ),
        maybe_newline(paren_end),
    )(inp)
}

pub fn propagation<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Option<Propagation>> {
    let (rest, (order, nodes)) = tuple((opt(prop_order), opt(prop_nodes)))(inp)?;
    if order.is_none() && nodes.is_none() {
        Ok((rest, None))
    } else {
        Ok((
            rest,
            Some(Propagation {
                order: order.unwrap_or_default(),
                nodes: nodes.unwrap_or_default(),
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

pub fn nadi_struct_def<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, NadiStruct> {
    let (rest, (_, name, fields)) = tuple((
        kw_struct,
        maybe_space(variable_name), // name
        delimited(
            maybe_space(brace_start),
            separated_list0(
                maybe_space(comma),
                // fd, ty, val
                tuple((
                    maybe_newline(variable_name),
                    preceded(maybe_space(colon), maybe_space(attr_type)),
                    opt(preceded(
                        maybe_space(assignment),
                        maybe_space(attribute_inline),
                    )),
                )),
            ),
            maybe_newline(brace_end),
        ),
    ))(inp)?;
    let mut nstr = NadiStruct::with_name(name);
    for (fd, ty, val) in fields {
        if let Some(val) = val {
            nstr.values.insert(fd.clone().into(), val);
        }
        nstr.fields.insert(fd.into(), ty);
    }
    Ok((rest, nstr))
}

pub fn typed_var<'a, 'b>(
    inp: &'a [Token<'b>],
) -> MatchRes<'a, 'b, (Vec<String>, Option<NadiAttrType>)> {
    pair(
        dot_variable,
        opt(preceded(maybe_space(colon), maybe_space(attr_type))),
    )(inp)
}

pub fn help_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    map(
        tuple((
            kw_help,
            opt(after_space(keyword_val)),
            opt(after_space(alt((
                map(dot_variable, |v| v.join(".")),
                string_val,
            )))),
        )),
        |(_, kw, st)| Task::Help(kw, st),
    )(inp)
}

pub fn hook_task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<Task>> {
    let (rest, tasks) = preceded(kw_hook, maybe_space(tasks_block))(inp)?;
    Ok((rest, tasks))
}

pub fn task<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Task> {
    alt((
        map(function_def, Task::Function),
        map(nadi_struct_def, Task::StructDef),
        // set should be before get
        // map(set_series_task, Task::SetSeries),
        // map(get_series_task, Task::GetSeries),
        // eval task should come after series, otherwise series gets
        // interpreted as attribute expression
        // // removing temp to see if expression works
        // map(eval_task, Task::Eval),
        // map(attr_task, Task::Attr),
        // map(cond_task, Task::Conditional),
        // map(while_task, Task::WhileLoop),
        map(hook_task, Task::Hook),
        // map(import_task, Task::Import),
        map(raw_expr(maybe_silent_expression), Task::Expr),
        help_task,
        value(Task::Clear, kw_clear),
        value(Task::Exit, kw_exit),
        value(Task::Network, kw_network),
        value(Task::Env, kw_env),
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

/// get function at the given position
///
/// not super accurate about the context though
pub fn get_function_at(tasks: &str, line: usize, column: usize) -> Option<(FunctionType, String)> {
    // if the current line can be parsed into a proper task, use that
    let task_str = tasks.lines().nth(line)?;
    let tokens_v = crate::parser::tokenizer::get_tokens(task_str);
    let mut tokens = tokens_v.iter().peekable();
    let mut ty = None;
    let mut name = None;
    let mut col = 0;
    // let mut ind = 0;
    while col <= column {
        let tk = match tokens.next() {
            Some(t) => t,
            None => break,
        };
        col += tk.content.len();
        use crate::parser::tokenizer::TaskToken;
        match &tk.ty {
            TaskToken::Function if col >= column => {
                name = Some(tk.content.to_string());
            }
            TaskToken::Keyword(kw) if ty.is_none() => {
                ty = FunctionType::from_keyword(kw);
            }
            _ => (),
        }
    }
    name.map(|n| (ty.unwrap_or_default(), n))
}

/// Get the function detail if we're currently inside its context
///
/// Not super accurate about the type if it's derived from context
pub fn get_current_function_context(
    tasks: &str,
    line: usize,
    column: usize,
) -> Option<(FunctionType, String)> {
    // discard everything till our mark
    let mut lines = Vec::with_capacity(line + 1);
    for line in tasks.lines().take(line) {
        lines.push(line);
    }
    lines.push(&tasks.lines().nth(line)?[..column]);
    let mut tasks = lines.join("\n");
    let mut tokens = crate::parser::tokenizer::get_tokens(&tasks);
    if Token::validate(tokens.clone()).is_err() {
        // in cases where we're in middle of a string and that makes it invalid
        tasks.push('"');
        tokens = crate::parser::tokenizer::get_tokens(&tasks)
    }
    let err = parse(tokens).err()?;
    match err.ty {
        ParseErrorType::IncompleteFunction(ty, f) => Some((ty.unwrap_or_default(), f)),
        _ => None,
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
    #[case("env {x}")]
    #[case("env {x + 1}")]
    #[case("env.call_sth(x + 1)")]
    #[case("env.x")]
    #[case("node.x")]
    #[case("network.x")]
    #[case("inputs.x")]
    #[case("env {(x + 1) != 5}")] //
    #[case("env {\"val\" in selected_vals}")]
    #[case("env.x = []")]
    #[case("env.echo(x)")]
    #[case("network {load_file(test)}")]
    #[case("network.gis.load_file(12)")]
    #[case("node {call_sth(x + 1)}")]
    #[case("node.some_func()")]
    #[case("nodes<inverse>.some_func()")]
    #[case("nodes<outputfirst>[a] {some_func()}")]
    #[case("nodes<inputsfirst>(cond).some_func()")]
    #[case("edges[a -> b] {some_func()}")]
    #[case("edges(cond).some_func()")]
    #[case("nodes[a] {some_func()}")]
    #[case("nodes(cond) {(some_func() + 12) > 12}")] //
    #[case("while (true) {\n\tenv {echo(x)}\n}")]
    #[case("if (true) {\n\tenv.echo(x)\n} else {\n\tenv.echo(y)\n}")]
    #[case("while (true) {\n\tenv.echo(x)\n}")]
    #[case("struct HiThere {\nval: Integer = 0\n}")]
    pub fn task_valid_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tasks) = task(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let tsk = tasks.to_string().replace([' ', '\n', '\t'], "");
        let txt = txt.replace([' ', '\n', '\t'], "");
        assert_eq!(txt, tsk);
    }

    #[rstest]
    #[case("struct HiThere {\nval: Integer = 0\n}")]
    pub fn struct_def_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, tasks) = nadi_struct_def(&tokens).unwrap();
        assert_eq!(rest, vec![]);
        let tsk = tasks.to_string().replace([' ', '\n', '\t'], "");
        let txt = txt.replace([' ', '\n', '\t'], "");
        assert_eq!(txt, tsk);
    }

    #[rstest]
    #[case("\n # test\nexit")]
    #[case("help")]
    #[case("help node")]
    #[case("help variable")]
    #[case("help network var")]
    #[case("env.x")]
    pub fn parse_valid_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        parse(tokens).unwrap();
    }

    /// Testing the codes in mdbook
    #[rstest]
    #[case(
        "network.load_file(\"./data/mississippi.net\")\nnode[ohio].render(\"{_NAME:case(title)} River\")"
    )]
    pub fn parse_valid_mdbook_test(#[case] txt: &str) {
        let tokens = get_tokens(txt);
        parse(tokens).unwrap();
    }

    #[rstest]
    #[case("  s🇮ome()", Some("some"))]
    #[case("som🇮e()", Some("some"))]
    #[case("🇮some()", Some("some"))]
    #[case("some(🇮)", None)]
    #[case("some()\n🇮", None)]
    #[case("\nsom🇮e()", Some("some"))]
    #[case("#some()\n🇮what()", Some("what"))]
    #[case("#some()\nnet.w🇮hat()", Some("what"))]
    #[case("#some()\nnetwork.🇮what()", Some("what"))]
    pub fn get_current_function_test(#[case] txt: &str, #[case] name: Option<&str>) {
        let (pre, post) = txt.split_once("🇮").unwrap();
        let line = pre.split('\n').count() - 1;
        let col = pre.split('\n').next_back().map(|l| l.len()).unwrap_or(0);
        let tasks = format!("{pre}{post}");
        let res = get_function_at(&tasks, line, col);
        let fname = res.as_ref().map(|(_, n)| n.as_str());
        assert_eq!(fname, name);
    }

    #[rstest]
    #[case("  🇮some()\nother()", None)]
    #[case(" som🇮e()\nother()", None)]
    #[case(" some()\nother(🇮)", Some("other"))]
    #[case(" some()\n#other(🇮)", None)]
    #[case("some(🇮)\nother()", Some("some"))]
    #[case("\nsome(🇮value)\nother()", Some("some"))]
    #[case("\nwhat.some(value🇮)\nother()", Some("what.some"))]
    #[case("\nnode.what.some(value🇮)\nother()", Some("what.some"))]
    #[case("load_str(🇮)", Some("load_str"))]
    #[case("load_str(\"a -> b\"🇮)", Some("load_str"))]
    #[case("load_str(\"a -> b🇮\n b -> d\n c -> d\")", Some("load_str"))]
    #[case("net.load_str(🇮)", Some("load_str"))]
    #[case("net.load_str(\"a -> b\"🇮)", Some("load_str"))]
    #[case("net.load_str(\"a -> b🇮\n b -> d\n c -> d\")", Some("load_str"))]
    pub fn get_current_function_context_test(#[case] txt: &str, #[case] name: Option<&str>) {
        let (pre, post) = txt.split_once("🇮").unwrap();
        let line = pre.split('\n').count() - 1;
        let col = pre.split('\n').next_back().map(|l| l.len()).unwrap_or(0);
        let tasks = format!("{pre}{post}");
        let res = get_current_function_context(&tasks, line, col);
        let fname = res.as_ref().map(|(_, n)| n.as_str());
        assert_eq!(fname, name);
    }
}
