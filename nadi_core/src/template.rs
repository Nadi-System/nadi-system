use crate::attrs::HasAttributes;
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
            .any(|p| matches!(p, TemplatePart::Variable(_)))
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
                TemplatePart::Variable(s) => match attrmap.attr_dot(s) {
                    Ok(Some(v)) => res.push_str(&v.repr()),
                    Ok(None) => {
                        return Err(TemplateError {
                            pos: *pos,
                            ty: TemplateErrorType::AttributeNotFound(s.to_string()),
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
    Variable(String),
}

impl TemplatePart {
    pub fn lit(&self) -> Option<&str> {
        match self {
            Self::Literal(s) => Some(s.as_str()),
            Self::Variable(_) => None,
        }
    }
}

// struct VarFormat {
//     len: usize,
//     sig: usize,
//     pad: usize,
//     align: Align,
// }

// enum Align {
//     Left,
//     Right,
//     // Center,
// }

enum State {
    Literal,
    Variable,
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

fn parse_template(val: &str) -> Result<Template, TemplateError> {
    let mut state = State::Literal;
    let mut chars = val.chars();
    let mut data = String::new();
    let mut parts = Vec::new();
    let mut positions = Vec::new();
    let mut pos: usize = 0;
    let mut lastpos: usize = 0;
    loop {
        let c = match chars.next() {
            Some(c) => c,
            None => break,
        };
        pos += 1;
        match c {
            '\\' => {
                _ = chars.next();
            }
            '{' => match state {
                State::Literal => {
                    if !data.is_empty() {
                        parts.push(TemplatePart::Literal(data.clone()));
                        positions.push(lastpos);
                    }
                    data.clear();
                    state = State::Variable;
                    lastpos = pos;
                }
                State::Variable => {
                    return Err(TemplateError {
                        pos,
                        ty: TemplateErrorType::InvalidChar(c),
                    });
                }
            },
            '}' => match state {
                State::Variable => {
                    if !data.is_empty() {
                        parts.push(TemplatePart::Variable(data.clone()));
                        positions.push(lastpos);
                    }
                    data.clear();
                    state = State::Literal;
                    lastpos = pos;
                }
                State::Literal => {
                    return Err(TemplateError {
                        pos,
                        ty: TemplateErrorType::InvalidChar(c),
                    });
                }
            },
            // ':' => ,
            _ => data.push(c),
        }
    }
    if !data.is_empty() {
        match state {
            // if we're still reading variable it can't end
            State::Literal => {
                parts.push(TemplatePart::Literal(data.clone()));
                positions.push(lastpos);
            }
            State::Variable => {
                return Err(TemplateError {
                    pos,
                    ty: TemplateErrorType::Incomplete,
                });
            }
        }
    }
    Ok(Template {
        parts,
        positions,
        original: val.to_string(),
    })
}
