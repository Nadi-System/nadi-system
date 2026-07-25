use crate::parser::{
    components::*,
    errors::{ParseError, ParseErrorType},
    tokenizer::{RawToken, Token},
};
use abi_stable::std_types::RString;
use nadi_core::network::{NodeInput, StrPath};
use nom::{
    branch::alt,
    combinator::map,
    multi::separated_list1,
    sequence::{delimited, separated_pair},
    Finish,
};

pub fn node_name<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, RString> {
    err_ctx(
        &ParseErrorType::ValueError("Invalid node name"),
        alt((
            // keywords are valid because they are like variables, but
            // can only be used as string in tasks
            map(alt((variable, integer, float, boolean, keyword)), |v| {
                RString::from(v.content)
            }),
            map(string_val, RString::from),
        )),
    )(inp)
}

pub fn node_group<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<RString>> {
    delimited(
        brace_start,
        separated_list1(maybe_space(comma), maybe_space(node_name)),
        maybe_space(brace_end),
    )(inp)
}

pub fn node_input<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, NodeInput> {
    alt((
        map(str_path, NodeInput::Path),
        map(group_path, |(a, b)| NodeInput::Group(a, b)),
        // has to be after the path ones otherwise it will read just
        // the first node
        map(node_name, NodeInput::Single),
    ))(inp)
}

// TODO: just make the multiple node connections as well

// make a node or group, and then make single edge, or multiple edge

// {a,b} -> c
// c -> {a ,b}
// {a, b} -> {c,d}
// a -> b -> c

// might also consider adding undirected network, it will have nodes, maybe add edges field to the nodes that is only present for undirected.
// or edges enum, that is either undirected(edges) or directed(inp, out).
// could be made easy with better error handling, where different errors can be converted to evalerror. maybe that is a good next step

pub fn str_path<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, StrPath> {
    let (rest, (start, end)) = separated_pair(
        node_name,
        err_ctx(&ParseErrorType::ExpectedPath, maybe_space(path_sep)),
        err_ctx(&ParseErrorType::IncompletePath, maybe_space(node_name)),
    )(inp)?;
    Ok((rest, StrPath::new(start, end)))
}

pub fn group_path<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, (Vec<RString>, Vec<RString>)> {
    let (rest, (start, end)) = separated_pair(
        alt((node_group, map(node_name, |n| vec![n]))),
        err_ctx(&ParseErrorType::ExpectedPath, maybe_space(path_sep)),
        err_ctx(
            &ParseErrorType::IncompletePath,
            maybe_space(alt((node_group, map(node_name, |n| vec![n])))),
        ),
    )(inp)?;
    Ok((rest, (start, end)))
}

pub fn network<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<StrPath>> {
    trailing_newlines(newline_separated(str_path))(inp)
}

pub fn parse(tokens: Vec<RawToken>) -> Result<Vec<StrPath>, ParseError> {
    let tokens = Token::validate(tokens)?;
    match network(&tokens).finish() {
        Ok((rest, paths)) => {
            if rest.is_empty() {
                Ok(paths)
            } else {
                match maybe_newline(str_path)(rest).finish() {
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
    #[case("12.23")]
    #[case("12")]
    #[case("012")]
    #[case("name")]
    #[case("node_name")]
    #[should_panic]
    #[case("0_node_name")]
    #[should_panic]
    #[case("node-name")]
    pub fn node_name_test(#[case] txt: &str) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, name) = node_name(&tokens).unwrap();
        assert!(rest.is_empty());
        assert_eq!(name, txt);
    }

    #[rstest]
    #[case("12.23->name", ("12.23", "name"))]
    #[case("12 -> \"12\"", ("12", "12"))]
    #[case("012-> xyz_is_12", ("012", "xyz_is_12"))]
    #[case("node_name -> name", ("node_name", "name"))]
    #[should_panic]
    #[case("0_node_name -> name", ("0_node_name", "name"))]
    #[should_panic]
    #[case("node-name -> another", ("node-name", "another"))]
    pub fn str_path_test(#[case] txt: &str, #[case] path: (&str, &str)) {
        let tokens = Token::validate(get_tokens(txt)).unwrap();
        let (rest, p) = str_path(&tokens).unwrap();
        let path2 = (p.start.as_str(), p.end.as_str());
        assert!(rest.is_empty());
        assert_eq!(path2, path);
    }

    #[rstest]
    #[case("12.23->name", vec![("12.23", "name")])]
    #[case("12 -> \"12\"", vec![("12", "12")])]
    #[case("012-> xyz_is_12", vec![("012", "xyz_is_12")])]
    #[case("valid -> edge \nnode_name -> another", vec![("valid", "edge"), ("node_name", "another")])]
    #[case("# test this \nnode_name -> another", vec![("node_name", "another")])]
    pub fn parse_test(#[case] txt: &str, #[case] paths: Vec<(&str, &str)>) {
        let tokens = get_tokens(txt);
        let edges = parse(tokens).unwrap();
        let paths2: Vec<_> = edges
            .iter()
            .map(|p| (p.start.as_str(), p.end.as_str()))
            .collect();
        assert_eq!(paths2, paths);
    }

    #[rstest]
    #[case("0_node_name -> name", 1)]
    #[case("valid -> edge \nnode-name -> another", 2)]
    #[case("# test this \nnode-name -> another", 2)]
    #[should_panic]
    #[case("012-> xyz_is_12", 1)]
    pub fn parse_error_test(#[case] txt: &str, #[case] line: usize) {
        let tokens = get_tokens(txt);
        let err = parse(tokens).err().unwrap();
        assert_eq!(err.line, line);
    }
}
