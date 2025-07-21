use crate::attrs::{Attribute, FromAttribute, HasAttributes};
use crate::functions::FunctionCtx;
use crate::node::Node;
use crate::tasks::{FunctionType, TaskContext, TaskKeyword};
use std::collections::HashMap;

/// Collection of Errors that can happen during expression evaluation
#[derive(Debug, PartialEq, Clone)]
pub enum EvalError {
    /// Varible doesn't exist in given context
    UnresolvedVariable,
    /// Function doesn't exist in given context
    FunctionNotFound(FunctionType, String),
    /// Error in Function Evaluation
    FunctionError(String, String),
    /// Function didn't return a value to be used in expression
    NoReturnValue(String),
    /// Node with the name doesn't exit
    NodeNotFound(String),
    /// Given Nodes are not connected with a path
    PathNotFound(String, String, String),
    /// Attribute with name doesn't exist
    AttributeNotFound,
    // AttributeNotFound(Option<String>, String),
    /// The node doesn't have output node
    NoOutputNode,
    /// The node, doesn't have attribute with the given name
    NodeAttributeError(String, String),
    /// Generic error while accessing attribute (type, nesting, etc)
    AttributeError(String),
    /// Operation not valid (like true + 23)
    InvalidOperation,
    /// Variable is not of correct type (e.g. node variable in network function)
    InvalidVariableType,
    /// Number required for operation
    NotANumber,
    /// Boolean required for operation
    NotABool,
    /// Arrays are of different length
    DifferentLength(usize, usize),
    /// Division by zero
    DivideByZero,
    /// Regex compilation failed (invalid pattern)
    RegexError(regex::Error),
    /// Logical error by the developer
    LogicalError(&'static str),
    /// Lock on mutex failed
    MutexError(&'static str, u32),
}

impl From<EvalError> for String {
    fn from(val: EvalError) -> String {
        val.message()
    }
}

impl EvalError {
    /// Format the error into a message using the values
    pub fn message(&self) -> String {
        match self {
            Self::UnresolvedVariable => "Unresolved variable in expression",
            Self::FunctionNotFound(t, n) => return format!("{} function: {n:?} not found", t),
            Self::FunctionError(n, s) => return format!("Error in function {n}: {s}"),
            Self::NoReturnValue(n) => return format!("Function {n} did not return a value"),
            Self::NodeNotFound(n) => return format!("Node: {n:?} not found"),
            Self::PathNotFound(s, e, t) => {
                return format!("No path found between Nodes {s:?} and {t:?}, path ends at {e:?}");
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
            Self::DifferentLength(a, b) => {
                return format!("Different number of members in an array: {a} and {b}");
            }
            Self::DivideByZero => "Division by Zero",
            Self::RegexError(e) => return format!("Error in regex: {e}"),
            Self::LogicalError(s) => return format!("Logical Error: {s}, contact developer"),
            Self::MutexError(f, l) => {
                return format!("Mutex Error on file: {f}::{l}, contact developer");
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

/// Expression for the task system
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// Literal attribute values like `2`, `true`, etc
    Literal(Attribute),
    /// Variable (dot separated, optionally with context)
    Variable(InputVar),
    /// Error in variable resolve process. Will propagation if evaluation is tried.
    ///
    /// This is for cases where the evaluation might short circuit,
    /// ifelse etc, this lets the error be ignored during resolve step
    /// but raised during eval step. That way the error can be ignored
    /// if the evaluation doesn't hit it,
    ResolveError(EvalError),
    /// Function call
    Function(FunctionCall),
    /// Multiple function calls after resolving `nodes` and `inputs` type context
    MultiFunction(Vec<FunctionCall>),
    /// With Unary operators e.g. `-``, `!true`
    UniOp(UniOperator, Box<Expression>),
    /// With Binary operator e.g. `1 + 3`
    BiOp(BiOperator, Box<Expression>, Box<Expression>),
    /// if-else statement
    IfElse(Box<Expression>, Box<Expression>, Box<Expression>),
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Literal(a) => std::fmt::Display::fmt(a, f),
            Self::Variable(v) => std::fmt::Display::fmt(v, f),
            Self::ResolveError(e) => write!(f, "ResolveError: {}", e),
            Self::Function(fc) => std::fmt::Display::fmt(fc, f),
            // multifunction is only generated after resolving
            // function; so this shouldn't be used much, but I'm
            // representing it as array of function, even though it
            // cann't be loaded with this syntax from tasks file
            Self::MultiFunction(fcs) => {
                write!(f, "[")?;
                for (i, fc) in fcs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    std::fmt::Display::fmt(fc, f)?;
                }
                write!(f, "[")
            }
            Self::UniOp(op, expr) => {
                if expr.nested() {
                    write!(f, "{} ({})", op.to_string(), expr.to_string())
                } else {
                    write!(f, "{} {}", op.to_string(), expr.to_string())
                }
            }
            Self::BiOp(op, expr1, expr2) => write!(
                f,
                "{} {} {}",
                if expr1.nested() {
                    format!("({})", expr1.to_string())
                } else {
                    expr1.to_string()
                },
                op.to_string(),
                if expr2.nested() {
                    format!("({})", expr2.to_string())
                } else {
                    expr2.to_string()
                },
            ),
            Self::IfElse(cond, expr1, expr2) => write!(
                f,
                "if ({}) {{{}}} else {{{}}}",
                cond.to_string(),
                expr1.to_string(),
                expr2.to_string()
            ),
        }
    }
}

impl Expression {
    /// check if the expression is nested (needs parenthesis)
    pub fn nested(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::ResolveError(_) => false,
            Self::Variable(_) => false,
            Self::Function(_) => false,
            Self::MultiFunction(_) => false,
            Self::UniOp(_, _) => true,
            Self::BiOp(_, _, _) => true,
            Self::IfElse(_, _, _) => true,
        }
    }

    /// check if the expression contains variables or not
    pub fn has_variables(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::ResolveError(_) => false,
            Self::Variable(_) => true,
            Self::Function(fc) => {
                fc.args.iter().any(|e| e.has_variables())
                    || fc.kwargs.iter().any(|e| e.1.has_variables())
            }
            Self::MultiFunction(fcs) => fcs.iter().any(|fc| {
                fc.args.iter().any(|e| e.has_variables())
                    || fc.kwargs.iter().any(|e| e.1.has_variables())
            }),
            Self::UniOp(_, e) => e.has_variables(),
            Self::BiOp(_, e1, e2) => e1.has_variables() || e2.has_variables(),
            Self::IfElse(c, e1, e2) => {
                c.has_variables() || e1.has_variables() || e2.has_variables()
            }
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
            // this should also be handled on has_variables()
            Self::ResolveError(e) => Err(e),
            Self::Function(mut fc) => {
                fc.simplify(ft, ctx)?;
                Ok(Self::Function(fc))
            }
            Self::MultiFunction(fcs) => fcs
                .into_iter()
                .map(|mut fc| {
                    fc.simplify(ft, ctx)?;
                    Ok(fc)
                })
                .collect::<Result<Vec<FunctionCall>, EvalError>>()
                .map(|fcs| Self::MultiFunction(fcs)),
            Self::UniOp(op, expr) => Ok(Self::UniOp(op, Box::new(expr.simplify(ft, ctx)?))),
            Self::BiOp(op, expr1, expr2) => Ok(Self::BiOp(
                op,
                Box::new(expr1.simplify(ft, ctx)?),
                Box::new(expr2.simplify(ft, ctx)?),
            )),
            Self::IfElse(cond, expr1, expr2) => Ok(Self::IfElse(
                Box::new(cond.simplify(ft, ctx)?),
                Box::new(expr1.simplify(ft, ctx)?),
                Box::new(expr2.simplify(ft, ctx)?),
            )),
        }
    }

    /// Call [`Self::resolve`] then [`Self::eval`]
    pub fn resolve_eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, node)
            .and_then(|e| e.eval(ft, ctx, node))
    }

    /// Call [`Self::resolve`] then [`Self::eval_mut`]
    pub fn resolve_eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, node)
            .and_then(|e| e.eval_mut(ft, ctx, node))
    }

    /// Resolve the variables in the expression for the given context
    ///
    /// This is an important process where the variables are extracted
    /// and a literal expression is made to be evaluated. This takes
    /// the function type (env/node/network) and possibly current node
    /// and resolves the variables. Unresolved error is kept as a
    /// Valid [`Expression`] on this step for lazy evaluation.
    pub fn resolve(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Expression, EvalError> {
        match self {
            Self::ResolveError(_) => Ok(self.clone()),
            Self::Literal(_) => Ok(self.clone()),
            Self::Variable(vt) => {
                let attr = match &vt.ty {
                    Some(ty) => match ty {
                        VarType::Env => ctx
                            .env
                            .attr_nested(&vt.prefix, &vt.name)
                            .map(|a| a.cloned()),
                        VarType::Network => ctx
                            .network
                            .attr_nested(&vt.prefix, &vt.name)
                            .map(|a| a.cloned()),
                        VarType::Node => match node {
                            Some(n) => n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .attr_nested(&vt.prefix, &vt.name)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError(
                                        "Node variable tried without Node value",
                                    ),
                                    _ => EvalError::InvalidVariableType,
                                });
                            }
                        },
                        VarType::Inputs => match node {
                            Some(n) => {
                                if vt.check {
                                    let res: Vec<Attribute> = n
                                        .try_lock()
                                        .into_option()
                                        .ok_or(EvalError::MutexError(file!(), line!()))?
                                        .inputs()
                                        .iter()
                                        .map(|i| {
                                            match i
                                                .try_lock()
                                                .into_option()
                                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                                .attr_nested(&vt.prefix, &vt.name)
                                            {
                                                Ok(Some(_)) => Ok(Attribute::Bool(true)),
                                                _ => Ok(Attribute::Bool(false)),
                                            }
                                        })
                                        .collect::<Result<_, EvalError>>()?;
                                    return Ok(Self::Literal(Attribute::Array(res.into())));
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
                                            .attr_nested(&vt.prefix, &vt.name)
                                            .map(|a| a.cloned());
                                        match a {
                                            Ok(Some(v)) => vars.push(v),
                                            Ok(None) => {
                                                return Ok(Self::ResolveError(
                                                    EvalError::AttributeNotFound,
                                                ));
                                            }
                                            Err(e) => {
                                                return Ok(Self::ResolveError(
                                                    EvalError::AttributeError(e),
                                                ));
                                            }
                                        }
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
                                });
                            }
                        },
                        VarType::Output => match node {
                            Some(n) => match n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .output()
                                .into_option()
                            {
                                Some(o) => o,
                                None if vt.check => {
                                    return Ok(Self::Literal(Attribute::Bool(false)));
                                }
                                None => return Ok(Self::ResolveError(EvalError::NoOutputNode)),
                            }
                            .try_lock()
                            .into_option()
                            .ok_or(EvalError::MutexError(file!(), line!()))?
                            .attr_nested(&vt.prefix, &vt.name)
                            .map(|a| a.cloned()),
                            None => {
                                return Err(match ft {
                                    FunctionType::Node => EvalError::LogicalError(
                                        "Output variable tried without Node value",
                                    ),
                                    _ => EvalError::InvalidVariableType,
                                });
                            }
                        },
                        VarType::Nodes => {
                            let mut vars = Vec::new();
                            for n in ctx.network.nodes() {
                                let a = n
                                    .try_lock()
                                    .into_option()
                                    .ok_or(EvalError::MutexError(file!(), line!()))?
                                    .attr_nested(&vt.prefix, &vt.name)
                                    .map(|a| a.cloned());
                                let val = a.map_err(EvalError::AttributeError)?;
                                if vt.check {
                                    vars.push(val.is_some().into());
                                } else {
                                    vars.push(val.ok_or(EvalError::AttributeNotFound)?);
                                }
                            }
                            return Ok(Self::Literal(Attribute::Array(vars.into())));
                        }
                    },
                    None => match ft {
                        FunctionType::Env => ctx
                            .env
                            .attr_nested(&vt.prefix, &vt.name)
                            .map(|a| a.cloned()),
                        FunctionType::Network => ctx
                            .network
                            .attr_nested(&vt.prefix, &vt.name)
                            .map(|a| a.cloned()),
                        FunctionType::Node => match node {
                            Some(n) => n
                                .try_lock()
                                .into_option()
                                .ok_or(EvalError::MutexError(file!(), line!()))?
                                .attr_nested(&vt.prefix, &vt.name)
                                .map(|a| a.cloned()),
                            None => {
                                return Err(EvalError::LogicalError(
                                    "Node function ran without Node value",
                                ));
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
                    match attr {
                        Ok(Some(v)) => Ok(Self::Literal(v)),
                        Ok(None) => Ok(Self::ResolveError(EvalError::AttributeNotFound)),
                        Err(e) => Ok(Self::ResolveError(EvalError::AttributeError(e))),
                    }
                }
            }
            Self::Function(fc) => match fc.ty {
                Some(VarType::Nodes) => {
                    let fcs = ctx
                        .network
                        .nodes()
                        .map(|n| fc.resolve(ft, ctx, Some(n)))
                        .collect::<Result<Vec<FunctionCall>, EvalError>>()?;
                    Ok(Self::MultiFunction(fcs))
                }
                Some(VarType::Inputs) => {
                    let fcs = node
                        .ok_or(EvalError::LogicalError(
                            "Inputs Function tried without Node value",
                        ))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?
                        .inputs()
                        .into_iter()
                        .map(|n| fc.resolve(ft, ctx, Some(n)))
                        .collect::<Result<Vec<FunctionCall>, EvalError>>()?;
                    Ok(Self::MultiFunction(fcs))
                }
                Some(VarType::Output) => {
                    let v = match node
                        .ok_or(EvalError::LogicalError(
                            "Output Function tried without Node value",
                        ))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?
                        .output()
                        .into_option()
                    {
                        Some(o) => Self::Function(fc.resolve(ft, ctx, Some(o))?),
                        None => Expression::ResolveError(EvalError::NoOutputNode),
                    };
                    Ok(v)
                }
                _ => fc.resolve(ft, ctx, node).map(Self::Function),
            },
            Self::MultiFunction(fcs) => fcs
                .into_iter()
                .map(|fc| fc.resolve(ft, ctx, node))
                .collect::<Result<Vec<FunctionCall>, EvalError>>()
                .map(|fcs| Self::MultiFunction(fcs)),
            Self::UniOp(op, expr) => Ok(Self::UniOp(
                op.clone(),
                Box::new(expr.resolve(ft, ctx, node)?),
            )),
            Self::BiOp(op, expr1, expr2) => Ok(Self::BiOp(
                op.clone(),
                Box::new(expr1.resolve(ft, ctx, node)?),
                Box::new(expr2.resolve(ft, ctx, node)?),
            )),
            Self::IfElse(cond, expr1, expr2) => Ok(Self::IfElse(
                Box::new(cond.resolve(ft, ctx, node)?),
                Box::new(expr1.resolve(ft, ctx, node)?),
                Box::new(expr2.resolve(ft, ctx, node)?),
            )),
        }
    }

    /// Evaluate the expression
    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval(ft, ctx, node),
            e => e.eval_value(ft, ctx, node).map(Some),
        }
    }

    /// Evaluate the expression with mutable context
    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval_mut(ft, ctx, node),
            e => e.eval_value(ft, ctx, node).map(Some),
        }
    }

    /// Evaluate the expression and return a value
    ///
    /// All expressions except functions return values by default,
    /// functions may or may not return value based on the evaluation
    /// results. Refer to [`functions::FunctionRet`] for possible
    /// return values from a function.
    pub fn eval_value(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Attribute, EvalError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Variable(_) => Err(EvalError::UnresolvedVariable),
            Self::ResolveError(e) => Err(e.clone()),
            Self::Function(fc) => match fc.eval(ft, ctx, node) {
                Ok(None) => Err(EvalError::NoReturnValue(fc.name.to_string())),
                Ok(Some(v)) => Ok(v),
                Err(e) => Err(e),
            },
            Self::MultiFunction(fcs) => fcs
                .into_iter()
                .map(|fc| match fc.eval(ft, ctx, node) {
                    Ok(None) => Err(EvalError::NoReturnValue(fc.name.to_string())),
                    Ok(Some(v)) => Ok(v),
                    Err(e) => Err(e),
                })
                .collect::<Result<Vec<Attribute>, EvalError>>()
                .map(|ar| Attribute::Array(ar.into())),
            Self::UniOp(op, expr) => op.eval(expr.eval_value(ft, ctx, node)?),
            Self::BiOp(op, expr1, expr2) => {
                let first = expr1.eval_value(ft, ctx, node)?;
                // short circuit logical operations to prevent eval error
                match (op, &first) {
                    (BiOperator::And, Attribute::Bool(false)) => return Ok(false.into()),
                    (BiOperator::Or, Attribute::Bool(true)) => return Ok(true.into()),
                    _ => (),
                }
                op.eval(first, expr2.eval_value(ft, ctx, node)?)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ft, ctx, node)?;
                let cond = bool::from_attr(&cond).ok_or(EvalError::NotABool)?;
                if cond {
                    expr1.eval_value(ft, ctx, node)
                } else {
                    expr2.eval_value(ft, ctx, node)
                }
            }
        }
    }
}

/// Unary operator
#[derive(Debug, Clone, PartialEq)]
pub enum UniOperator {
    /// Logical Not operator
    Not,
    /// Numerical negative operator
    Negative,
}

impl UniOperator {
    /// Evaluate the expression
    pub fn eval(&self, value: Attribute) -> Result<Attribute, EvalError> {
        match self {
            Self::Not => !value,
            Self::Negative => -value,
        }
    }
}
impl std::fmt::Display for UniOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
            Self::Negative => write!(f, "-"),
        }
    }
}

/// Binary operator
#[derive(Debug, Clone, PartialEq)]
pub enum BiOperator {
    /// Numerical addition
    Add,
    /// Numerical substraction
    Substract,
    /// Numerical multiplication
    Multiply,
    /// Numerical division
    Divide,
    /// Integer division
    IntDivide,
    /// Modulus (remainder) operation
    Modulus,
    /// Check equality
    Equal,
    /// Check inequality
    NotEqual,
    /// Check lesser than
    LessThan,
    /// Check greater than
    GreaterThan,
    /// Check lesser than or equal
    LessThanEqual,
    /// Check greater than or equal
    GreaterThanEqual,
    /// Check if the value is in other
    In,
    /// Check for string match with regex pattern
    Match,
    /// Logical and operation
    And,
    /// logical or operation
    Or,
}

impl BiOperator {
    /// Evaluate the expression
    pub fn eval(&self, val1: Attribute, val2: Attribute) -> Result<Attribute, EvalError> {
        match self {
            Self::Add => val1 + val2,
            Self::Substract => val1 - val2,
            Self::Multiply => val1 * val2,
            Self::Divide => val1 / val2,
            Self::IntDivide => val1.int_div(&val2),
            Self::Modulus => val1 % val2,
            Self::Equal => Ok(Attribute::Bool(val1 == val2)),
            Self::NotEqual => Ok(Attribute::Bool(val1 != val2)),
            Self::LessThan => Ok(Attribute::Bool(val1 < val2)),
            Self::GreaterThan => Ok(Attribute::Bool(val1 > val2)),
            Self::LessThanEqual => Ok(Attribute::Bool(val1 <= val2)),
            Self::GreaterThanEqual => Ok(Attribute::Bool(val1 >= val2)),
            Self::In => val2.contains(&val1).map(Attribute::Bool),
            Self::Match => val1.str_match(&val2).map(Attribute::Bool),
            Self::And => val1 & val2,
            Self::Or => val1 | val2,
        }
    }
}

impl std::fmt::Display for BiOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let op = match self {
            Self::Add => "+",
            Self::Substract => "-",
            Self::Multiply => "*",
            Self::Divide => "/",
            Self::IntDivide => "//",
            Self::Modulus => "%",
            Self::Equal => "==",
            Self::NotEqual => "!=",
            Self::LessThan => "<",
            Self::GreaterThan => ">",
            Self::LessThanEqual => "<=",
            Self::GreaterThanEqual => ">=",
            Self::In => "in",
            Self::Match => "match",
            Self::And => "&",
            Self::Or => "|",
        };
        write!(f, "{op}")
    }
}

/// Variable in task system that can be used as an input
#[derive(Clone, PartialEq, Debug)]
pub struct InputVar {
    /// Type of the variable
    pub ty: Option<VarType>,
    /// prefix of the variable name (nested names)
    pub prefix: Vec<String>,
    /// variable name
    pub name: String,
    /// Only check the presence of a value
    pub check: bool,
}

impl std::fmt::Display for InputVar {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            self.ty
                .as_ref()
                .map(|t| format!("{}.", t.to_string()))
                .unwrap_or_default(),
            self.prefix
                .iter()
                .map(|p| format!("{p}."))
                .collect::<Vec<String>>()
                .join(""),
            self.name,
            self.check.then_some("?").unwrap_or_default(),
        )
    }
}

impl InputVar {
    /// new input variable
    pub fn new(ty: Option<VarType>, prefix: Vec<String>, name: String, check: bool) -> Self {
        Self {
            ty,
            prefix,
            name,
            check,
        }
    }
}

/// Type of variable
#[derive(PartialEq, Debug, Clone)]
pub enum VarType {
    /// Environmen Variable
    Env,
    /// Node variable (only valid in a node function)
    Node,
    /// Network variable
    Network,
    /// Inputs variable (only valid in a node function)
    Inputs,
    /// Output variable (only valid in a node function)
    Output,
    /// Nodes variable (array of variable from each node)
    Nodes,
}

impl VarType {
    /// Build a VarType from [`TaskKeyword`] if valid
    pub fn from_keyword(kw: &TaskKeyword) -> Option<Self> {
        match kw {
            TaskKeyword::Node => Some(VarType::Node),
            TaskKeyword::Network => Some(VarType::Network),
            TaskKeyword::Env => Some(VarType::Env),
            TaskKeyword::Inputs => Some(VarType::Inputs),
            TaskKeyword::Output => Some(VarType::Output),
            TaskKeyword::Nodes => Some(VarType::Nodes),
            _ => None,
        }
    }

    /// Convert to [`FunctionType`]
    pub fn to_functiontype(&self) -> &'static FunctionType {
        match self {
            VarType::Node => &FunctionType::Node,
            VarType::Network => &FunctionType::Network,
            VarType::Env => &FunctionType::Env,
            VarType::Inputs => &FunctionType::Node,
            VarType::Output => &FunctionType::Node,
            VarType::Nodes => &FunctionType::Node,
        }
    }
}

impl std::fmt::Display for VarType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ty = match self {
            VarType::Node => "node",
            VarType::Network => "network",
            VarType::Env => "env",
            VarType::Inputs => "inputs",
            VarType::Output => "output",
            VarType::Nodes => "nodes",
        };
        write!(f, "{ty}")
    }
}

/// A function call in the task system
#[derive(Clone)]
pub struct FunctionCall {
    /// Type of the function (nodes, inputs, and output are node function)
    pub ty: Option<VarType>,
    /// Current node: useful to store node to act on, for output/inputs/nodes variety
    pub node: Option<Node>,
    /// Name of the function
    pub name: String,
    /// Positional Arguments
    pub args: Vec<Expression>,
    /// Keyword Arguments
    pub kwargs: HashMap<String, Expression>,
}

impl PartialEq for FunctionCall {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.name == other.name
            && self.args == other.args
            && self.kwargs == other.kwargs
    }
}

impl std::fmt::Debug for FunctionCall {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("FunctionCall")
            .field("ty", &self.ty)
            .field("name", &self.name)
            .field("args", &self.args)
            .field("kwargs", &self.kwargs)
            .finish()
    }
}

impl std::fmt::Display for FunctionCall {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
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
        if let Some(t) = &self.ty {
            write!(f, "{}.{}({}{}{})", t, self.name, args, middle, kwargs)
        } else {
            write!(f, "{}({}{}{})", self.name, args, middle, kwargs)
        }
    }
}

impl FunctionCall {
    /// New functioncall
    pub fn new(
        ty: Option<VarType>,
        node: Option<Node>,
        name: String,
        args: Vec<Expression>,
        kwargs: HashMap<String, Expression>,
    ) -> Self {
        Self {
            ty,
            node,
            name,
            args,
            kwargs,
        }
    }

    /// Simplify the expressions in the functioncall without variables
    ///
    /// Recursively simplifies the expressions in the function arguments
    pub fn simplify(&mut self, ft: &FunctionType, ctx: &TaskContext) -> Result<(), EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.clone().simplify(ft, ctx)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.clone().simplify(ft, ctx)?);
        }
        self.args = args;
        self.kwargs = kwargs;
        Ok(())
    }

    /// Resolve the variables in the functioncall
    ///
    /// Recursively resolves the expressions in the function arguments
    pub fn resolve(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Self, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.resolve(ft, ctx, node)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.resolve(ft, ctx, node)?);
        }
        Ok(FunctionCall {
            ty: self.ty.clone(),
            node: node.cloned(),
            name: self.name.clone(),
            args,
            kwargs,
        })
    }

    /// Eval the function in a mutable context
    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.eval_value(ft, ctx, node)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.eval_value(ft, ctx, node)?);
        }
        let fctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        self.run_w_ctx_mut(ft, ctx, fctx, node, None)
    }

    /// Eval the function in immutable context
    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(a.eval_value(ft, ctx, node)?);
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(k.clone(), a.eval_value(ft, ctx, node)?);
        }
        let fctx = FunctionCtx::from_arg_kwarg(args, kwargs);
        self.run_w_ctx(ft, ctx, fctx, node, None)
    }

    /// Run the function with given context
    pub fn run_w_ctx(
        &self,
        ft: &FunctionType,
        tctx: &TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
        original: Option<FunctionType>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        match ft {
            FunctionType::Env => match tctx.functions.env(&self.name) {
                Some(f) => f
                    .call(&fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(self.name.to_string(), s)),
                None => Err(EvalError::FunctionNotFound(
                    original.unwrap_or_else(|| ft.clone()),
                    self.name.to_string(),
                )),
            },
            FunctionType::Node => match tctx.functions.node(&self.name) {
                Some(f) => {
                    let n = node
                        .ok_or(EvalError::LogicalError("Node function called without node"))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?;
                    f.call(&n, &fctx)
                        .res()
                        .map_err(|s| EvalError::FunctionError(self.name.to_string(), s))
                }
                None => self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone())),
            },
            FunctionType::Network => match tctx.functions.network(&self.name) {
                Some(f) => f
                    .call(&tctx.network, &fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(self.name.to_string(), s)),
                None => self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone())),
            },
        }
    }

    /// Run the function with given immutable context
    pub fn run_w_ctx_mut(
        &self,
        ft: &FunctionType,
        tctx: &mut TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
        original: Option<FunctionType>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        match ft {
            FunctionType::Env => match tctx.functions.env(&self.name) {
                Some(f) => f
                    .call(&fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(self.name.to_string(), s)),
                None => Err(EvalError::FunctionNotFound(
                    original.unwrap_or_else(|| ft.clone()),
                    self.name.to_string(),
                )),
            },
            FunctionType::Node => match tctx.functions.node(&self.name) {
                Some(f) => {
                    let mut n = node
                        .ok_or(EvalError::LogicalError("Node function called without node"))?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?;
                    f.call_mut(&mut n, &fctx)
                        .res()
                        .map_err(|s| EvalError::FunctionError(self.name.to_string(), s))
                }
                None => self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone())),
            },
            FunctionType::Network => match tctx.functions.network(&self.name) {
                Some(f) => f
                    .call_mut(&mut tctx.network, &fctx)
                    .res()
                    .map_err(|s| EvalError::FunctionError(self.name.to_string(), s)),
                None => self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone())),
            },
        }
    }
}
