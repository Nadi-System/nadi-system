use nom::{
    branch::alt,
    bytes::complete::{is_not, tag, take_while},
    character::complete::{alpha1, alphanumeric1},
    combinator::{opt, recognize, value},
    error::{context, ParseError, VerboseError},
    multi::{many0_count, many1},
    sequence::{delimited, pair, preceded},
    IResult,
};

pub mod attrs;
pub mod functions;
pub mod network;
pub mod string;
pub mod table;

pub(crate) type Res<T, U> = IResult<T, U, VerboseError<T>>;

fn ws<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (), E> {
    let chars = " \n\t\r";
    value((), take_while(move |c| chars.contains(c)))(i)
}

fn eol<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (), E> {
    // only unix, mac and windows line end supported for now
    value((), alt((tag("\n\r"), tag("\r\n"), tag("\n"))))(i)
}

fn comment<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (), E> {
    value(
        (), // Output is thrown away.
        many1(preceded(ws, delimited(tag("#"), is_not("\n\r"), opt(eol)))),
    )(i)
}

fn sp<'a, E: ParseError<&'a str>>(i: &'a str) -> IResult<&'a str, (), E> {
    value(
        (), // Output is thrown away.
        alt((comment, ws)),
    )(i)
}

fn identifier(input: &str) -> Res<&str, &str> {
    context(
        "identifier",
        preceded(
            sp,
            recognize(pair(
                alt((alpha1, tag("_"))),
                many0_count(alt((alphanumeric1, tag("_")))),
            )),
        ),
    )(input)
}

pub use functions::parse_script;
pub use network::{node_path, parse_network};
