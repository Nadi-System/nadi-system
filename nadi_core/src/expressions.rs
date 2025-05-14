use crate::attrs::{Attribute, FromAttribute, HasAttributes};
use crate::node::Node;
use crate::tasks::{FunctionType, TaskContext, TaskKeyword};
use std::collections::HashMap;

#[derive(Debug)]
pub enum EvalError {
    UnresolvedVariable,
    AttributeNotFound,
    NoOutputNode,
    AttributeError(String),
    InvalidOperation,
    InvalidVariableType,
    NotANumber,
    NotABool,
    DivideByZero,
    LogicalError,
}

impl EvalError {
    pub fn message(&self) -> String {
        match self {
            Self::UnresolvedVariable => "Unresolved variable in expression",
            Self::AttributeNotFound => "Attribute not found",
            Self::NoOutputNode => "Node doesn't have a output node",
            Self::AttributeError(s) => return format!("Attribute Error: {s}"),
            Self::InvalidOperation => "Operation not Allowed",
            Self::InvalidVariableType => "Variable type invalid in this context",
            Self::NotANumber => "Numerical Operation on Non Number",
            Self::NotABool => "Boolean Operation on Non Boolean",
            Self::DivideByZero => "Division by Zero",
            Self::LogicalError => "Logical Error inside the program, contact dev",
        }
        .to_string()
    }
}

impl std::error::Error for EvalError {}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "EvalError: {}", self.message())
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    Literal(Attribute),
    Variable(InputVar),
    Function(FunctionCall),
    UniOp(UniOperator, Box<Expression>),
    BiOp(BiOperator, Box<Expression>, Box<Expression>),
}

impl ToString for Expression {
    fn to_string(&self) -> String {
        match self {
            Self::Literal(a) => a.to_string(),
            Self::Variable(v) => v.to_string(),
            Self::Function(fc) => fc.to_string(),
            Self::UniOp(op, expr) => {
                if expr.nested() {
                    format!("{} ({})", op.to_string(), expr.to_string())
                } else {
                    format!("{} {}", op.to_string(), expr.to_string())
                }
            }
            Self::BiOp(op, expr1, expr2) => format!(
                "{} {} {}",
                if expr1.nested() {
                    format!("({})", expr1.to_string())
                } else {
                    format!("{}", expr1.to_string())
                },
                op.to_string(),
                if expr2.nested() {
                    format!("({})", expr2.to_string())
                } else {
                    format!("{}", expr2.to_string())
                },
            ),
        }
    }
}

impl Expression {
    pub fn nested(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::Variable(_) => false,
            Self::Function(_) => false,
            Self::UniOp(_, _) => true,
            Self::BiOp(_, _, _) => true,
        }
    }

    pub fn has_variables(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::Variable(_) => true,
            Self::Function(fc) => {
                fc.args.iter().any(|e| e.has_variables())
                    || fc.kwargs.iter().any(|e| e.1.has_variables())
            }
            Self::UniOp(_, e) => e.has_variables(),
            Self::BiOp(_, e1, e2) => e1.has_variables() || e2.has_variables(),
        }
    }

    pub fn resolve(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Expression, EvalError> {
        match self {
            Self::Literal(v) => Ok(self.clone()),
            Self::Variable(vt) => {
                let attr = match &vt.ty {
                    Some(ty) => match ty {
                        VarType::Env => ctx.env.attr_nested(&vt.names).map(|a| a.cloned()),
                        VarType::Network => ctx.network.attr_nested(&vt.names).map(|a| a.cloned()),
                        VarType::Node => match node {
                            Some(n) => n.lock().attr_nested(&vt.names).map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError,
                                    _ => EvalError::InvalidVariableType,
                                })
                            }
                        },
                        VarType::Inputs => match node {
                            Some(n) => {
                                if vt.check {
                                    let res = n.lock().inputs().iter().all(|i| {
                                        if let Ok(Some(_)) = i.lock().attr_nested(&vt.names) {
                                            true
                                        } else {
                                            false
                                        }
                                    });
                                    return Ok(Self::Literal(Attribute::Bool(res)));
                                } else {
                                    let mut vars = Vec::new();
                                    for i in n.lock().inputs() {
                                        let a = i.lock().attr_nested(&vt.names).map(|a| a.cloned());
                                        vars.push(
                                            a.map_err(EvalError::AttributeError)?
                                                .ok_or(EvalError::AttributeNotFound)?,
                                        );
                                    }
                                    return Ok(Self::Literal(Attribute::Array(vars.into())));
                                }
                            }
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError,
                                    _ => EvalError::InvalidVariableType,
                                })
                            }
                        },
                        VarType::Output => match node {
                            Some(n) => n
                                .lock()
                                .output()
                                .into_option()
                                .ok_or(EvalError::NoOutputNode)?
                                .lock()
                                .attr_nested(&vt.names)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError,
                                    _ => EvalError::InvalidVariableType,
                                })
                            }
                        },
                    },
                    None => match ft {
                        FunctionType::Env => ctx.env.attr_nested(&vt.names).map(|a| a.cloned()),
                        FunctionType::Network => {
                            ctx.network.attr_nested(&vt.names).map(|a| a.cloned())
                        }
                        FunctionType::Node => match node {
                            Some(n) => n.lock().attr_nested(&vt.names).map(|a| a.cloned()),
                            None => return Err(EvalError::LogicalError),
                        },
                    },
                };
                if vt.check {
                    if let Ok(Some(_)) = attr {
                        Ok(Self::Literal(true.into()))
                    } else {
                        Ok(Self::Literal(false.into()))
                    }
                } else {
                    attr.map_err(EvalError::AttributeError)?
                        .ok_or(EvalError::AttributeNotFound)
                        .map(Self::Literal)
                }
            }
            Self::Function(fc) => {
                let mut args = Vec::with_capacity(fc.args.len());
                for a in &fc.args {
                    args.push(a.resolve(ft, ctx, node)?);
                }
                let mut kwargs = HashMap::with_capacity(fc.kwargs.len());
                for (k, a) in &fc.kwargs {
                    kwargs.insert(k.clone(), a.resolve(ft, ctx, node)?);
                }
                Ok(Self::Function(FunctionCall {
                    name: fc.name.clone(),
                    args,
                    kwargs,
                }))
            }
            Self::UniOp(op, expr) => Ok(Self::UniOp(
                op.clone(),
                Box::new(expr.resolve(ft, ctx, node)?),
            )),
            Self::BiOp(op, expr1, expr2) => Ok(Self::BiOp(
                op.clone(),
                Box::new(expr1.resolve(ft, ctx, node)?),
                Box::new(expr2.resolve(ft, ctx, node)?),
            )),
        }
    }

    pub fn eval(&self, ft: &FunctionType, ctx: &TaskContext) -> Result<Attribute, EvalError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Variable(_) => Err(EvalError::UnresolvedVariable),
            Self::Function(fc) => fc.eval(ft, ctx),
            Self::UniOp(op, expr) => op.eval(expr.eval(ft, ctx)?),
            Self::BiOp(op, expr1, expr2) => op.eval(expr1.eval(ft, ctx)?, expr2.eval(ft, ctx)?),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UniOperator {
    Not,
    Negative,
}

impl UniOperator {
    pub fn eval(&self, value: Attribute) -> Result<Attribute, EvalError> {
        match self {
            Self::Not => !value,
            Self::Negative => -value,
        }
    }
}
impl ToString for UniOperator {
    fn to_string(&self) -> String {
        match self {
            Self::Not => "!",
            Self::Negative => "-",
        }
        .to_string()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum BiOperator {
    Add,
    Substract,
    Multiply,
    Divide,
    Modulus,
    Equal,
    LessThan,
    GreaterThan,
    LessThanEqual,
    GreaterThanEqual,
    And,
    Or,
}

impl BiOperator {
    pub fn eval(&self, val1: Attribute, val2: Attribute) -> Result<Attribute, EvalError> {
        match self {
            Self::Add => val1 + val2,
            Self::Substract => val1 - val2,
            Self::Multiply => val1 * val2,
            Self::Divide => val1 / val2,
            Self::Modulus => val1 % val2,
            Self::Equal => Ok(Attribute::Bool(val1 == val2)),
            Self::LessThan => Ok(Attribute::Bool(val1 < val2)),
            Self::GreaterThan => Ok(Attribute::Bool(val1 > val2)),
            Self::LessThanEqual => Ok(Attribute::Bool(val1 <= val2)),
            Self::GreaterThanEqual => Ok(Attribute::Bool(val1 >= val2)),
            Self::And => val1 & val2,
            Self::Or => val1 | val2,
        }
    }
}

impl ToString for BiOperator {
    fn to_string(&self) -> String {
        match self {
            Self::Add => "+",
            Self::Substract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::Modulus => "%",
            Self::Equal => "==",
            Self::LessThan => "<",
            Self::GreaterThan => ">",
            Self::LessThanEqual => "<=",
            Self::GreaterThanEqual => ">=",
            Self::And => "&",
            Self::Or => "|",
        }
        .to_string()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct InputVar {
    pub ty: Option<VarType>,
    pub names: Vec<String>,
    pub check: bool,
}

impl ToString for InputVar {
    fn to_string(&self) -> String {
        format!(
            "{}{}{}",
            self.ty
                .as_ref()
                .map(|t| format!("{}.", t.to_string()))
                .unwrap_or_default(),
            self.names.join("."),
            self.check.then(|| "?").unwrap_or_default(),
        )
    }
}

impl InputVar {
    pub fn new(ty: Option<VarType>, names: Vec<String>, check: bool) -> Self {
        Self { ty, names, check }
    }
}

#[derive(PartialEq, Debug, Clone)]
pub enum VarType {
    Env,
    Node,
    Network,
    Inputs,
    Output,
}

impl VarType {
    pub fn from_keyword(kw: &TaskKeyword) -> Option<Self> {
        match kw {
            TaskKeyword::Node => Some(VarType::Node),
            TaskKeyword::Network => Some(VarType::Network),
            TaskKeyword::Env => Some(VarType::Env),
            TaskKeyword::Inputs => Some(VarType::Inputs),
            TaskKeyword::Output => Some(VarType::Output),
            _ => None,
        }
    }
}

impl ToString for VarType {
    fn to_string(&self) -> String {
        match self {
            VarType::Node => "node",
            VarType::Network => "network",
            VarType::Env => "env",
            VarType::Inputs => "inputs",
            VarType::Output => "output",
        }
        .to_string()
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<Expression>,
    pub kwargs: HashMap<String, Expression>,
}

impl ToString for FunctionCall {
    fn to_string(&self) -> String {
        let args = self
            .args
            .iter()
            .map(|a| a.to_string())
            .collect::<Vec<String>>()
            .join(", ");
        let kwargs = self
            .kwargs
            .iter()
            .map(|a| format!("{} = {}", a.0, a.1.to_string()))
            .collect::<Vec<String>>()
            .join(", ");
        let middle = if args.is_empty() || kwargs.is_empty() {
            ""
        } else {
            ", "
        };
        format!("{}({}{}{})", self.name, args, middle, kwargs)
    }
}

impl FunctionCall {
    pub fn new(name: String, args: Vec<Expression>, kwargs: HashMap<String, Expression>) -> Self {
        Self { name, args, kwargs }
    }

    pub fn eval(&self, ft: &FunctionType, ctx: &TaskContext) -> Result<Attribute, EvalError> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokenizer::get_tokens;
    use rstest::rstest;
}
