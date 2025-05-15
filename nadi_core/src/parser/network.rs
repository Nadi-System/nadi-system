use crate::parser::{
    components::*,
    errors::{ParseError, ParseErrorType},
    tokenizer::{check_tokens, Token},
};
use nadi_core::network::StrPath;
use nom::{branch::alt, combinator::map, sequence::separated_pair, Finish};

pub fn node_name<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, String> {
    err_ctx(
        &ParseErrorType::ValueError("Invalid node name"),
        alt((
            map(alt((variable, integer, float, boolean)), |v| {
                v.content.to_string()
            }),
            string_val,
        )),
    )(inp)
}

pub fn str_path<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, StrPath> {
    let (rest, (start, end)) = separated_pair(
        node_name,
        err_ctx(&ParseErrorType::ExpectedPath, maybe_space(path_sep)),
        err_ctx(&ParseErrorType::IncompletePath, maybe_space(node_name)),
    )(inp)?;
    Ok((rest, StrPath::new(start.into(), end.into())))
}

pub fn network<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<StrPath>> {
    trailing_newlines(newline_separated(str_path))(inp)
}

pub fn parse(tokens: &[Token]) -> Result<Vec<StrPath>, ParseError> {
    check_tokens(tokens)?;
    match network(tokens).finish() {
        Ok((rest, paths)) => {
            if rest.is_empty() {
                Ok(paths)
            } else {
                let err = maybe_newline(str_path)(rest)
                    .finish()
                    .expect_err("Rest should be empty if network parse is complete");
                Err(ParseError::new(tokens, err.internal.input, err.ty))
            }
        }
        Err(e) => Err(ParseError::new(tokens, e.internal.input, e.ty)),
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
        let tokens = get_tokens(txt);
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
        let tokens = get_tokens(txt);
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
        let edges = parse(&tokens).unwrap();
        let paths2: Vec<_> = edges
            .iter()
            .map(|p| (p.start.as_str(), p.end.as_str()))
            .collect();
        assert_eq!(paths2, paths);
    }

    #[rstest]
    #[case("0_node_name -> name", 1, 3)]
    #[case("valid -> edge \nnode-name -> another", 2, 1)]
    #[case("# test this \nnode-name -> another", 2, 1)]
    #[should_panic]
    #[case("012-> xyz_is_12", 1, 1)]
    pub fn parse_error_test(#[case] txt: &str, #[case] line: usize, #[case] col: usize) {
        let tokens = get_tokens(txt);
        let err = parse(&tokens).err().unwrap();
        assert_eq!(err.line, line);
        assert_eq!(err.col, col);
    }
}
