use crate::attrs::{Attribute, HasAttributes};
use std::str::FromStr;

/// String template to use for NADI DSL
///
/// The template can contain variable parts inside `{}` with optional
/// format strings after `:`
#[derive(Clone, Debug)]
pub struct Template {
    pub parts: Vec<TemplatePart>,
    pub positions: Vec<usize>,
    pub original: String,
}

impl FromStr for Template {
    type Err = TemplateError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        parse_template(s)
    }
}

impl std::cmp::PartialEq for Template {
    fn eq(&self, other: &Self) -> bool {
        std::cmp::PartialEq::eq(&self.original, &other.original)
    }
}

impl Template {
    pub fn original(&self) -> &str {
        &self.original
    }

    pub fn has_variables(&self) -> bool {
        self.parts
            .iter()
            .any(|p| matches!(p, TemplatePart::Variable(..)))
    }

    pub fn lit(&self) -> Option<String> {
        self.parts
            .iter()
            .map(TemplatePart::lit)
            .collect::<Option<Vec<&str>>>()
            .map(|v| v.join(""))
    }
}

impl Template {
    /// Render the given template using the attribute values
    ///
    /// The attributes will be available to be used in the template
    /// based on the following rules:
    /// - [TODO] String attributes will be quoted, extra variable with `_`
    ///   prefix will be available to use unquoted string variables,
    /// - nested variables will be available using the `.` separator
    /// - all other variables will be available with their name,
    ///   their value will be their string representation.
    pub fn render<T: HasAttributes>(&self, attrmap: &T) -> Result<String, TemplateError> {
        let mut res = String::new();
        for (part, pos) in self.parts.iter().zip(&self.positions) {
            match part {
                TemplatePart::Literal(s) => res.push_str(s),
                TemplatePart::Variable(var) => match attrmap.attr_dot(&var.name) {
                    Ok(Some(v)) => {
                        res.push_str(&var.format.as_ref().map(|f| f.format(v)).unwrap_or(v.repr()))
                    }
                    Ok(None) if var.optional => {
                        res.push_str(&var.format.as_ref().map(|f| f.empty()).unwrap_or_default())
                    }
                    Ok(None) => {
                        return Err(TemplateError {
                            pos: *pos,
                            ty: TemplateErrorType::AttributeNotFound(var.name.to_string()),
                        });
                    }
                    Err(e) => {
                        return Err(TemplateError {
                            pos: *pos,
                            ty: TemplateErrorType::AttributeError(e),
                        });
                    }
                },
            }
        }
        Ok(res)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum TemplatePart {
    Literal(String),
    Variable(TemplateVar),
}

impl TemplatePart {
    pub fn lit(&self) -> Option<&str> {
        match self {
            Self::Literal(s) => Some(s.as_str()),
            Self::Variable(_) => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct TemplateVar {
    name: String,
    optional: bool,
    format: Option<VarFormat>,
}

#[derive(Default, Debug, Clone, PartialEq)]
pub struct VarFormat {
    len: Option<usize>,
    sig: Option<usize>,
    pad: Option<char>,
    align: Align,
    quote: Option<char>,
}

impl VarFormat {
    fn empty(&self) -> String {
        self.quote(self.align(String::new()))
    }

    fn quote(&self, s: String) -> String {
        if let Some(q) = self.quote {
            format!("{q}{}{q}", s.replace(q, &format!("\\{q}")))
        } else {
            s
        }
    }

    fn align(&self, s: String) -> String {
        if let Some(width) = self.len {
            if width <= s.len() {
                return s;
            }
            let pad_len = width - s.len();
            let pad_char = self.pad.unwrap_or(' ');
            let pad = pad_char.to_string().repeat(pad_len);
            match self.align {
                Align::Left => format!("{s}{pad}"),
                Align::Right => format!("{pad}{s}"),
                Align::Center => {
                    // Split padding evenly on both sides
                    let left = pad_len / 2;
                    let right = pad_len - left;
                    format!(
                        "{}{}{}",
                        pad_char.to_string().repeat(left),
                        s,
                        pad_char.to_string().repeat(right)
                    )
                }
            }
        } else {
            s
        }
    }

    fn format(&self, val: &Attribute) -> String {
        match val {
            Attribute::Bool(b) => self.quote(b.to_string()),
            Attribute::Integer(i) => {
                let num = if let Some(s) = &self.sig {
                    format!("{i}.{}", "0".repeat(*s))
                } else {
                    i.to_string()
                };
                self.quote(self.align(num))
            }
            Attribute::Float(f) => {
                let num = if let Some(s) = &self.sig {
                    format!("{f:.s$}")
                } else {
                    f.to_string()
                };
                self.quote(self.align(num))
            }
            Attribute::String(v) => self.quote(self.align(v.to_string())),
            Attribute::Date(v) => self.quote(self.align(v.to_string())),
            Attribute::Time(v) => self.quote(self.align(v.to_string())),
            Attribute::DateTime(v) => self.quote(self.align(v.to_string())),
            x => x.repr(),
        }
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
pub enum Align {
    Left,
    #[default]
    Right,
    Center,
}

#[derive(PartialEq, Debug, Clone)]
pub struct TemplateError {
    pub pos: usize,
    pub ty: TemplateErrorType,
}

impl std::error::Error for TemplateError {}

impl std::fmt::Display for TemplateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TemplateError at pos {}: {}", self.pos, self.ty)
    }
}

#[derive(PartialEq, Clone, Debug)]
pub enum TemplateErrorType {
    Incomplete,
    InvalidChar(char),
    AttributeNotFound(String),
    AttributeError(String),
}

impl std::fmt::Display for TemplateErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Incomplete => write!(f, "Incomplete Template"),
            Self::InvalidChar(c) => write!(f, "Invalid Character '{}'", c),
            Self::AttributeNotFound(s) => write!(f, "Attribute {:?} not found", s),
            Self::AttributeError(s) => write!(f, "Attribute Error: {}", s),
        }
    }
}

#[derive(PartialEq)]
enum ParseState {
    Literal,
    Variable,
    Format,
}

fn parse_template(val: &str) -> Result<Template, TemplateError> {
    let mut state = ParseState::Literal;
    let mut chars = val.chars().peekable();
    let mut data = String::new();
    let mut parts = Vec::new();
    let mut positions = Vec::new();
    let mut pos: usize = 0;
    let mut lastpos: usize = 0;
    let mut fmt = None;
    loop {
        let c = match chars.next() {
            Some(c) => c,
            None => break,
        };
        pos += 1;
        match c {
            '\\' => {
                if let Some(x) = chars.next() {
                    data.push(x);
                } else {
                    return Err(TemplateError {
                        pos,
                        ty: TemplateErrorType::Incomplete,
                    });
                }
            }
            '{' => match state {
                ParseState::Literal => {
                    if !data.is_empty() {
                        parts.push(TemplatePart::Literal(data.clone()));
                        positions.push(lastpos);
                    }
                    data.clear();
                    state = ParseState::Variable;
                    lastpos = pos;
                }
                ParseState::Variable | ParseState::Format => {
                    return Err(TemplateError {
                        pos,
                        ty: TemplateErrorType::InvalidChar(c),
                    });
                }
            },
            '}' => match state {
                ParseState::Variable | ParseState::Format => {
                    if !data.is_empty() {
                        let (name, opt) = if let Some(data) = data.strip_suffix("?") {
                            (data.to_string(), true)
                        } else {
                            (data.clone(), false)
                        };
                        parts.push(TemplatePart::Variable(TemplateVar {
                            name,
                            optional: opt,
                            format: fmt.take(),
                        }));
                        positions.push(lastpos);
                    }
                    data.clear();
                    state = ParseState::Literal;
                    lastpos = pos;
                }
                ParseState::Literal => {
                    return Err(TemplateError {
                        pos,
                        ty: TemplateErrorType::InvalidChar(c),
                    });
                }
            },
            ':' if state == ParseState::Variable => {
                fmt = Some(VarFormat::default());
                state = ParseState::Format;
            }
            _ => {
                if matches!(state, ParseState::Format) {
                    if let Some(f) = &mut fmt {
                        match c {
                            '"' | '\'' => f.quote = Some(c),
                            '<' => f.align = Align::Left,
                            '>' => f.align = Align::Right,
                            '^' => f.align = Align::Center,
                            '.' => {
                                let mut w = String::new();
                                while let Some(d) = chars.peek() {
                                    if d.is_ascii_digit() {
                                        w.push(*d);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if let Ok(n) = w.parse::<usize>() {
                                    f.sig = Some(n);
                                }
                            }
                            c if c.is_ascii_digit() => {
                                // width – read all following digits
                                let mut w = String::new();
                                w.push(c);
                                while let Some(d) = chars.peek() {
                                    if d.is_ascii_digit() {
                                        w.push(*d);
                                        chars.next();
                                    } else {
                                        break;
                                    }
                                }
                                if let Ok(n) = w.parse::<usize>() {
                                    f.len = Some(n);
                                }
                            }
                            _ => {
                                return Err(TemplateError {
                                    pos,
                                    ty: TemplateErrorType::InvalidChar(c),
                                });
                            }
                        }
                    }
                } else {
                    data.push(c)
                }
            }
        }
    }
    match state {
        // if we're still reading variable it can't end
        ParseState::Literal => {
            if !data.is_empty() {
                parts.push(TemplatePart::Literal(data.clone()));
                positions.push(lastpos);
            }
        }
        ParseState::Variable | ParseState::Format => {
            return Err(TemplateError {
                pos,
                ty: TemplateErrorType::Incomplete,
            });
        }
    }
    Ok(Template {
        parts,
        positions,
        original: val.to_string(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prelude::AttrMap;
    use nadi_core::attr_map;
    use rstest::rstest;
    use std::str::FromStr;

    #[rstest]
    #[case("")] // empty string
    #[case("hello")] // plain text
    #[case("{foo}")] // simple placeholder
    #[case("{foo:0.2}")] // placeholder with numeric format
    #[case("{foo:>10}")] // placeholder with alignment
    // #[case("{foo:?}")] // placeholder with optional render
    #[case("{foo:01}")] // placeholder with zero‑padding
    #[case("{foo:>10.3}")] // complex format
    #[case("{foo}bar{baz}")] // multiple placeholders
    #[case("{foo}  {bar}")] // placeholders separated by spaces
    #[case("{foo}  ")] // trailing spaces
    #[case("{foo} {bar} ")] // leading/trailing spaces
    #[case("{foo}{bar}")] // back‑to‑back placeholders
    #[case("{foo}{bar} baz")] // placeholder followed by text
    #[case("{foo?}baz{bar}")] // text between placeholders
    #[case("{foo}{bar}{baz}")] // three placeholders
    #[case("{foo:}")] // placeholder with an *empty* format
    #[case("{foo?:0}")] // placeholder with zero format
    // #[case("{foo: >10}")] // alignment with space
    #[case("{foo:>10}")] // alignment to the right
    #[case("{foo?:<10}")] // alignment to the left
    #[case("{foo:0.3}")] // float format
    #[case("{foo:06}")] // zero padding
    #[case("{foo:5}")] // minimal width
    #[case("{foo:<5.2}")] // left‑aligned, precision
    #[case("{foo:>5.2}")] // right‑aligned, precision
    fn is_valid_template(#[case] templ: &str) {
        Template::from_str(templ).unwrap();
    }

    #[rstest]
    #[case("{foo")] // missing closing brace
    #[case("foo}")] // missing opening brace
    #[case("{{foo}")] // unclosed nested brace
    #[case("{foo}{bar")] // missing closing brace
    #[case("{foo:}bar}")] // stray closing brace
    #[case("{foo:}bar}}")] // double stray brace
    #[case("{foo::}")] // double colon
    #[case("{foo:0x:}")] // two format specifiers
    #[case("{foo:??}")] // unknown format specifier
    #[case("{foo::}")] // colon with nothing after
    #[case("{foo}bar{")] // trailing open brace
    #[case("{foo}bar}")] // stray closing brace at end
    #[case("{foo}bar{baz}}")] // mismatched brackets
    #[case("{foo}{bar}baz{")] // unmatched opening brace
    #[case("{{{")] // too many opening braces
    #[case("}}}")] // too many closing braces
    #[case("{foo}{{bar}")] // escape attempt but wrong
    fn test_invalid_template(#[case] templ: &str) {
        assert!(Template::from_str(templ).is_err());
    }

    #[rstest]
    #[case("", attr_map!(), "")]
    #[case("\\{", attr_map!(), "{")]
    #[case("\\}", attr_map!(), "}")]
    #[case("something", attr_map!(), "something")]
    #[case("{sething}", attr_map!(sething => 0), "0")]
    #[case("som{eng}", attr_map!(eng => 21), "som21")]
    #[case("SOME{THING}", attr_map!(THING => "what"), "SOMEwhat")]
    #[case("{So:0.2}Me", attr_map!(So => 1.2343), "1.23Me")]
    #[case("{someng:}", attr_map!(someng => true), "true")]
    #[case(" {SoMe} ", attr_map!(SoMe => 1), " 1 ")]
    #[case("{foo}",  attr_map!(foo => "bar"), "bar")]
    #[case("{foo}",  attr_map!(foo => 42),   "42")]
    #[case("{foo}",  attr_map!(foo => 3.14), "3.14")]
    #[case("{foo}",  attr_map!(foo => true), "true")]
    #[case("{foo}",  attr_map!(foo => vec![1,2,3]), "[1, 2, 3]")]
    #[case("{foo:0.2}", attr_map!(foo => 3.1415), "3.14")]
    #[case("{foo:0.1}", attr_map!(foo => 3.1415), "3.1")]
    #[case("{foo:0.0}", attr_map!(foo => 3.1415), "3")]
    #[case("{foo:>10}", attr_map!(foo => "hi"), "        hi")]
    #[case("{foo:<10}", attr_map!(foo => "hi"), "hi        ")]
    // #[case("{foo:^10}", attr_map!(foo => "hi"), "    hi    ")]
    // #[case("{foo:0?}", attr_map!(foo => "test"), "\"test\"")]
    #[case("{foo:1.2}", attr_map!(foo => 1.2345), "1.23")]
    #[case("{foo:0.2}", attr_map!(foo => 1), "1.00")]
    // Zero pad not implemented
    // #[case("{foo:02}", attr_map!(foo => 7), "07")]
    // #[case("{foo:02}", attr_map!(foo => 42), "42")]
    #[case("{foo:>5}", attr_map!(foo => 3), "    3")]
    #[case("{foo:>5.2}", attr_map!(foo => 3.1), " 3.10")]
    #[case("{foo:>5.2}", attr_map!(foo => 3.1415), " 3.14")]
    #[should_panic]
    #[case("{foo}", attr_map!(), "???")] // placeholder missing – expected placeholder‑token
    #[should_panic]
    #[case("{foo:0.2}", attr_map!(), "???")] // missing placeholder with format
    #[case("{foo}{bar}", attr_map!(foo => "a", bar => "b"), "ab")]
    #[should_panic]
    #[case("{foo}{bar}", attr_map!(foo => "a"), "a???")] // bar missing
    #[should_panic]
    #[case("{foo}{bar}", attr_map!(bar => "b"), "???b")] // foo missing
    #[case("{foo}{bar}{baz}", attr_map!(foo => "x", bar => "y", baz => "z"), "xyz")]
    #[should_panic]
    #[case("{foo}{bar}{baz}", attr_map!(foo => "x", bar => "y"), "xy???")]
    #[case("{foo}  {bar}", attr_map!(foo => "x", bar => "y"), "x  y")]
    #[case("{foo}{bar} ", attr_map!(foo => "x", bar => "y"), "xy ")]
    #[case("{foo:0.2} {bar:0.2}", attr_map!(foo => 1.2345, bar => 2.3456), "1.23 2.35")]
    #[case("{foo:>10}bar{bar}", attr_map!(foo => "hi", bar => "z"), "        hibarz")]
    #[case("\\{{foo}\\}", attr_map!(foo => "baz"), "{baz}")] // literal opening brace
    #[case("{foo}\\{bar\\}", attr_map!(foo => "x", bar => "y"), "x{bar}")] // test nested escape? (implementation dependent)
    fn render_template(#[case] templ: &str, #[case] values: AttrMap, #[case] res: String) {
        let templ = Template::from_str(templ).unwrap();
        let rend = templ.render(&values).unwrap();
        assert_eq!(rend, res);
    }

    #[rstest]
    #[case("\\{{foo?}\\}", attr_map!(foo => "baz"), "{baz}")]
    #[case("\\{{foo?}\\}", attr_map!(), "{}")]
    #[case("{foo?}", attr_map!(bar => "baz"), "")]
    #[case("{foo?}\\{bar\\}", attr_map!(foo => "x", bar => "y"), "x{bar}")]
    fn render_optional_template(#[case] templ: &str, #[case] values: AttrMap, #[case] res: String) {
        let templ = Template::from_str(templ).unwrap();
        let rend = templ.render(&values).unwrap();
        assert_eq!(rend, res);
    }
}
