use crate::functions::{FunctionArg, FunctionCall, FunctionType, KeyVal, Propagation};
use crate::parser::attrs::{attr_any, attr_key_val};
use crate::parser::string::parse_string;
use crate::parser::{eol, identifier, node_path, sp, Res};

use abi_stable::std_types::{RString, RVec};

use nom::error::convert_error;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::multispace0,
    combinator::{all_consuming, map, opt, value},
    error::context,
    multi::{many0, separated_list1},
    sequence::{delimited, preceded, terminated, tuple},
};

fn parse_arg(txt: &str) -> Res<&str, FunctionArg> {
    let (txt, _) = multispace0(txt)?;
    context(
        "argument",
        alt((
            // kwarg before arg as "key"=val will parse "key" string and
            // error out.
            map(attr_key_val, |(k, v)| {
                FunctionArg::KwArg(KeyVal { key: k, val: v })
            }),
            map(attr_any, FunctionArg::Arg),
        )),
    )(txt)
}

fn parse_args(txt: &str) -> Res<&str, Option<Vec<FunctionArg>>> {
    context(
        "function arguments",
        delimited(
            preceded(sp, tag("(")),
            opt(separated_list1(
                preceded(sp, tag(",")),
                preceded(sp, parse_arg),
            )),
            preceded(sp, tag(")")),
        ),
    )(txt)
}

fn node_list(txt: &str) -> Res<&str, RVec<RString>> {
    let (rest, lst) = context(
        "node list",
        preceded(
            sp,
            separated_list1(
                preceded(sp, tag(",")),
                preceded(
                    sp,
                    alt((
                        map(parse_string, RString::from),
                        map(identifier, RString::from),
                    )),
                ),
            ),
        ),
    )(txt)?;
    Ok((rest, lst.into()))
}

pub fn node_list_or_path(txt: &str) -> Res<&str, Propagation> {
    delimited(
        preceded(sp, tag("[")),
        alt((
            map(node_path, Propagation::Path),
            map(node_list, Propagation::List),
        )),
        preceded(sp, tag("]")),
    )(txt)
}

fn propagation(txt: &str) -> Res<&str, Propagation> {
    context(
        "propagation",
        alt((
            value(Propagation::Sequential, tag(".sequential")),
            value(Propagation::Inverse, tag(".inverse")),
            value(Propagation::InputsFirst, tag(".inputsfirst")),
            value(Propagation::OutputFirst, tag(".outputfirst")),
            node_list_or_path,
        )),
    )(txt)
}

fn node_task_type(txt: &str) -> Res<&str, FunctionType> {
    preceded(
        sp,
        preceded(tag("node"), map(propagation, FunctionType::Node)),
    )(txt)
}

fn function_type(txt: &str) -> Res<&str, FunctionType> {
    context(
        "tasks type",
        preceded(
            tuple((sp, opt(tag("!")))), // temp ignore !
            alt((
                value(FunctionType::Network, preceded(sp, tag("network"))),
                node_task_type,
                value(
                    FunctionType::Node(Propagation::Sequential),
                    preceded(sp, tag("node")),
                ),
            )),
        ),
    )(txt)
}

pub fn parse_function(txt: &str) -> Res<&str, FunctionCall> {
    // TODO make it terminated in either \n or ;
    let (rest, (tt, func, args)) = context(
        "task function call",
        delimited(sp, tuple((function_type, identifier, parse_args)), eol),
    )(txt)?;
    Ok((rest, FunctionCall::new(tt, func, args)))
}

pub fn parse_script(txt: &str) -> Res<&str, Vec<FunctionCall>> {
    context(
        "tasks script",
        all_consuming(terminated(preceded(sp, many0(parse_function)), sp)),
    )(txt)
}

pub fn parse_script_complete(txt: &str) -> Result<Vec<FunctionCall>, String> {
    // let's add the final line end as the file are likely to miss them
    let (_rest, val) = match parse_script(&format!("{}\n", txt)) {
        Ok(v) => v,
        Err(e) => {
            let er = match e {
                nom::Err::Error(er) | nom::Err::Failure(er) => er,
                nom::Err::Incomplete(_er) => panic!("shouldn't happen"),
            };
            return Err(convert_error(txt, er));
        }
    };
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attrs::Date;
    use abi_stable::std_types::RHashMap;
    use rstest::rstest;

    #[rstest]
    #[case("true", FunctionArg::Arg(Attribute::Bool(true)), "")]
    #[case("false", FunctionArg::Arg(Attribute::Bool(false)), "")]
    #[case(" false ", FunctionArg::Arg(Attribute::Bool(false)), " ")]
    #[case("true", FunctionArg::Arg(Attribute::Bool(true)), "")]
    #[case(
        "\"hello\"",
        FunctionArg::Arg(Attribute::String(RString::from("hello"))),
        ""
    )]
    #[case("foo=true", FunctionArg::KwArg(KeyVal {
        key: RString::from("foo"),
        val: Attribute::Bool(true),
    }), "")]
    #[case("123", FunctionArg::Arg(Attribute::Integer(123i64)), "")]
    #[case("-0.5", FunctionArg::Arg(Attribute::Float(-0.5f64)), "")]
    #[case("\"foo bar\"=2022-01-01", FunctionArg::KwArg(KeyVal {
        key: RString::from("foo bar"),
        val: Attribute::Date(Date::new(2022, 1, 1)),
    }), "")]
    fn parse_arg_test(#[case] txt: &str, #[case] value: FunctionArg, #[case] reminder: &str) {
        let (rest, n) = parse_arg(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case("network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)\n", FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})])), "")]
    #[case("
# node print_attrs(\"NAME\", \"INDEX\")
network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)
", FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})])), "")]
    #[case("
# node print_attrs()
# node load_attrs(\"attrs/{_NAME}.toml\")
# node print_all_attrs()
# node rendertext(\"Test {NAME}\")
# network fancy_print()
# node render(\"Test {NAME}\")
# network save_graphviz(\"/tmp/test.gv\")
# 
# node print_attrs(\"NAME\", \"INDEX\")
network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)
", FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})])), "")]
    fn parse_function_test(#[case] txt: &str, #[case] value: FunctionCall, #[case] reminder: &str) {
        let (rest, n) = parse_function(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case("network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)\n", vec![
    FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})]))], "")]
    #[case("
# node print_attrs(\"NAME\", \"INDEX\")
network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)
", vec![
    FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})]))], "")]
    #[case("
# node print_attrs()
# node load_attrs(\"attrs/{_NAME}.toml\")
# node print_all_attrs()
# node rendertext(\"Test {NAME}\")
# network fancy_print()
# node render(\"Test {NAME}\")
# network save_graphviz(\"/tmp/test.gv\")
# 
# node print_attrs(\"NAME\", \"INDEX\")
network debug(\"test.table\", \"/tmp/test.tex\", true, radius=0.2, offset=1.4)
", vec![
    FunctionCall::new(FunctionType::Network, "debug", Some(vec![FunctionArg::Arg(Attribute::String("test.table".into())), FunctionArg::Arg(Attribute::String("/tmp/test.tex".into())), FunctionArg::Arg(Attribute::Bool(true)), FunctionArg::KwArg(KeyVal{key:"offset".into(), val: Attribute::Float(1.4)}), FunctionArg::KwArg(KeyVal{key:"radius".into(), val: Attribute::Float(0.2)})]))], "")]
    fn parse_script_test(
        #[case] txt: &str,
        #[case] value: Vec<FunctionCall>,
        #[case] reminder: &str,
    ) {
        let (rest, n) = parse_script(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }
}
