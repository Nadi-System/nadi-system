use crate::parser::{sp, Res};
use crate::table::{Column, ColumnAlign};

use nom::bytes::complete::{take_till, take_until};

use nom::error::convert_error;
use nom::multi::many0;
use nom::{
    branch::alt,
    bytes::complete::tag,
    combinator::{all_consuming, map, opt, value},
    error::context,
    sequence::{delimited, preceded, separated_pair, terminated, tuple},
};

use super::attrs::parse_f64;

fn column_align(txt: &str) -> Res<&str, ColumnAlign> {
    preceded(
        sp,
        alt((
            value(ColumnAlign::Left, tag("<")),
            value(ColumnAlign::Center, tag("^")),
            value(ColumnAlign::Right, tag(">")),
        )),
    )(txt)
}

fn column(txt: &str) -> Res<&str, Column> {
    let (rest, (align, (head, templ))) = context(
        "column definition",
        preceded(
            sp,
            tuple((
                opt(column_align),
                preceded(
                    sp,
                    separated_pair(
                        map(take_until("=>"), str::trim),
                        tag("=>"),
                        map(delimited(sp, take_till(|c| c == '\n'), sp), str::trim),
                    ),
                ),
            )),
        ),
    )(txt)?;
    Ok((rest, Column::new(head, templ, align)))
}

pub fn parse_table(txt: &str) -> Res<&str, Vec<Column>> {
    context(
        "table file",
        all_consuming(terminated(preceded(sp, many0(column)), sp)),
    )(txt)
}

pub fn parse_table_complete(txt: &str) -> Result<Vec<Column>, String> {
    // let's add the final line end as the file are likely to miss them
    let (_rest, val) = match parse_table(&format!("{}\n", txt)) {
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
    #[case("box", NodeShape::Square, "")]
    #[case("square", NodeShape::Square, "")]
    #[case("circle", NodeShape::Circle, "")]
    #[case("triangle", NodeShape::Triangle, "")]
    #[case("rectangle", NodeShape::Rectangle(1.5), "")]
    #[case("ellipse", NodeShape::Ellipse(1.5), "")]
    #[case("rectangle(0.5)", NodeShape::Rectangle(0.5), "")]
    #[case("ellipse(2.0)", NodeShape::Ellipse(2.0), "")]
    fn node_shape_test(#[case] txt: &str, #[case] value: NodeShape, #[case] reminder: &str) {
        let (rest, n) = node_shape(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case(
        "field=> test {here}",
        Column::new("field", "test {here}", Some(ColumnAlign::Center)),
        ""
    )]
    #[case(
        "<Field 1 =>{here} is {more_test?\"default\"} 2.4",
        Column::new(
            "Field 1",
            "{here} is {more_test?\"default\"} 2.4",
            Some(ColumnAlign::Left)
        ),
        ""
    )]
    #[case(
        "#new field\n < field => test {here}\n",
        Column::new("field", "test {here}", Some(ColumnAlign::Left)),
        ""
    )]
    fn column_test(#[case] txt: &str, #[case] value: Column, #[case] reminder: &str) {
        let (rest, n) = column(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }

    #[rstest]
    #[case(
        "field=> test {here}",
        vec![Column::new("field", "test {here}", Some(ColumnAlign::Center))],
        ""
    )]
    #[case(
        "<Field 1 =>{here} is {more_test?\"default\"} 2.4",
        vec![Column::new(
            "Field 1",
            "{here} is {more_test?\"default\"} 2.4",
            Some(ColumnAlign::Left)
        )],
        ""
    )]
    #[case(
        "#new field\n < field => test {here}\n",
        vec![Column::new("field", "test {here}", Some(ColumnAlign::Left))],
        ""
    )]
    #[case(
        "#new field\n < field => test {here}\n# an co\n\n<Field 1 =>{here} is {more_test?\"default\"} 2.4",
        vec![Column::new("field", "test {here}", Some(ColumnAlign::Left)),
	     Column::new(
		 "Field 1",
		 "{here} is {more_test?\"default\"} 2.4",
		 Some(ColumnAlign::Left)
             )],
        ""
    )]
    fn parse_table_test(#[case] txt: &str, #[case] value: Vec<Column>, #[case] reminder: &str) {
        let (rest, n) = parse_table(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }
}
