use crate::attrs::{Date, DateTime, Time};
use crate::functions::KeyVal;
use crate::parser::string::parse_string;
use crate::parser::{identifier, sp, ws, Res};
use crate::{AttrMap, Attribute};
use abi_stable::std_types::RString;
use nom::bytes::complete::take_while1;
use nom::character::complete::{digit0, digit1};
use nom::character::is_digit;
use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take, take_while},
    character::complete::{alpha1, alphanumeric1, char, multispace0, one_of},
    combinator::{all_consuming, cut, map, map_res, opt, recognize, value},
    error::{context, convert_error, ParseError},
    multi::{many0, many0_count, many1, separated_list1},
    number,
    sequence::{delimited, pair, preceded, separated_pair, terminated, tuple},
};

fn attr_bool(txt: &str) -> Res<&str, Attribute> {
    context(
        "boolean",
        preceded(
            sp,
            alt((
                value(Attribute::Bool(true), tag("true")),
                value(Attribute::Bool(false), tag("false")),
            )),
        ),
    )(txt)
}

fn attr_int(txt: &str) -> Res<&str, Attribute> {
    let (rest, i) = context(
        "integer",
        preceded(
            sp,
            alt((
                recognize(tuple((
                    one_of("+-"),
                    cut(many1(terminated(digit1, many0(char('_'))))),
                ))),
                recognize(many1(terminated(digit1, many0(char('_'))))),
            )),
        ),
    )(txt)?;
    Ok((
        rest,
        Attribute::Integer(i.replace("_", "").parse().unwrap()),
    ))
}

pub fn parse_f64(txt: &str) -> Res<&str, f64> {
    preceded(
        sp,
        map(
            alt((
                recognize(tuple((
                    attr_int,
                    preceded(char('.'), cut(digit1)),
                    opt(tuple((one_of("eE"), attr_int))),
                ))),
                // even if there is no decimal 1e10 is float.
                recognize(tuple((
                    attr_int,
                    opt(preceded(char('.'), cut(digit1))),
                    tuple((one_of("eE"), attr_int)),
                ))),
            )),
            |v| v.replace("_", "").parse().unwrap(),
        ),
    )(txt)
}

fn attr_float(txt: &str) -> Res<&str, Attribute> {
    context("float", map(parse_f64, Attribute::Float))(txt)
}

fn attr_string(txt: &str) -> Res<&str, Attribute> {
    // because of escape sequences and other complicated
    // stuffs. parsing string was too complicated to do, so copied
    // from examples file in nom github
    let (rest, s) = context("string", preceded(sp, parse_string))(txt)?;
    Ok((rest, Attribute::String(s.into())))
}

pub fn digit_2(input: &str) -> Res<&str, u8> {
    map_res(take(2_u8), |s: &str| s.parse::<u8>())(input)
}

pub fn digit_4(input: &str) -> Res<&str, u16> {
    map_res(take(4_u8), |s: &str| s.parse::<u16>())(input)
}

fn parse_date(txt: &str) -> Res<&str, Date> {
    let (rest, (y, _, m, _, d)) = context(
        "date",
        preceded(sp, tuple((digit_4, tag("-"), digit_2, tag("-"), digit_2))),
    )(txt)?;
    Ok((rest, Date::new(y, m, d)))
}

fn parse_time(txt: &str) -> Res<&str, Time> {
    // TODO support nanoseconds
    let (rest, (h, _, m, s)) = context(
        "time",
        preceded(
            sp,
            tuple((digit_2, tag(":"), digit_2, opt(preceded(tag(":"), digit_2)))),
        ),
    )(txt)?;
    Ok((rest, Time::new(h, m, s.unwrap_or_default(), 0)))
}

fn parse_datetime(txt: &str) -> Res<&str, DateTime> {
    // TODO support offset
    let (rest, (d, _, t)) = preceded(sp, tuple((parse_date, one_of(" T"), parse_time)))(txt)?;
    Ok((rest, DateTime::new(d, t, None)))
}

fn attr_datetime(txt: &str) -> Res<&str, Attribute> {
    context(
        "datetime",
        preceded(
            sp,
            alt((
                // datetime should be before date, otherwise it'll take date and exit
                map(parse_datetime, Attribute::DateTime),
                map(parse_date, Attribute::Date),
                map(parse_time, Attribute::Time),
            )),
        ),
    )(txt)
}

fn attr_array(txt: &str) -> Res<&str, Attribute> {
    let (rest, lst) = context(
        "attributes array",
        preceded(
            sp,
            delimited(
                preceded(sp, tag("[")),
                opt(separated_list1(
                    preceded(sp, tag(",")),
                    preceded(sp, attr_any),
                )),
                preceded(sp, tag("]")),
            ),
        ),
    )(txt)?;
    Ok((rest, Attribute::Array(lst.unwrap_or_default().into())))
}

pub fn attr_key_val(txt: &str) -> Res<&str, (RString, Attribute)> {
    let (rest, lst) = context(
        "attributes key value pairs",
        separated_pair(
            preceded(
                sp,
                alt((
                    map(identifier, RString::from),
                    map(parse_string, RString::from),
                )),
            ),
            preceded(sp, tag("=")),
            preceded(sp, cut(attr_any)),
        ),
    )(txt)?;
    Ok((rest, lst))
}

fn attr_table(txt: &str) -> Res<&str, Attribute> {
    let (rest, lst) = context(
        "attributes table",
        delimited(
            preceded(sp, tag("{")),
            opt(separated_list1(preceded(sp, tag(",")), attr_key_val)),
            preceded(sp, tag("}")),
        ),
    )(txt)?;
    let tbl = lst
        .unwrap_or_default()
        .into_iter()
        .map(|(k, v)| (k, v))
        .collect();
    Ok((rest, Attribute::Table(tbl)))
}

pub fn attr_any(txt: &str) -> Res<&str, Attribute> {
    context(
        "attribute",
        alt((
            // float should come before int, otherwise it'll parse the
            // first part of float as int
            attr_bool,
            attr_datetime,
            attr_float,
            attr_int,
            attr_string,
            attr_array,
            attr_table,
        )),
    )(txt)
}

pub fn attr_group(txt: &str) -> Res<&str, Vec<(RString, Attribute)>> {
    terminated(preceded(sp, many0(attr_key_val)), sp)(txt)
}

fn group(txt: &str) -> Res<&str, Vec<&str>> {
    context(
        "attributes group",
        preceded(
            sp,
            delimited(tag("["), separated_list1(tag("."), identifier), tag("]")),
        ),
    )(txt)
}

pub fn attr_file(
    txt: &str,
) -> Res<
    &str,
    (
        Option<Vec<&str>>,
        Vec<(RString, Attribute)>,
        Vec<(Vec<&str>, Vec<(RString, Attribute)>)>,
    ),
> {
    // todo add error when the parsing did not consume the whole text
    context(
        "attributes file",
        all_consuming(preceded(
            sp,
            tuple((opt(group), attr_group, many0(tuple((group, attr_group))))),
        )),
    )(txt)
}

pub fn parse_attr_complete(
    txt: &str,
) -> Result<
    (
        Option<Vec<&str>>,
        Vec<(RString, Attribute)>,
        Vec<(Vec<&str>, Vec<(RString, Attribute)>)>,
    ),
    String,
> {
    let (rest, val) = match attr_file(txt) {
        Ok(v) => v,
        Err(e) => {
            let er = match e {
                nom::Err::Error(er) | nom::Err::Failure(er) => er,
                nom::Err::Incomplete(er) => panic!("shouldn't happen"),
            };
            return Err(convert_error(txt, er));
        }
    };
    Ok(val)
}

#[cfg(test)]
mod tests {
    use super::*;
    use abi_stable::std_types::RHashMap;
    use rstest::rstest;

    #[rstest]
    #[case("true", true, "")]
    #[case("false", false, "")]
    #[case(" false ", false, " ")]
    fn attr_bool_test(#[case] txt: &str, #[case] value: bool, #[case] reminder: &str) {
        let (rest, n) = attr_bool(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, Attribute::Bool(value));
    }

    #[rstest]
    #[case("12", 12, "")]
    #[case("-12", -12, "")]
    #[case(" 123_456", 123456, "")]
    #[case(" -123_456", -123456, "")]
    fn attr_int_test(#[case] txt: &str, #[case] value: i64, #[case] reminder: &str) {
        let (rest, n) = attr_int(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, Attribute::Integer(value));
    }

    #[rstest]
    #[case("1.2", 1.2, "")]
    #[case("-1.2", -1.2, "")]
    #[should_panic]
    #[case("1.-2", 1.0, "-2")]
    #[case("1.2e2", 1.2e2, "")]
    #[case("-1.2e2", -1.2e2, "")]
    #[should_panic]
    #[case("1.2e 2", 1.2e2, "")]
    #[should_panic]
    #[case("1.2 e2", 1.2e2, "")]
    #[case("+1.2e-2", 1.2e-2, "")]
    #[case("+2e3", 2e3, "")]
    #[case("+1.2e+2", 1.2e+2, "")]
    #[case(" 123_456.2", 123456.2, "")]
    fn attr_float_test(#[case] txt: &str, #[case] value: f64, #[case] reminder: &str) {
        let (rest, n) = attr_float(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, Attribute::Float(value));
    }

    #[rstest]
    #[case("1990-12-21", Date::new(1990, 12, 21), "")]
    fn parse_date_test(#[case] txt: &str, #[case] value: Date, #[case] reminder: &str) {
        let (rest, n) = parse_date(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case("12:21", Time::new(12, 21, 0, 0), "")]
    #[case("12:21:34", Time::new(12, 21, 34, 0), "")]
    fn parse_time_test(#[case] txt: &str, #[case] value: Time, #[case] reminder: &str) {
        let (rest, n) = parse_time(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case("true", Attribute::Bool(true), "")]
    #[case("false", Attribute::Bool(false), "")]
    #[should_panic] // no quote means not a string
    #[case("hello", Attribute::String(RString::from("hello")), "")]
    #[case("\"hello\"", Attribute::String(RString::from("hello")), "")]
    #[case(
        "\"hello\\\"there\"",
        Attribute::String(RString::from("hello\"there")),
        ""
    )]
    #[case("123", Attribute::Integer(123i64), "")]
    #[case("123_456", Attribute::Integer(123456i64), "")]
    #[case("-0.5", Attribute::Float(-0.5f64), "")]
    #[case("+0.5", Attribute::Float(0.5f64), "")]
    #[case("+0.5e1", Attribute::Float(0.5e1), "")]
    #[case("2022-01-01", Attribute::Date(Date::new(2022, 1, 1)), "")]
    #[case("14:30", Attribute::Time(Time::new(14, 30, 0, 0)), "")]
    #[case("14:30:43", Attribute::Time(Time::new(14, 30, 43, 0)), "")]
    #[case(
        "2022-01-01T14:30",
        Attribute::DateTime(DateTime::new(Date::new(2022, 1, 1), Time::new(14, 30, 0, 0), None)),
        ""
    )]
    #[case(
        "2022-01-01T14:30:45",
        Attribute::DateTime(DateTime::new(Date::new(2022, 1, 1), Time::new(14, 30, 45, 0), None)),
        ""
    )]
    #[case(
        "2022-01-01 14:30:45",
        Attribute::DateTime(DateTime::new(Date::new(2022, 1, 1), Time::new(14, 30, 45, 0), None)),
        ""
    )]
    #[case(
        "[true, \"hello\", 123]",
        Attribute::Array(vec![
            Attribute::Bool(true),
            Attribute::String(RString::from("hello")),
            Attribute::Integer(123i64),
        ].into()), "")]
    #[case(
        "{key= true, \"another_key\"= \"hello\"}",
        Attribute::Table({
	    let mut hm = RHashMap::new();
	    hm.insert("key".into(), Attribute::Bool(true));
	    hm.insert("another_key".into(), Attribute::String("hello".into()));
	    hm
	})
    , "")]
    fn attr_any_test(#[case] txt: &str, #[case] value: Attribute, #[case] reminder: &str) {
        let (rest, n) = attr_any(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case("x_name=12", ("x_name", Attribute::Integer(12)), "")]
    #[case("x_100yr_name=12.245", ("x_100yr_name", Attribute::Float(12.245)), "")]
    #[case("\"some flag\"=true", ("some flag", Attribute::Bool(true)), "")]
    #[case("x_name=12", ("x_name", Attribute::Integer(12)), "")]
    #[case("x=12:21", ("x", Attribute::Time(Time::new(12,21, 0,0))), "")]
    #[case("x=1998-12-21", ("x", Attribute::Date(Date::new(1998, 12,21))), "")]
    #[case("x=\"my {name}\"", ("x", Attribute::String("my {name}".into())), "")]
    fn attr_key_val_test(
        #[case] txt: &str,
        #[case] value: (&str, Attribute),
        #[case] reminder: &str,
    ) {
        let (rest, n) = attr_key_val(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, (RString::from(value.0), value.1));
    }

    #[rstest]
    #[case("x_name=12\nx_100yr_name=12.245", vec![("x_name", Attribute::Integer(12)), ("x_100yr_name", Attribute::Float(12.245))], "")]
    #[case("\"some flag\"=true\n # comment\nx_name=12", vec![("some flag", Attribute::Bool(true)), ("x_name", Attribute::Integer(12))], "")]
    #[case("# comment \n# comment 2 \n# comment 3 \n# comment 3\n\"some flag\"=true\nx_name=12", vec![("some flag", Attribute::Bool(true)), ("x_name", Attribute::Integer(12))], "")]
    #[case("\"some flag\"=true\n # comment # comment 2\nx_name=12", vec![("some flag", Attribute::Bool(true)), ("x_name", Attribute::Integer(12))], "")]
    #[case("x=12:21", vec![("x", Attribute::Time(Time::new(12,21, 0,0)))], "")]
    #[case("x=12:21", vec![("x", Attribute::Time(Time::new(12,21, 0,0)))], "")]
    fn attr_group_test(
        #[case] txt: &str,
        #[case] value: Vec<(&str, Attribute)>,
        #[case] reminder: &str,
    ) {
        let (rest, n) = attr_group(txt).unwrap();
        assert_eq!(rest, reminder);
        let values: Vec<(RString, Attribute)> = value
            .into_iter()
            .map(|(k, v)| (RString::from(k), v))
            .collect();
        assert_eq!(n, values);
    }
}
