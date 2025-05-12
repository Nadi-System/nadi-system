use crate::parser::{
    components::*,
    errors::{ParseError, ParseErrorType},
    tokenizer::{check_tokens, TaskToken, Token, VecTokens},
};
use nadi_core::network::StrPath;
use nom::{
    branch::alt,
    combinator::{all_consuming, map},
    multi::separated_list0,
    sequence::separated_pair,
    Finish,
};

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
        err_ctx(&ParseErrorType::Incomplete, maybe_space(node_name)),
    )(inp)?;
    Ok((rest, StrPath::new(start.into(), end.into())))
}

pub fn network<'a, 'b>(inp: &'a [Token<'b>]) -> MatchRes<'a, 'b, Vec<StrPath>> {
    traling_newlines(newline_separated(str_path))(inp)
}

pub fn parse(tokens: Vec<Token>) -> Result<Vec<StrPath>, ParseError> {
    check_tokens(&tokens)?;
    match network(&tokens).finish() {
        Ok((rest, paths)) => {
            if rest.is_empty() {
                Ok(paths)
            } else {
                let err = maybe_newline(str_path)(rest) // need this to fail
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
        assert!(name == txt);
    }

    #[rstest]
    #[case("12.23->name", ("12.23", "name"))]
    #[case("12 -> \"12\"", ("12", "12"))]
    #[case("012-> xyz_is_12", ("012", "xyz_is_12"))]
    #[should_panic]
    #[case("0_node_name -> name", ("0_node_name", "name"))]
    #[should_panic]
    #[case("node-name -> another", ("node-name", "another"))]
    pub fn str_path_test(#[case] txt: &str, #[case] path: (&str, &str)) {
        let tokens = get_tokens(txt);
        let (rest, p) = str_path(&tokens).unwrap();
        let path2 = (p.start.as_str(), p.end.as_str());
        assert!(rest.is_empty());
        assert!(path2 == path);
    }

    #[rstest]
    #[case("0_node_name -> name", 1, 3)]
    // invalid token error (from -) comes before node error, change if we allow
    // math operators later
    #[case("node-name -> another", 1, 5)]
    pub fn parse_error_test(#[case] txt: &str, #[case] line: usize, #[case] col: usize) {
        let tokens = get_tokens(txt);
        let err = parse(tokens).err().unwrap();
        assert!(err.line == line);
        assert!(err.col == col);
    }
}
