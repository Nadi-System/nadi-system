use crate::network::StrPath;
use crate::parser::string::parse_string;
use crate::parser::{sp, Res};
use abi_stable::std_types::RString;
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{alpha1, alphanumeric1},
    combinator::{all_consuming, map, opt, recognize},
    error::{context, convert_error},
    multi::{many0, many0_count},
    sequence::{pair, preceded, separated_pair, terminated},
};

fn node_name(input: &str) -> Res<&str, RString> {
    context(
        "node name",
        alt((
            map(
                recognize(pair(
                    alt((alpha1, tag("_"))),
                    // because the name can have -, there needs to be space before ->
                    many0_count(alt((alphanumeric1, tag("_"), tag("-")))),
                )),
                RString::from,
            ),
            map(parse_string, RString::from),
        )),
    )(input)
}

pub fn node_path(txt: &str) -> Res<&str, StrPath> {
    let (rest, res) = context(
        "node path",
        separated_pair(
            preceded(sp, node_name),
            preceded(sp, tag("->")),
            preceded(sp, node_name),
        ),
    )(txt)?;
    Ok((rest, StrPath::new(res.0, res.1)))
}

pub fn maybe_node_path(txt: &str) -> Res<&str, Option<StrPath>> {
    terminated(opt(node_path), sp)(txt)
}

pub fn parse_network(txt: &str) -> Res<&str, Vec<StrPath>> {
    context(
        "network",
        all_consuming(terminated(preceded(sp, many0(node_path)), sp)),
    )(txt)
}

pub fn parse_network_complete(txt: &str) -> Result<Vec<StrPath>, String> {
    let (_rest, val) = match parse_network(txt) {
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
    use rstest::rstest;

    #[rstest]
    #[case("A", "A", "")]
    #[case("name", "name", "")]
    #[case("\"name-is\"", "name-is", "")]
    #[case("name_is_nth # here", "name_is_nth", " # here")]
    #[case("\"name is\"", "name is", "")]
    fn node_name_test(#[case] txt: &str, #[case] name: &str, #[case] reminder: &str) {
        let (rest, n) = node_name(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(name, n.as_str());
    }

    #[rstest]
    #[case(" A ->B", ("A", "B"), "")]
    #[case(" \nname -> other", ("name", "other"), "")]
    #[case("\"name-is\" -> other_one", ("name-is", "other_one"), "")]
    #[case("\"name is\"\n->\nsomething", ("name is", "something"), "")]
    fn node_path_test(#[case] txt: &str, #[case] path: (&str, &str), #[case] reminder: &str) {
        let (rest, n) = node_path(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(
            n,
            StrPath::new(RString::from(path.0), RString::from(path.1))
        );
    }

    #[rstest]
    #[case("A ->B", Some(("A", "B")))]
    #[case("name -> other", Some(("name", "other")))]
    #[case("\"name-is\" -> other_one", Some(("name-is", "other_one")))]
    #[case("\"name is\"\n->\nsomething", Some(("name is", "something")))]
    #[case("\"name-is\" ", None)]
    #[case("->\nsomething", None)]
    fn maybe_node_path_test(#[case] txt: &str, #[case] path: Option<(&str, &str)>) {
        let (rest, n) = maybe_node_path(txt).unwrap();
        if path.is_some() {
            assert_eq!(rest, "");
        }
        assert_eq!(
            n,
            path.map(|path| StrPath::new(RString::from(path.0), RString::from(path.1)))
        );
    }

    #[rstest]
    #[case("A ->B", vec![("A", "B")])]
    #[case("name -> other", vec![("name", "other")])]
    #[case("\"name-is\" -> other_one", vec![("name-is", "other_one")])]
    #[case("# test \"name is not\"->something\n\n\"name is\"\n->\nsomething", vec![("name is", "something")])]
    #[case("\"name is\"\n->\nsomething# test \"name is not\"->something", vec![("name is", "something")])]
    #[case("\"name-is\" -> other_one\n#test -> other\n\"name is\"\n->\nsomething", vec![("name-is", "other_one"), ("name is", "something")])]
    fn parse_network_test(#[case] txt: &str, #[case] path: Vec<(&str, &str)>) {
        let (rest, n) = parse_network(txt).unwrap();
        assert_eq!(rest, "");
        assert_eq!(
            n,
            path.into_iter()
                .map(|path| StrPath::new(RString::from(path.0), RString::from(path.1)))
                .collect::<Vec<StrPath>>()
        );
    }
}
