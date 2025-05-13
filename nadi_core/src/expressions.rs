use crate::attrs::Attribute;
use crate::tasks::TaskKeyword;
use std::collections::HashMap;

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

    pub fn solve(&self) -> Option<Attribute> {
        Some(match self {
            Self::Literal(v) => v.clone(),
            Self::Variable(_) => return None,
            Self::Function(fc) => todo!(),
            Self::UniOp(_, _) => todo!(),
            Self::BiOp(_, _, _) => todo!(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum UniOperator {
    Not,
    Negative,
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
}
