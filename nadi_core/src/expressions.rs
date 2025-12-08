use crate::attrs::{AttrMap, Attribute, FromAttribute, HasAttributes};
use crate::functions::FunctionCtx;
use crate::network::Propagation;
use crate::node::Node;
use crate::tasks::{
    AttrTask, CondTask, EvalTask, FunctionType, TaskContext, TaskKeyword, WhileTask,
};
use crate::template::Template;
use crate::timeseries::{CompleteSeries, HasSeries, Series};
use crate::udf::UserFunction;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct EvalError {
    /// Type of Eval Error
    pub ty: EvalErrorType,
    /// Position of Eval Error
    pub position: Vec<(usize, usize)>,
}

impl EvalError {
    pub fn pos(mut self, position: (usize, usize)) -> EvalError {
        self.position.push(position);
        self
    }
}

impl From<EvalError> for String {
    fn from(val: EvalError) -> String {
        val.to_string()
    }
}

impl std::error::Error for EvalError {}

impl std::fmt::Display for EvalError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        if let Some(pos) = self.position.iter().last() {
            write!(
                f,
                "EvalError at Line {} Column {}: {}",
                pos.0,
                pos.1,
                self.ty.message()
            )
        } else {
            write!(f, "EvalError: {}", self.ty.message())
        }
    }
}

impl From<EvalErrorType> for EvalError {
    fn from(val: EvalErrorType) -> EvalError {
        val.no_pos()
    }
}

pub trait TaskPosition {
    fn position(&self) -> (usize, usize);
}

macro_rules! impl_position {
    ($ty:ty) => {
        impl TaskPosition for $ty {
            fn position(&self) -> (usize, usize) {
                self.start
            }
        }
    };
}

impl_position!(EvalTask);
impl_position!(AttrTask);
impl_position!(CondTask);
impl_position!(WhileTask);
impl_position!(InputVar);
impl_position!(FunctionCall);
impl_position!(Propagation);

impl EvalErrorType {
    pub fn at<T: TaskPosition>(self, pos: &T) -> EvalError {
        EvalError {
            ty: self,
            position: vec![pos.position()],
        }
    }

    pub fn pos(self, position: (usize, usize)) -> EvalError {
        EvalError {
            ty: self,
            position: vec![position],
        }
    }

    pub fn no_pos(self) -> EvalError {
        EvalError {
            ty: self,
            position: Vec::new(),
        }
    }
}

/// Collection of Errors that can happen during expression evaluation
#[derive(Debug, PartialEq, Clone)]
pub enum EvalErrorType {
    /// User raised error
    UserError(String),
    /// Varible doesn't exist in given context
    UnresolvedVariable,
    /// Function doesn't exist in given context
    FunctionNotFound(Option<FunctionType>, String),
    /// Error in Function Evaluation
    FunctionError(String, String),
    /// Unknown Function Type
    UnknownFunctionType,
    /// Function didn't return a value to be used in expression
    NoReturnValue(String),
    /// Return Statement that returns a value, but if it's outside function this is error
    InvalidReturn(Option<Attribute>),
    /// Node with the name doesn't exit
    NodeNotFound(String),
    /// Node functions run on a non-node context
    NotANodeContext,
    /// Given Nodes are not connected with a path
    PathNotFound(String, String, String),
    /// Attribute with name doesn't exist
    AttributeNotFound,
    /// Series with name doesn't exist
    SeriesNotFound(String),
    /// TimeSeries with name doesn't exist
    TimeSeriesNotFound(String),
    /// Index out of range for the array
    IndexError,
    // AttributeNotFound(Option<String>, String),
    /// The node doesn't have output node
    NoOutputNode,
    /// The network doesn't have a root node
    NoRootNode,
    /// The node, doesn't have attribute with the given name
    NodeAttributeError(String, String),
    /// Generic error while accessing attribute (type, nesting, etc)
    AttributeError(String),
    /// Operation not valid (like true + 23)
    InvalidOperation,
    /// Variable is not of correct type (e.g. node variable in network function)
    InvalidVariableType,
    /// Array required for operation
    NotAnArray,
    /// Number required for operation
    NotANumber,
    /// Boolean required for operation
    NotABool,
    /// Arrays are of different length
    DifferentLength(usize, usize),
    /// Division by zero
    DivideByZero,
    /// String Template Rendering Failed
    RenderError(String),
    /// Regex compilation failed (invalid pattern)
    RegexError(regex::Error),
    /// Parse Error from import or other operations
    ParseError(String),
    /// Logical error by the developer
    LogicalError(&'static str),
    /// Lock on mutex failed
    MutexError(&'static str, u32),
}

impl From<EvalErrorType> for String {
    fn from(val: EvalErrorType) -> String {
        val.message()
    }
}

impl std::error::Error for EvalErrorType {}

impl std::fmt::Display for EvalErrorType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "EvalError: {}", self.message())
    }
}

impl EvalErrorType {
    /// Format the error into a message using the values
    pub fn message(&self) -> String {
        match self {
            Self::UserError(s) => return format!("Error: {s}"),
            Self::UnresolvedVariable => "Unresolved variable in expression",
            Self::FunctionNotFound(t, n) => {
                return format!(
                    "{} function named {n:?} not found",
                    t.as_ref().map(|t| t.name()).unwrap_or("Any")
                );
            }
            Self::FunctionError(n, s) => return format!("Error in function {n}: {s}"),
            Self::UnknownFunctionType => "Unknown function type",
            Self::NoReturnValue(n) => return format!("Function {n} did not return a value"),
            // if return is inside a function it is caught and the value is returned
            Self::InvalidReturn(_) => "Return statement outside of function",
            Self::NodeNotFound(n) => return format!("Node: {n:?} not found"),
            Self::NotANodeContext => "Not inside a node context, cannot use node attributes",
            Self::PathNotFound(s, e, t) => {
                return format!("No path found between Nodes {s:?} and {t:?}, path ends at {e:?}");
            }
            Self::AttributeNotFound => "Attribute not found",
            Self::SeriesNotFound(msg) => return format!("No Series: {msg}"),
            Self::TimeSeriesNotFound(msg) => return format!("No TimeSeries: {msg}"),
            Self::IndexError => "Index out of range for array",
            // Self::AttributeNotFound(Some(n), var) => {
            //     return format!("Node: {n:?} Attribute {var:?} not found")
            // }
            // Self::AttributeNotFound(None, var) => return format!("Attribute {var:?} not found"),
            Self::NoOutputNode => "Node doesn't have a output node",
            Self::NoRootNode => "Network doesn't have a root node",
            Self::AttributeError(s) => return format!("Attribute Error: {s}"),
            Self::NodeAttributeError(n, s) => return format!("Node {n:?} Attribute Error: {s}"),
            Self::InvalidOperation => "Operation not Allowed",
            Self::InvalidVariableType => "Variable type invalid in this context",
            Self::NotAnArray => "Array required Non-Array found",
            Self::NotANumber => "Numerical Operation on Non Number",
            Self::NotABool => "Boolean Operation on Non Boolean",
            Self::DifferentLength(a, b) => {
                return format!("Different number of members in an array: {a} and {b}");
            }
            Self::DivideByZero => "Division by Zero",
            Self::RenderError(e) => return format!("Rendering Failed: {e}"),
            Self::RegexError(e) => return format!("Error in regex: {e}"),
            Self::ParseError(e) => return format!("Error parsing: {e}"),
            Self::LogicalError(s) => return format!("Logical Error: {s}, contact developer"),
            Self::MutexError(f, l) => {
                return format!("Mutex Error on file: {f}::{l}, contact developer");
            }
        }
        .to_string()
    }
}

/// Expression for the task system
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// Literal attribute values like `2`, `true`, etc
    Literal(Attribute),
    /// Variable (dot separated, optionally with context)
    Variable(InputVar),
    /// String Template to Render in given context
    Render(Template),
    /// Error in variable resolve process. Will propagation if evaluation is tried.
    ///
    /// This is for cases where the evaluation might short circuit,
    /// ifelse etc, this lets the error be ignored during resolve step
    /// but raised during eval step. That way the error can be ignored
    /// if the evaluation doesn't hit it,
    ResolveError(EvalError),
    /// User raised errors
    UserError(String),
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
    /// try-catch blocks
    TryCatch(Box<Expression>, Box<Expression>),
    /// for loop blocks that filters and runs through an expression
    ForEachIf(
        String,
        Box<Expression>,
        Box<Expression>,
        Option<Box<Expression>>,
    ),
    /// Return the value if inside a function
    Return(Option<Box<Expression>>),
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Literal(a) => std::fmt::Display::fmt(a, f),
            Self::Variable(v) => std::fmt::Display::fmt(v, f),
            Self::Render(v) => write!(f, "r{v:?}"),
            Self::ResolveError(e) => write!(f, "error {:?}", e.to_string()),
            Self::UserError(e) => write!(f, "error {:?}", e),
            Self::Function(fc) => std::fmt::Display::fmt(fc, f),
            // multifunction is only generated after resolvingg
            // function; so this shouldn't be used much, but I'm
            // representing it as array of function, even though it
            // cann't be loaded with this syntax from tasks file
            Self::MultiFunction(fcs) => {
                write!(f, "array(")?;
                for (i, fc) in fcs.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    std::fmt::Display::fmt(fc, f)?;
                }
                write!(f, ")")
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
                    format!("({})", expr1)
                } else {
                    expr1.to_string()
                },
                op.to_string(),
                if expr2.nested() {
                    format!("({})", expr2)
                } else {
                    expr2.to_string()
                },
            ),
            Self::IfElse(cond, expr1, expr2) => {
                write!(f, "if ({}) {{{}}} else {{{}}}", cond, expr1, expr2)
            }
            Self::TryCatch(expr1, expr2) => write!(f, "try {{{}}} catch {{{}}}", expr1, expr2),
            Self::ForEachIf(var, expr1, expr2, cond) => {
                write!(
                    f,
                    "for {var} in {} {{{}}}",
                    if expr1.nested() {
                        format!("({})", expr1)
                    } else {
                        expr1.to_string()
                    },
                    expr2
                )?;
                if let Some(c) = cond {
                    if c.nested() {
                        write!(f, "if ({})", c)
                    } else {
                        write!(f, "if {}", c)
                    }
                } else {
                    Ok(())
                }
            }
            Self::Return(None) => write!(f, "return"),
            Self::Return(Some(expr)) => write!(f, "return {expr}"),
        }
    }
}

impl Expression {
    /// check if the expression is nested (needs parenthesis)
    pub fn nested(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::ResolveError(_) => false,
            Self::UserError(_) => false,
            Self::Variable(_) => false,
            Self::Render(_) => false,
            Self::Function(_) => false,
            Self::MultiFunction(_) => false,
            Self::UniOp(_, _) => true,
            Self::BiOp(_, _, _) => true,
            Self::IfElse(_, _, _) => true,
            Self::TryCatch(_, _) => true,
            Self::ForEachIf(..) => true,
            Self::Return(_) => false,
        }
    }

    /// check if the expression contains variables or not
    pub fn has_variables(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::ResolveError(_) => false,
            Self::UserError(_) => false,
            Self::Variable(_) => true,
            // Could also do true here, as render stirng without variable is converted to a string
            Self::Render(templ) => templ.has_variables(),
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
            Self::TryCatch(e1, e2) => e1.has_variables() || e2.has_variables(),
            Self::ForEachIf(..) => true, // TODO: it will have local var, so we need to test if it has var by getting a list of variables.
            Self::Return(None) => false,
            Self::Return(Some(expr)) => expr.has_variables(),
        }
    }

    /// This simplifies the expression by evaluating the nested expressions without variables
    ///
    /// It makes it easier to catch any mistakes and reduce the
    /// complexity while evaluating expressions later with actual
    /// attribute variables.
    pub fn simplify(self, ft: &FunctionType, ctx: &TaskContext) -> Result<Expression, EvalError> {
        if !self.has_variables() {
            return Ok(Self::Literal(self.eval_value(ft, ctx, None, None)?));
        }
        match self {
            Self::Literal(v) => {
                // shouldn't happen
                eprintln!("WARN: Logic Error, literal shouldn't be considered a variable");
                Ok(Self::Literal(v))
            }
            Self::Variable(v) => Ok(Self::Variable(v)),
            Self::Render(v) => match v.lit() {
                Some(s) => Ok(Self::Literal(Attribute::String(s.into()))),
                None => Ok(Self::Render(v)),
            },
            // this should also be handled on has_variables()
            Self::ResolveError(e) => Err(e),
            Self::UserError(s) => Err(EvalErrorType::UserError(s).no_pos()),
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
            Self::TryCatch(expr1, expr2) => match expr1.simplify(ft, ctx) {
                Ok(blk) => Ok(Self::TryCatch(
                    Box::new(blk),
                    Box::new(expr2.simplify(ft, ctx)?),
                )),
                _ => expr2.simplify(ft, ctx),
            },
            Self::ForEachIf(var, expr1, expr2, cond) => Ok(Self::ForEachIf(
                var,
                Box::new(expr1.simplify(ft, ctx)?),
                Box::new(expr2.simplify(ft, ctx)?),
                if let Some(c) = cond {
                    Some(Box::new(c.simplify(ft, ctx)?))
                } else {
                    None
                },
            )),
            Self::Return(None) => Ok(Self::Return(None)),
            Self::Return(Some(expr)) => Ok(Self::Return(Some(Box::new(expr.simplify(ft, ctx)?)))),
        }
    }

    /// Call [`Self::resolve`] then [`Self::eval`]
    pub fn resolve_eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, local, node)
            .and_then(|e| e.eval(ft, ctx, local, node))
    }

    /// Call [`Self::resolve`] then [`Self::eval_value`]
    pub fn resolve_eval_value(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Attribute, EvalError> {
        self.resolve(ft, ctx, local, node)
            .and_then(|e| e.eval_value(ft, ctx, local, node))
    }

    /// Call [`Self::resolve`] then [`Self::eval_mut`]
    pub fn resolve_eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        self.resolve(ft, ctx, local, node)
            .and_then(|e| e.eval_mut(ft, ctx, local, node))
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
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Expression, EvalError> {
        match self {
            Self::ResolveError(_) => Ok(self.clone()),
            Self::UserError(_) => Ok(self.clone()),
            Self::Literal(_) => Ok(self.clone()),
            Self::Variable(vt) => vt.resolve(ft, ctx, local, node),
            Self::Render(t) => {
                let res = match ft {
                    FunctionType::Env => t.render(&ctx.env),
                    FunctionType::Network => t.render(&ctx.network),
                    FunctionType::Node => match node {
                        Some(n) => t.render(
                            &n.try_lock()
                                .into_option()
                                .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?,
                        ),
                        None => {
                            return Err(EvalErrorType::LogicalError(
                                "Node function ran without Node value",
                            )
                            .no_pos());
                        }
                    },
                };
                match res {
                    Ok(s) => Ok(Self::Literal(Attribute::String(s.into()))),
                    Err(e) => Ok(Self::ResolveError(
                        EvalErrorType::RenderError(e.to_string()).no_pos(),
                    )),
                }
            }
            Self::Function(fc) => match &fc.ty {
                Some(VarType::Nodes(prop)) => {
                    let fcs = ctx
                        .propagation(*prop.clone())?
                        .iter()
                        .map(|n| fc.resolve(ft, ctx, local, Some(n)))
                        .collect::<Result<Vec<FunctionCall>, EvalError>>()?;
                    Ok(Self::MultiFunction(fcs))
                }
                Some(VarType::Inputs) => {
                    let fcs = node
                        .ok_or(
                            EvalErrorType::LogicalError("Inputs Function tried without Node value")
                                .pos(fc.position()),
                        )?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(fc.position()))?
                        .inputs()
                        .into_iter()
                        .map(|n| fc.resolve(ft, ctx, local, Some(n)))
                        .collect::<Result<Vec<FunctionCall>, EvalError>>()?;
                    Ok(Self::MultiFunction(fcs))
                }
                Some(VarType::Output) => {
                    let v = match node
                        .ok_or(
                            EvalErrorType::LogicalError("Output Function tried without Node value")
                                .pos(fc.position()),
                        )?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(fc.position()))?
                        .output()
                        .into_option()
                    {
                        Some(o) => Self::Function(fc.resolve(ft, ctx, local, Some(o))?),
                        None => {
                            Expression::ResolveError(EvalErrorType::NoOutputNode.pos(fc.position()))
                        }
                    };
                    Ok(v)
                }
                Some(VarType::Root) => {
                    let v = match ctx.network.outlet() {
                        Some(o) => Self::Function(fc.resolve(ft, ctx, local, Some(o))?),
                        None => {
                            Expression::ResolveError(EvalErrorType::NoRootNode.pos(fc.position()))
                        }
                    };
                    Ok(v)
                }
                _ => fc.resolve(ft, ctx, local, node).map(Self::Function),
            },
            Self::MultiFunction(fcs) => fcs
                .into_iter()
                .map(|fc| fc.resolve(ft, ctx, local, node))
                .collect::<Result<Vec<FunctionCall>, EvalError>>()
                .map(|fcs| Self::MultiFunction(fcs)),
            Self::UniOp(op, expr) => Ok(Self::UniOp(
                op.clone(),
                Box::new(expr.resolve(ft, ctx, local, node)?),
            )),
            Self::BiOp(op, expr1, expr2) => Ok(Self::BiOp(
                op.clone(),
                Box::new(expr1.resolve(ft, ctx, local, node)?),
                Box::new(expr2.resolve(ft, ctx, local, node)?),
            )),
            Self::IfElse(cond, expr1, expr2) => Ok(Self::IfElse(
                Box::new(cond.resolve(ft, ctx, local, node)?),
                Box::new(expr1.resolve(ft, ctx, local, node)?),
                Box::new(expr2.resolve(ft, ctx, local, node)?),
            )),
            Self::TryCatch(expr1, expr2) => match expr1.resolve(ft, ctx, local, node) {
                Ok(blk) => Ok(Self::TryCatch(
                    Box::new(blk),
                    Box::new(expr2.resolve(ft, ctx, local, node)?),
                )),
                _ => expr2.resolve(ft, ctx, local, node),
            },
            Self::ForEachIf(var, expr1, expr2, cond) => Ok(Self::ForEachIf(
                var.clone(),
                Box::new(expr1.resolve(ft, ctx, local, node)?),
                // can't resolve these two yet since they have local variables
                expr2.clone(),
                cond.clone(),
            )),
            Self::Return(None) => Ok(Self::Return(None)),
            Self::Return(Some(expr)) => Ok(Self::Return(Some(Box::new(
                expr.resolve(ft, ctx, local, node)?,
            )))),
        }
    }

    /// Evaluate the expression
    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval(ft, ctx, local, node),
            e => e.eval_value(ft, ctx, local, node).map(Some),
        }
    }

    /// Evaluate the expression with mutable context
    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        match self {
            Self::Function(fc) => fc.eval_mut(ft, ctx, local, node),
            e => e.eval_value(ft, ctx, local, node).map(Some),
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
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Attribute, EvalError> {
        match self {
            Self::Literal(v) => Ok(v.clone()),
            Self::Variable(vt) => {
                // local variables might not be resolved, so let's
                // resolve them again here
                if vt.ty.is_none() {
                    if let Some(loc) = local {
                        if let Ok(v) = vt.attr_nested(loc) {
                            return Ok(v.clone());
                        }
                    }
                }
                Err(EvalErrorType::UnresolvedVariable.pos(vt.position()))
            }
            // Resolve should have converted Render to Lit(String)
            Self::Render(_) => Err(EvalErrorType::UnresolvedVariable.no_pos()),
            Self::ResolveError(e) => Err(e.clone()),
            Self::UserError(s) => Err(EvalErrorType::UserError(s.clone()).no_pos()),
            Self::Function(fc) => match fc.eval(ft, ctx, local, node) {
                Ok(None) => {
                    Err(EvalErrorType::NoReturnValue(fc.name.to_string()).pos(fc.position()))
                }
                Ok(Some(v)) => Ok(v),
                Err(e) => Err(e),
            },
            Self::MultiFunction(fcs) => fcs
                .into_iter()
                .map(|fc| match fc.eval(ft, ctx, local, node) {
                    Ok(None) => {
                        Err(EvalErrorType::NoReturnValue(fc.name.to_string()).pos(fc.position()))
                    }
                    Ok(Some(v)) => Ok(v),
                    Err(e) => Err(e),
                })
                .collect::<Result<Vec<Attribute>, EvalError>>()
                .map(|ar| Attribute::Array(ar.into())),
            Self::UniOp(op, expr) => op.eval(expr.eval_value(ft, ctx, local, node)?),
            Self::BiOp(op, expr1, expr2) => {
                let first = expr1.eval_value(ft, ctx, local, node)?;
                // short circuit logical operations to prevent eval error
                match (op, &first) {
                    (BiOperator::And, Attribute::Bool(false)) => return Ok(false.into()),
                    (BiOperator::Or, Attribute::Bool(true)) => return Ok(true.into()),
                    _ => (),
                }
                op.eval(first, expr2.eval_value(ft, ctx, local, node)?)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ft, ctx, local, node)?;
                let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                if cond {
                    expr1.eval_value(ft, ctx, local, node)
                } else {
                    expr2.eval_value(ft, ctx, local, node)
                }
            }
            Self::TryCatch(expr1, expr2) => match expr1.eval_value(ft, ctx, local, node) {
                Ok(val) => Ok(val),
                _ => expr2.eval_value(ft, ctx, local, node),
            },
            Self::ForEachIf(var, expr1, expr2, cond) => {
                let parent = match expr1.eval_value(ft, ctx, local, node)? {
                    Attribute::Array(ar) => ar,
                    _ => return Err(EvalErrorType::NotAnArray.no_pos()),
                };
                let mut results = Vec::with_capacity(parent.len());
                for val in parent {
                    // I think in future we want a data type that
                    // saved reference of parent locales
                    let mut loc = local.cloned().unwrap_or_default();
                    loc.insert(var.to_string().into(), val.clone());
                    if let Some(c) = cond {
                        let var = c.resolve_eval_value(ft, ctx, Some(&loc), node)?;
                        match var {
                            Attribute::Bool(true) => (),
                            Attribute::Bool(false) => continue,
                            _ => return Err(EvalErrorType::NotABool.no_pos()),
                        }
                    }
                    let val = expr2.resolve_eval_value(ft, ctx, Some(&loc), node)?;
                    results.push(val);
                }
                Ok(Attribute::Array(results.into()))
            }

            // return is done through errors due to easier propagation
            // to parent expressions. If the expression is inside a
            // function it will be caught and returned as the result
            // of the function evaluation
            Self::Return(None) => Err(EvalErrorType::InvalidReturn(None).no_pos()),
            Self::Return(Some(expr)) => {
                let ret = expr.eval(ft, ctx, local, node)?;
                Err(EvalErrorType::InvalidReturn(ret).no_pos())
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
        .map_err(|e| e.no_pos())
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
    /// Precedence of each binary operator.
    pub fn precedence(&self) -> i32 {
        match self {
            // arithmetic – highest
            Self::Divide | Self::Multiply | Self::IntDivide | Self::Modulus => 5,
            Self::Add | Self::Substract => 4,

            // comparisons
            Self::GreaterThan
            | Self::LessThan
            | Self::GreaterThanEqual
            | Self::LessThanEqual
            | Self::Equal
            | Self::NotEqual
            | Self::In
            | Self::Match => 3,

            // logical
            Self::And => 2,
            Self::Or => 1,
        }
    }

    /// Generate Expression using a stackof expressions
    pub fn expr_from_stack(self, stack: &mut Vec<Expression>) -> Option<Expression> {
        let right = stack.pop()?;
        let left = stack.pop()?;
        Some(Expression::BiOp(self, Box::new(left), Box::new(right)))
    }

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
        .map_err(|e| e.no_pos())
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

/// Different ways to index a variable
#[derive(Clone, PartialEq, Debug)]
pub enum InputVarIndex {
    Str(String),
    Int(usize),
}

impl InputVarIndex {
    pub fn index<'a, 'b>(&'a self, val: &'b Attribute) -> Result<&'b Attribute, EvalErrorType> {
        match (self, val) {
            (Self::Str(s), Attribute::Table(am)) => am
                .attr(s)
                .ok_or(EvalErrorType::AttributeError(format!("Key {s} not found"))),
            (Self::Int(i), Attribute::Array(ar)) => ar.get(*i).ok_or(EvalErrorType::IndexError),
            (i, v) => Err(EvalErrorType::AttributeError(format!(
                "{} can not be indexed by {}",
                v.type_name(),
                i.type_name(),
            ))),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Str(_) => "string",
            Self::Int(_) => "integer",
        }
    }
}

impl std::fmt::Display for InputVarIndex {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Str(s) => write!(f, "{s}"),
            Self::Int(i) => write!(f, "{i}"),
        }
    }
}

/// Variable in task system that can be used as an input
#[derive(Clone, PartialEq, Debug)]
pub struct InputVar {
    /// Type of the variable
    pub ty: Option<VarType>,
    /// variable name
    pub name: String,
    /// suffix of the variable names/indices
    pub indices: Vec<InputVarIndex>,
    /// Only check the presence of a value
    pub check: bool,
    /// start position of the variable
    pub start: (usize, usize),
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
            self.name,
            self.indices
                .iter()
                .map(|p| format!(".{p}"))
                .collect::<Vec<String>>()
                .join(""),
            self.check.then_some("?").unwrap_or_default(),
        )
    }
}

impl InputVar {
    /// new input variable
    pub fn new(
        ty: Option<VarType>,
        name: String,
        indices: Vec<InputVarIndex>,
        check: bool,
        start: (usize, usize),
    ) -> Self {
        Self {
            ty,
            name,
            indices,
            check,
            start,
        }
    }

    pub fn attr_nested<'a, 'b, T: HasAttributes>(
        &'a self,
        attrmap: &'b T,
    ) -> Result<&'b Attribute, EvalErrorType> {
        let mut at = attrmap
            .attr(&self.name)
            .ok_or(EvalErrorType::AttributeError(format!(
                "Attribute {} not found",
                self.name
            )))?;
        for ind in &self.indices {
            at = ind.index(at)?;
        }
        Ok(at)
    }

    /// Resolve the variable given the context
    ///
    /// it returns expression intead of Attribute because we want to
    /// return Expression::ResolveError
    pub fn resolve(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Expression, EvalError> {
        let attr = match &self.ty {
            Some(ty) => match ty {
                VarType::Local => self.attr_nested(local.unwrap_or(&ctx.env)).cloned(),
                VarType::Env => self.attr_nested(&ctx.env).cloned(),
                VarType::Network => self.attr_nested(&ctx.network).cloned(),
                VarType::Root => self
                    .attr_nested(
                        &ctx.network
                            .outlet()
                            .ok_or(EvalErrorType::NoRootNode.pos(self.position()))?
                            .try_lock()
                            .into_option()
                            .ok_or(
                                EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                            )?,
                    )
                    .cloned(),
                VarType::Node(n) => match (n, node) {
                    // Node name explicitly given
                    (Some(n), _) => self
                        .attr_nested(
                            &ctx.network
                                .node_by_name(n)
                                .ok_or(
                                    EvalErrorType::NodeNotFound(n.to_string()).pos(self.position()),
                                )?
                                .try_lock()
                                .into_option()
                                .ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?,
                        )
                        .cloned(),
                    // take node from the context (while running node functions)
                    (None, Some(n)) => self
                        .attr_nested(&n.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?)
                        .cloned(),
                    (None, None) => {
                        return Err(match ft {
                            FunctionType::Node => EvalErrorType::LogicalError(
                                "Node variable tried without Node value",
                            )
                            .pos(self.position()),
                            _ => EvalErrorType::InvalidVariableType.pos(self.position()),
                        });
                    }
                },
                VarType::Inputs => match node {
                    Some(n) => {
                        if self.check {
                            let res: Vec<Attribute> = n
                                .try_lock()
                                .into_option()
                                .ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?
                                .inputs()
                                .iter()
                                .map(|i| {
                                    Ok(Attribute::Bool(
                                        self.attr_nested(
                                            &i.try_lock().into_option().ok_or(
                                                EvalErrorType::MutexError(file!(), line!())
                                                    .pos(self.position()),
                                            )?,
                                        )
                                        .is_ok()
                                        .into(),
                                    ))
                                })
                                .collect::<Result<_, EvalError>>()?;
                            return Ok(Expression::Literal(Attribute::Array(res.into())));
                        } else {
                            let mut vars = Vec::new();
                            for i in n
                                .try_lock()
                                .into_option()
                                .ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?
                                .inputs()
                            {
                                let a = self
                                    .attr_nested(
                                        &i.try_lock().into_option().ok_or(
                                            EvalErrorType::MutexError(file!(), line!())
                                                .pos(self.position()),
                                        )?,
                                    )
                                    .cloned();
                                vars.push(a.map_err(|e| e.pos(self.position()))?);
                            }
                            return Ok(Expression::Literal(Attribute::Array(vars.into())));
                        }
                    }
                    None => {
                        return Err(match ft {
                            FunctionType::Node => EvalErrorType::LogicalError(
                                "Inputs variable tried without Node value",
                            )
                            .pos(self.position()),
                            _ => EvalErrorType::InvalidVariableType.pos(self.position()),
                        });
                    }
                },
                VarType::Output => match node {
                    Some(n) => self
                        .attr_nested(
                            &match n
                                .try_lock()
                                .into_option()
                                .ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?
                                .output()
                                .into_option()
                            {
                                Some(o) => o,
                                None if self.check => {
                                    return Ok(Expression::Literal(Attribute::Bool(false)));
                                }
                                None => {
                                    return Ok(Expression::ResolveError(
                                        EvalErrorType::NoOutputNode.pos(self.position()),
                                    ));
                                }
                            }
                            .try_lock()
                            .into_option()
                            .ok_or(
                                EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                            )?,
                        )
                        .cloned(),
                    None => {
                        return Err(match ft {
                            FunctionType::Node => EvalErrorType::LogicalError(
                                "Output variable tried without Node value",
                            )
                            .pos(self.position()),
                            _ => EvalErrorType::InvalidVariableType.pos(self.position()),
                        });
                    }
                },
                VarType::Nodes(prop) => {
                    let mut vars = Vec::new();
                    for n in ctx.propagation(*prop.clone())? {
                        let a = self
                            .attr_nested(&n.try_lock().into_option().ok_or(
                                EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                            )?)
                            .cloned();
                        if self.check {
                            vars.push(a.is_ok().into());
                        } else {
                            vars.push(a.map_err(|e| e.pos(self.position()))?);
                        }
                    }
                    return Ok(Expression::Literal(Attribute::Array(vars.into())));
                }
            },
            None => match ft {
                // since function expressions are only evaluated as env functions now
                FunctionType::Env => self.attr_nested(local.unwrap_or(&ctx.env)).cloned(),
                FunctionType::Network => self.attr_nested(&ctx.network).cloned(),
                FunctionType::Node => match node {
                    Some(n) => self
                        .attr_nested(
                            &n.try_lock()
                                .into_option()
                                .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?,
                        )
                        .cloned(),
                    None => {
                        return Err(EvalErrorType::LogicalError(
                            "Node function ran without Node value",
                        )
                        .no_pos());
                    }
                },
            },
        };
        if self.check {
            Ok(Expression::Literal(attr.is_ok().into()))
        } else {
            match attr {
                Ok(v) => Ok(Expression::Literal(v)),
                Err(e) => Ok(Expression::ResolveError(e.pos(self.position()))),
            }
        }
    }
}

/// Type of variable
#[derive(PartialEq, Debug, Clone)]
pub enum VarType {
    /// Local Variables
    Local,
    /// Environmen Variable
    Env,
    /// Node variable (only valid in a node function without explicit name)
    Node(Option<String>),
    /// Network variable
    Network,
    /// Inputs variable (only valid in a node function)
    Inputs,
    /// Output variable (only valid in a node function)
    Output,
    /// Nodes variable (array of variable from each node)
    Nodes(Box<Propagation>),
    /// variable for the Root node of the network
    Root,
}

impl VarType {
    /// Build a VarType from [`TaskKeyword`] if valid
    pub fn from_keyword(
        kw: &TaskKeyword,
        prop: Option<Propagation>,
        node: Option<String>,
    ) -> Option<Self> {
        match kw {
            TaskKeyword::Node => Some(VarType::Node(node)),
            TaskKeyword::Network => Some(VarType::Network),
            TaskKeyword::Env => Some(VarType::Env),
            TaskKeyword::Local => Some(VarType::Local),
            TaskKeyword::Inputs => Some(VarType::Inputs),
            TaskKeyword::Output => Some(VarType::Output),
            TaskKeyword::Nodes => Some(VarType::Nodes(Box::new(prop.unwrap_or_default()))),
            TaskKeyword::Root => Some(VarType::Root),
            _ => None,
        }
    }

    /// Convert to [`FunctionType`]
    pub fn to_functiontype(&self) -> &'static FunctionType {
        match self {
            VarType::Node(_) => &FunctionType::Node,
            VarType::Network => &FunctionType::Network,
            VarType::Env | VarType::Local => &FunctionType::Env,
            VarType::Inputs | VarType::Output | VarType::Nodes(_) | VarType::Root => {
                &FunctionType::Node
            }
        }
    }
}

impl std::fmt::Display for VarType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let ty = match self {
            VarType::Node(None) => "node",
            VarType::Node(Some(n)) => {
                return write!(f, "node[{n:?}]");
            }
            VarType::Network => "network",
            VarType::Env => "env",
            VarType::Local => "local",
            VarType::Inputs => "inputs",
            VarType::Output => "output",
            VarType::Nodes(p) => {
                return write!(f, "nodes{p}");
            }
            VarType::Root => "root",
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
    /// start position of the function call
    pub start: (usize, usize),
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
        start: (usize, usize),
    ) -> Self {
        Self {
            ty,
            node,
            name,
            args,
            kwargs,
            start,
        }
    }

    /// Simplify the expressions in the functioncall without variables
    ///
    /// Recursively simplifies the expressions in the function arguments
    pub fn simplify(&mut self, ft: &FunctionType, ctx: &TaskContext) -> Result<(), EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(
                a.clone()
                    .simplify(ft, ctx)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(
                k.clone(),
                a.clone()
                    .simplify(ft, ctx)
                    .map_err(|e| e.pos(self.position()))?,
            );
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
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Self, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(
                a.resolve(ft, ctx, local, node)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(
                k.clone(),
                a.resolve(ft, ctx, local, node)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        Ok(FunctionCall {
            ty: self.ty.clone(),
            node: node.cloned(),
            name: self.name.clone(),
            args,
            kwargs,
            start: self.start,
        })
    }

    pub fn function_ctx(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<FunctionCtx, EvalError> {
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(
                a.eval_value(ft, ctx, local, node)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(
                k.clone(),
                a.eval_value(ft, ctx, local, node)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        Ok(FunctionCtx::from_arg_kwarg(args, kwargs))
    }

    /// Eval the function in a mutable context
    pub fn eval_mut(
        &self,
        ft: &FunctionType,
        ctx: &mut TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let fctx = self.function_ctx(ft, ctx, local, node)?;
        self.run_w_ctx_mut(ft, ctx, fctx, node, None)
    }

    /// Eval the function in immutable context
    pub fn eval(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Option<Attribute>, EvalError> {
        let ft = self.ty.as_ref().map(VarType::to_functiontype).unwrap_or(ft);
        let node = self.node.as_ref().or(node);
        let fctx = self.function_ctx(ft, ctx, local, node)?;
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
            FunctionType::Env => {
                match tctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    Some(func) if ft == &FunctionType::Env => func.eval(tctx, fctx),
                    _ => match tctx.functions.env(&self.name) {
                        Some(f) => f.call(&fctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                        }),
                        None => Err(EvalErrorType::FunctionNotFound(
                            Some(original.unwrap_or_else(|| ft.clone())),
                            self.name.to_string(),
                        )
                        .pos(self.position())),
                    },
                }
            }
            FunctionType::Node => match tctx.functions.node(&self.name) {
                Some(f) => {
                    let n = node
                        .ok_or(
                            EvalErrorType::LogicalError("Node function called without node")
                                .pos(self.position()),
                        )?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?;
                    f.call(&n, &fctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    })
                }
                // if the function is not called by explicit type then also test environment function
                None if self.ty.is_none() => {
                    self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone()))
                }
                None => Err(EvalErrorType::FunctionNotFound(
                    Some(original.unwrap_or_else(|| ft.clone())),
                    self.name.to_string(),
                )
                .pos(self.position())),
            },
            FunctionType::Network => match tctx.functions.network(&self.name) {
                Some(f) => f.call(&tctx.network, &fctx).res().map_err(|s| {
                    EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                }),
                // if the function is not called by explicit type then also test environment function
                None if self.ty.is_none() => {
                    self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone()))
                }
                None => Err(EvalErrorType::FunctionNotFound(
                    Some(original.unwrap_or_else(|| ft.clone())),
                    self.name.to_string(),
                )
                .pos(self.position())),
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
            FunctionType::Env => match tctx.udf(&self.name).cloned() {
                // priority for the locally defined function
                Some(func) if ft == &FunctionType::Env => func.eval(tctx, fctx),
                _ => match tctx.functions.env(&self.name) {
                    Some(f) => f.call(&fctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(original.unwrap_or_else(|| ft.clone())),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                },
            },
            FunctionType::Node => match tctx.functions.node(&self.name) {
                Some(f) => {
                    let mut n = node
                        .ok_or(
                            EvalErrorType::LogicalError("Node function called without node")
                                .pos(self.position()),
                        )?
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?;
                    f.call_mut(&mut n, &fctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    })
                }
                // if the function is not called by explicit type then also test environment function
                None if self.ty.is_none() => {
                    self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone()))
                }
                None => Err(EvalErrorType::FunctionNotFound(
                    Some(original.unwrap_or_else(|| ft.clone())),
                    self.name.to_string(),
                )
                .pos(self.position())),
            },
            FunctionType::Network => match tctx.functions.network(&self.name) {
                Some(f) => f.call_mut(&mut tctx.network, &fctx).res().map_err(|s| {
                    EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                }),
                // if the function is not called by explicit type then also test environment function
                None if self.ty.is_none() => {
                    self.run_w_ctx(&FunctionType::Env, tctx, fctx, node, Some(ft.clone()))
                }
                None => Err(EvalErrorType::FunctionNotFound(
                    Some(original.unwrap_or_else(|| ft.clone())),
                    self.name.to_string(),
                )
                .pos(self.position())),
            },
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum MapFunction {
    /// Annonymous function defintion
    Defn(UserFunction),
    /// Name of a Function to use
    Pointer(String),
}

impl std::fmt::Display for MapFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Defn(udf) => write!(f, "{udf}"),
            Self::Pointer(n) => write!(f, "@{n}"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum SeriesExpression {
    /// Expression evaluates to an attribute, but the attribute must be an array
    AttrExpr(Expression),
    /// Another Series, simply copy
    // if single or zip them if multiple (add type for that)
    Series(Option<VarType>, String),
    /// Series Mapped to a Function
    ///
    /// The functions should have same number of arguments as the
    /// number of series, they can have additional optional arguments
    SeriesMap(Option<VarType>, String, MapFunction),
}

impl std::fmt::Display for SeriesExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::AttrExpr(expr) => write!(f, "{expr}"),
            Self::Series(Some(vt), srs) => write!(f, "{vt}.{srs}"),
            Self::Series(None, srs) => write!(f, "{srs}"),
            Self::SeriesMap(Some(vt), srs, mf) => write!(f, "{vt}.{srs} -> {mf}"),
            Self::SeriesMap(None, srs, mf) => write!(f, "{srs} -> {mf}"),
        }
    }
}

impl SeriesExpression {
    /// Evaluates the series expression and returns a series
    pub fn resolve_eval_value(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Series, EvalError> {
        match self {
            Self::AttrExpr(expr) => match expr.resolve_eval_value(ft, ctx, local, node)? {
                Attribute::Array(ar) => Ok(CompleteSeries::from(ar).retype().into()),
                _ => return Err(EvalErrorType::NotAnArray.no_pos()),
            },
            Self::Series(ty, sr) => get_series(ft, ctx, node, ty, sr),
            Self::SeriesMap(ty, sr, func) => {
                let sr = get_series(ft, ctx, node, ty, sr)?;
                match func {
                    MapFunction::Defn(udf) => {
                        let func_call = |arg| {
                            udf.eval(ctx, FunctionCtx::from_arg_kwarg(vec![arg], HashMap::new()))
                        };
                        sr.map_values(&func_call)
                    }
                    MapFunction::Pointer(name) => {
                        let func_call = |arg| {
                            let fctx = FunctionCtx::from_arg_kwarg(vec![arg], HashMap::new());
                            match ctx.udf(&name).cloned() {
                                // priority for the locally defined function
                                Some(func) => func.eval(ctx, fctx),
                                _ => match ctx.functions.env(&name) {
                                    Some(f) => f.call(&fctx).res().map_err(|e| {
                                        EvalErrorType::FunctionError(
                                            name.to_string(),
                                            e.to_string(),
                                        )
                                        .no_pos()
                                    }),
                                    None => Err(EvalErrorType::FunctionNotFound(
                                        Some(FunctionType::Env),
                                        name.to_string(),
                                    )
                                    .no_pos()),
                                },
                            }
                        };
                        sr.map_values(&func_call)
                    }
                }
            }
        }
    }
}

fn get_series(
    ft: &FunctionType,
    ctx: &TaskContext,
    node: Option<&Node>,
    vt: &Option<VarType>,
    name: &str,
) -> Result<Series, EvalError> {
    match (vt, ft) {
        (None, FunctionType::Env) | (Some(VarType::Env), _) => ctx
            .try_series(name)
            .map_err(|e| EvalErrorType::SeriesNotFound(e).no_pos())
            .cloned(),
        // Node function, or node vartype without node name
        (None, FunctionType::Node) | (Some(VarType::Node(None)), _) => match node {
            Some(n) => get_node_series(n, name),
            None => Err(EvalErrorType::InvalidOperation.no_pos()),
        },
        // Node name given explicitely
        (Some(VarType::Node(Some(node))), _) => match ctx.network.node_by_name(node) {
            Some(n) => get_node_series(n, name),
            None => Err(EvalErrorType::NodeNotFound(node.to_string()).no_pos()),
        },
        // (Some(VarType::Inputs), _) => match ctx.network.outlet() {
        //     Some(o) => get_node_series(o, name),
        //     None => Err(EvalErrorType::NoRootNode.no_pos()),
        // },
        (Some(VarType::Output), _) => match node
            .ok_or(EvalErrorType::NotANodeContext.no_pos())?
            .try_lock()
            .into_option()
            .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
            .output()
            .into_option()
        {
            Some(o) => get_node_series(o, name),
            None => Err(EvalErrorType::NoRootNode.no_pos()),
        },
        (Some(VarType::Root), _) => match ctx.network.outlet() {
            Some(o) => get_node_series(o, name),
            None => Err(EvalErrorType::NoRootNode.no_pos()),
        },
        (None, FunctionType::Network) | (Some(VarType::Network), _) => ctx
            .network
            .try_series(name)
            .map_err(|e| EvalErrorType::SeriesNotFound(e).no_pos())
            .cloned(),
        _ => Err(
            EvalErrorType::LogicalError("Reading Series are not implemented for this type")
                .no_pos(),
        ),
    }
}

fn get_node_series(n: &Node, name: &str) -> Result<Series, EvalError> {
    n.try_lock()
        .into_option()
        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
        .try_series(name)
        .map_err(|e| EvalErrorType::SeriesNotFound(e).no_pos())
        .cloned()
}
