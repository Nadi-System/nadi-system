use crate::attrs::{Attribute, FromAttribute, HasAttributes};
use crate::functions::FunctionCtx;
use crate::node::Node;
use crate::tasks::{FunctionType, TaskContext, TaskKeyword};
use std::collections::HashMap;

#[derive(Debug, PartialEq)]
pub enum EvalError {
    UnresolvedVariable,
    FunctionNotFound(FunctionType, String),
    FunctionError(String, String),
    NoReturnValue(String),
    NodeNotFound(String),
    PathNotFound(String, String, String),
    AttributeNotFound,
    // AttributeNotFound(Option<String>, String),
    NoOutputNode,
    NodeAttributeError(String, String),
    AttributeError(String),
    InvalidOperation,
    InvalidVariableType,
    NotANumber,
    NotABool,
    DivideByZero,
    LogicalError(&'static str),
    MutexError(&'static str, u32),
}

impl EvalError {
    pub fn message(&self) -> String {
        match self {
            Self::UnresolvedVariable => "Unresolved variable in expression",
            Self::FunctionNotFound(t, n) => {
                return format!("{} function: {n:?} not found", t.to_string())
            }
            Self::FunctionError(n, s) => return format!("Error in function {n}: {s}"),
            Self::NoReturnValue(n) => return format!("Function {n} did not return a value"),
            Self::NodeNotFound(n) => return format!("Node: {n:?} not found"),
            Self::PathNotFound(s, e, t) => {
                return format!("No path found between Nodes {s:?} and {t:?}, path ends at {e:?}")
            }
            Self::AttributeNotFound => "Attribute not found",
            // Self::AttributeNotFound(Some(n), var) => {
            //     return format!("Node: {n:?} Attribute {var:?} not found")
            // }
            // Self::AttributeNotFound(None, var) => return format!("Attribute {var:?} not found"),
            Self::NoOutputNode => "Node doesn't have a output node",
            Self::AttributeError(s) => return format!("Attribute Error: {s}"),
            Self::NodeAttributeError(n, s) => return format!("Node {n:?} Attribute Error: {s}"),
            Self::InvalidOperation => "Operation not Allowed",
            Self::InvalidVariableType => "Variable type invalid in this context",
            Self::NotANumber => "Numerical Operation on Non Number",
            Self::NotABool => "Boolean Operation on Non Boolean",
            Self::DivideByZero => "Division by Zero",
            Self::LogicalError(s) => return format!("Logical Error: {s}, contact developer"),
            Self::MutexError(f, l) => {
                return format!("Mutex Error on file: {f}::{l}, contact developer")
            }
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

    /// This simplifies the expression by evaluating the nested expressions without variables
    ///
    /// It makes it easier to catch any mistakes and reduce the
    /// complexity while evaluating expressions later with actual
    /// attribute variables.
    pub fn simplify(self, ft: &FunctionType, ctx: &TaskContext) -> Result<Expression, EvalError> {
        if !self.has_variables() {
            return Ok(Self::Literal(self.eval_value(ft, ctx, None)?));
        }
        match self {
            Self::Literal(v) => {
                // shouldn't happen
                eprintln!("WARN: Logic Error, literal shouldn't be considered a variable");
                Ok(Self::Literal(v))
            }
            Self::Variable(v) => Ok(Self::Variable(v)),
            Self::Function(mut fc) => {
                let mut args = Vec::with_capacity(fc.args.len());
                for a in fc.args {
                    args.push(a.simplify(ft, ctx)?);
                }
                let mut kwargs = HashMap::with_capacity(fc.kwargs.len());
                for (k, a) in fc.kwargs {
                    kwargs.insert(k.clone(), a.simplify(ft, ctx)?);
                }
                fc.args = args;
                fc.kwargs = kwargs;
                Ok(Self::Function(fc))
            }
            Self::UniOp(op, expr) => Ok(Self::UniOp(op, Box::new(expr.simplify(ft, ctx)?))),
            Self::BiOp(op, expr1, expr2) => Ok(Self::BiOp(
                op,
                Box::new(expr1.simplify(ft, ctx)?),
                Box::new(expr2.simplify(ft, ctx)?),
            )),
        }
    }

    pub fn resolve_eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, node)
            .and_then(|e| e.eval(ft, ctx, node))
    }

    pub fn resolve_eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, node)
            .and_then(|e| e.eval_mut(ft, ctx, node))
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
                            Some(n) => n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .attr_nested(&vt.names)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError(
                                        "Node variable tried without Node value",
                                    ),
                                    _ => EvalError::InvalidVariableType,
                                })
                            }
                        },
                        VarType::Inputs => match node {
                            Some(n) => {
                                if vt.check {
                                    let res = n
                                        .try_lock()
                                        .into_option()
                                        .ok_or(EvalError::MutexError(file!(), line!()))?
                                        .inputs()
                                        .iter()
                                        .all(|i| {
                                            if let Ok(Some(_)) = i.lock().attr_nested(&vt.names) {
                                                true
                                            } else {
                                                false
                                            }
                                        });
                                    return Ok(Self::Literal(Attribute::Bool(res)));
                                } else {
                                    let mut vars = Vec::new();
                                    for i in n
                                        .try_lock()
                                        .into_option()
                                        .ok_or(EvalError::MutexError(file!(), line!()))?
                                        .inputs()
                                    {
                                        let a = i
                                            .try_lock()
                                            .into_option()
                                            .ok_or(EvalError::MutexError(file!(), line!()))?
                                            .attr_nested(&vt.names)
                                            .map(|a| a.cloned());
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
                                    FunctionType::Node => EvalError::LogicalError(
                                        "Inputs variable tried without Node value",
                                    ),
                                    _ => EvalError::InvalidVariableType,
                                })
                            }
                        },
                        VarType::Output => match node {
                            Some(n) => n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .output()
                                .into_option()
                                .ok_or(EvalError::NoOutputNode)?
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .attr_nested(&vt.names)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError(
                                        "Output variable tried without Node value",
                                    ),
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
                            Some(n) => n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .attr_nested(&vt.names)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(EvalError::LogicalError(
                                    "Node function ran without Node value",
                                ))
                            }
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

    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval(ft, ctx, node),
            e => e.eval_value(ft, ctx, node).map(|v| Some(v)),
        }
    }

    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval_mut(ft, ctx, node),
            e => e.eval_value(ft, ctx, node).map(|v| Some(v)),
        }
    }

    pub fn eval_value(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Attribute, EvalError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Variable(_) => Err(EvalError::UnresolvedVariable),
            Self::Function(fc) => match fc.eval(ft, ctx, node) {
                Ok(None) => Err(EvalError::NoReturnValue(fc.name.to_string())),
                Ok(Some(v)) => Ok(v),
                Err(e) => Err(e),
            },
            Self::UniOp(op, expr) => op.eval(expr.eval_value(ft, ctx, node)?),
            Self::BiOp(op, expr1, expr2) => op.eval(
                expr1.eval_value(ft, ctx, node)?,
                expr2.eval_value(ft, ctx, node)?,
            ),
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
    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.eval_value(ft, ctx, node)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.eval_value(ft, ctx, node)?);
        }
        let fctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        self.run_w_ctx_mut(ft, &self.name, ctx, fctx, node, None)
    }

    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.eval_value(ft, ctx, node)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.eval_value(ft, ctx, node)?);
        }
        let fctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        self.run_w_ctx(ft, &self.name, ctx, fctx, node, None)
    }

    pub fn run_w_ctx(
        &self,
        ft: &FunctionType,
        name: &str,
        tctx: &TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
        original: Option<FunctionType>,
    ) -> Result<Option<Attribute>, EvalError> {
        match ft {
            FunctionType::Env => match tctx.functions.env(name) {
                Some(f) => f
                    .call(&fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(name.to_string(), s)),
                None => Err(EvalError::FunctionNotFound(
                    original.unwrap_or_else(|| ft.clone()),
                    self.name.to_string(),
                )),
            },
            FunctionType::Node => match tctx.functions.node(name) {
                Some(f) => {
                    let n = node
                        .ok_or(EvalError::LogicalError("Node function called without node"))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?;
                    f.call(&n, &fctx)
                        .res()
                        .map_err(|s| EvalError::FunctionError(name.to_string(), s))
                }
                None => self.run_w_ctx(
                    &FunctionType::Env,
                    &self.name,
                    tctx,
                    fctx,
                    node,
                    Some(ft.clone()),
                ),
            },
            FunctionType::Network => match tctx.functions.network(name) {
                Some(f) => f
                    .call(&tctx.network, &fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(name.to_string(), s)),
                None => self.run_w_ctx(
                    &FunctionType::Env,
                    &self.name,
                    tctx,
                    fctx,
                    node,
                    Some(ft.clone()),
                ),
            },
        }
    }

    pub fn run_w_ctx_mut(
        &self,
        ft: &FunctionType,
        name: &str,
        tctx: &mut TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
        original: Option<FunctionType>,
    ) -> Result<Option<Attribute>, EvalError> {
        match ft {
            FunctionType::Env => match tctx.functions.env(name) {
                Some(f) => f
                    .call(&fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(name.to_string(), s)),
                None => Err(EvalError::FunctionNotFound(
                    original.unwrap_or_else(|| ft.clone()),
                    self.name.to_string(),
                )),
            },
            FunctionType::Node => match tctx.functions.node(name) {
                Some(f) => {
                    let mut n = node
                        .ok_or(EvalError::LogicalError("Node function called without node"))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?;
                    f.call_mut(&mut n, &fctx)
                        .res()
                        .map_err(|s| EvalError::FunctionError(name.to_string(), s))
                }
                None => self.run_w_ctx(
                    &FunctionType::Env,
                    &self.name,
                    tctx,
                    fctx,
                    node,
                    Some(ft.clone()),
                ),
            },
            FunctionType::Network => match tctx.functions.network(name) {
                Some(f) => f
                    .call_mut(&mut tctx.network, &fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(name.to_string(), s)),
                None => self.run_w_ctx(
                    &FunctionType::Env,
                    &self.name,
                    tctx,
                    fctx,
                    node,
                    Some(ft.clone()),
                ),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::tokenizer::get_tokens;
    use rstest::rstest;
}
