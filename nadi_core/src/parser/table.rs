use crate::parser::{sp, Res};
use crate::table::{Column, ColumnAlign, NodeShape};

use nom::bytes::complete::take_till;

use nom::{
    branch::alt,
    bytes::complete::{is_not, tag},
    combinator::{opt, value},
    sequence::{delimited, preceded, separated_pair, tuple},
};

use super::attrs::parse_f64;

fn node_shape_val(txt: &str) -> Res<&str, NodeShape> {
    let (rest, (shape, val)) = preceded(
        sp,
        tuple((
            // rectangle needs to be before rect, otherwise it'll
            // exist after matching rect
            alt((tag("rectangle"), tag("rect"), tag("ellipse"))),
            opt(delimited(tag("("), parse_f64, tag(")"))),
        )),
    )(txt)?;
    let val = val.unwrap_or(1.5);
    Ok((
        rest,
        match shape {
            "rect" | "rectangle" => NodeShape::Rectangle(val),
            "ellipse" => NodeShape::Ellipse(val),
            _ => panic!("should only match rectangle and ellipse"),
        },
    ))
}

fn node_shape(txt: &str) -> Res<&str, NodeShape> {
    preceded(
        sp,
        alt((
            value(NodeShape::Square, alt((tag("square"), tag("box")))),
            value(NodeShape::Circle, tag("circle")),
            value(NodeShape::Triangle, tag("triangle")),
            node_shape_val,
        )),
    )(txt)
}

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
    let (rest, (align, (head, templ))) = preceded(
        sp,
        tuple((
            opt(column_align),
            preceded(
                sp,
                separated_pair(
                    is_not(":"),
                    tag(":"),
                    delimited(sp, take_till(|c| c == '\n'), sp),
                ),
            ),
        )),
    )(txt)?;
    Ok((rest, Column::new(head, templ, align)))
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
        "field: test {here}",
        Column::new("field", "test {here}", Some(ColumnAlign::Center)),
        ""
    )]
    #[case(
        "<Field 1: {here} is {more_test?\"default\"} 2.4",
        Column::new(
            "Field 1",
            "{here} is {more_test?\"default\"} 2.4",
            Some(ColumnAlign::Left)
        ),
        ""
    )]
    #[case(
        "#new field\n < field: test {here}\n",
        Column::new("field", "test {here}", Some(ColumnAlign::Left)),
        ""
    )]
    fn column_test(#[case] txt: &str, #[case] value: Column, #[case] reminder: &str) {
        let (rest, n) = column(txt).unwrap();
        assert_eq!(rest, reminder);
        assert_eq!(n, value);
    }
}
