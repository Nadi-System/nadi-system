use crate::attrs::{AttrMap, Attribute, FromAttribute, HasAttributes};
use crate::functions::FunctionCtx;
use crate::network::Propagation;
use crate::node::{Node, NodeInner};
use crate::structs::{NadiAttrType, NadiStructExpr};
use crate::tasks::{
    AttrTask, CondTask, EvalTask, FunctionType, TaskContext, TaskKeyword, WhileTask,
};
use crate::template::Template;
use crate::timeseries::{CompleteSeries, HasSeries, HasTimeSeries, Series};
use crate::udf::UserFunction;
use abi_stable::std_types::{RHashMap, RNone, RSome, RString, Tuple2};
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct EvalError {
    /// Type of Eval Error
    pub ty: EvalErrorType,
    /// Position of Eval Error
    pub position: Vec<(usize, usize)>,
    /// Name of the Node if caused in a node
    pub node: Option<String>,
}

impl EvalError {
    pub fn pos(mut self, position: (usize, usize)) -> EvalError {
        self.position.push(position);
        self
    }

    pub fn node(mut self, name: String) -> EvalError {
        self.node.replace(name);
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
        let node = self
            .node
            .as_ref()
            .map(|n| format!("[{n}]"))
            .unwrap_or_default();
        if let Some(pos) = self.position.iter().last() {
            write!(
                f,
                "EvalError{node} at Line {} Column {}: {}",
                pos.0,
                pos.1,
                self.ty.message()
            )
        } else {
            write!(f, "EvalError{node}: {}", self.ty.message())
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
            node: None,
        }
    }

    pub fn pos(self, position: (usize, usize)) -> EvalError {
        EvalError {
            ty: self,
            position: vec![position],
            node: None,
        }
    }

    pub fn no_pos(self) -> EvalError {
        EvalError {
            ty: self,
            position: Vec::new(),
            node: None,
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
    /// Series value was empty
    EmptySeriesValue,
    /// TimeSeries with name doesn't exist
    TimeSeriesNotFound(String),
    /// Index out of range for the array
    IndexError,
    // AttributeNotFound(Option<String>, String),
    /// The node doesn't have input nodes (only used when not having inputs is a problem)
    NoInputNodes,
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
    /// Attribute is not of correct type (e.g. int instead of bool)
    InvalidAttributeType(NadiAttrType, NadiAttrType),
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
    /// Loop Longer than Maximum Iteration limit
    MaxIteratorError(usize),
    /// String Template Rendering Failed
    RenderError(String),
    /// Regex compilation failed (invalid pattern)
    RegexError(regex::Error),
    /// Parse Error from import or other operations
    ParseError(String),
    /// Logical error by the developer
    LogicalError(&'static str),
    /// Planned but not implemented features
    NotImplementedError(&'static str),
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
            Self::EmptySeriesValue => "Series value is empty, and no fill in value",
            Self::IndexError => "Index out of range for array",
            // Self::AttributeNotFound(Some(n), var) => {
            //     return format!("Node: {n:?} Attribute {var:?} not found")
            // }
            // Self::AttributeNotFound(None, var) => return format!("Attribute {var:?} not found"),
            Self::NoInputNodes => "Node doesn't have a input nodes",
            Self::NoOutputNode => "Node doesn't have a output node",
            Self::NoRootNode => "Network doesn't have a root node",
            Self::AttributeError(s) => return format!("Attribute Error: {s}"),
            Self::NodeAttributeError(n, s) => return format!("Node {n:?} Attribute Error: {s}"),
            Self::InvalidOperation => "Operation not Allowed",
            Self::InvalidVariableType => "Variable type invalid in this context",
            Self::InvalidAttributeType(e, f) => {
                return format!("Attribute type assertion failed: expected {e} found {f}");
            }
            Self::NotAnArray => "Array required Non-Array found",
            Self::NotANumber => "Numerical Operation on Non Number",
            Self::NotABool => "Boolean Operation on Non Boolean",
            Self::DifferentLength(a, b) => {
                return format!("Different number of members in an array: {a} and {b}");
            }
            Self::DivideByZero => "Division by Zero",
            Self::MaxIteratorError(n) => {
                return format!("Loop did not exit after {n} iterations, could be infinite loop");
            }
            Self::RenderError(e) => return format!("Rendering Failed: {e}"),
            Self::RegexError(e) => return format!("Error in regex: {e}"),
            Self::ParseError(e) => return format!("Error parsing: {e}"),
            Self::LogicalError(s) => return format!("Logical Error: {s}, contact developer"),
            Self::NotImplementedError(s) => {
                return format!(
                    "Not Implemented: {s}, this feature is planned for future versions"
                );
            }
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
    /// Assignment Expression
    SetVariable(InputVar, Box<Expression>),
    /// String Template to Render in given context
    Render(Template),
    /// Expression with context information
    WithContext(ExprWithContext),
    /// Range of numbers, only integers for now
    Range(Box<Expression>, Option<Box<Expression>>, Box<Expression>),
    /// array expression
    Array(Vec<Expression>),
    /// attrmap expression
    Table(HashMap<String, Expression>),
    /// Struct Expressions are hashmap expression with name
    StructExpr(NadiStructExpr),
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
    /// Get the series as Attributes
    Series(Option<VarType>, bool, String),
    /// Get a value from the series
    SeriesValue(Option<VarType>, bool, String, usize),
}

impl std::fmt::Display for Expression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Literal(a) => std::fmt::Display::fmt(a, f),
            Self::Variable(v) => std::fmt::Display::fmt(v, f),
            Self::SetVariable(v, e) => write!(f, "r{v} = {e}"),
            Self::Render(v) => write!(f, "r{v:?}"),
            Self::WithContext(e) => write!(f, "{e}"),
            Self::Range(b, s, e) => {
                if let Some(s) = s {
                    write!(f, "{b}:{s}:{e}")
                } else {
                    write!(f, "{b}:{e}")
                }
            }
            Self::Array(exprs) => {
                write!(
                    f,
                    "[{}]",
                    exprs
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::Table(exprs) => {
                write!(
                    f,
                    "{{{}}}",
                    exprs
                        .iter()
                        .map(|(k, e)| format!("{k} = {e}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
            Self::StructExpr(e) => write!(f, "{e}"),
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
                    write!(f, "{} ({})", op, expr)
                } else {
                    write!(f, "{} {}", op, expr)
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
                op,
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
            Self::Series(Some(vt), ts, name) => {
                write!(f, "{vt}{}{name}", if *ts { "$$" } else { "$" })
            }
            Self::Series(None, ts, name) => write!(f, "{}{name}", if *ts { "$$" } else { "$" }),
            Self::SeriesValue(Some(vt), ts, name, ind) => {
                write!(f, "{vt}{}{name}[{ind}]", if *ts { "$$" } else { "$" })
            }
            Self::SeriesValue(None, ts, name, ind) => {
                write!(f, "{}{name}[{ind}]", if *ts { "$$" } else { "$" })
            }
        }
    }
}

impl Expression {
    /// check if the expression is nested (needs parenthesis)
    pub fn nested(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::WithContext(_) => false,
            Self::Range(..) => false,
            Self::Array(_) => false,
            Self::Table(_) => false,
            Self::ResolveError(_) => false,
            Self::UserError(_) => false,
            Self::Variable(_) => false,
            Self::SetVariable(..) => false,
            Self::StructExpr(_) => false,
            Self::Render(_) => false,
            Self::Function(_) => false,
            Self::MultiFunction(_) => false,
            Self::UniOp(_, _) => true,
            Self::BiOp(_, _, _) => true,
            Self::IfElse(_, _, _) => true,
            Self::TryCatch(_, _) => true,
            Self::ForEachIf(..) => true,
            Self::Return(_) => false,
            Self::Series(..) => false,
            Self::SeriesValue(..) => false,
        }
    }

    /// check if the expression contains variables or not
    pub fn has_variables(&self) -> bool {
        match self {
            Self::Literal(_) => false,
            Self::ResolveError(_) => false,
            Self::UserError(_) => false,
            Self::Variable(_) => true,
            Self::SetVariable(_, _) => true,
            Self::WithContext(e) => e.expr.has_variables(),
            Self::Range(b, s, e) => {
                b.has_variables()
                    || e.has_variables()
                    || if let Some(s) = s {
                        s.has_variables()
                    } else {
                        false
                    }
            }
            Self::Array(ar) => ar.iter().any(|a| a.has_variables()),
            Self::Table(tb) => tb.iter().any(|a| a.1.has_variables()),
            Self::StructExpr(e) => e.has_variables(),
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
            Self::Series(..) => true,
            Self::SeriesValue(..) => true,
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
            Self::SetVariable(v, e) => e
                .simplify(ft, ctx)
                .map(|r| Self::SetVariable(v, Box::new(r))),
            Self::StructExpr(mut st) => {
                st.values = st
                    .values
                    .into_iter()
                    .map(|Tuple2(k, v)| v.simplify(ft, ctx).map(|r| Tuple2(k, r)))
                    .collect::<Result<_, EvalError>>()?;
                Ok(Self::StructExpr(st))
            }
            Self::Render(v) => match v.lit() {
                Some(s) => Ok(Self::Literal(Attribute::String(s.into()))),
                None => Ok(Self::Render(v)),
            },
            Self::WithContext(mut e) => {
                e.expr = Box::new(e.expr.simplify(e.ty.to_functiontype(), ctx)?);
                Ok(Self::WithContext(e))
            }
            Self::Range(b, s, e) => {
                let b = b.simplify(ft, ctx)?;
                let e = e.simplify(ft, ctx)?;
                let s = if let Some(s) = s {
                    Some(Box::new(s.simplify(ft, ctx)?))
                } else {
                    None
                };
                Ok(Self::Range(Box::new(b), s, Box::new(e)))
            }
            Self::Array(exprs) => {
                let vals: Vec<Expression> = exprs
                    .into_iter()
                    .map(|v| v.simplify(ft, ctx))
                    .collect::<Result<_, _>>()?;
                if vals.iter().all(|e| matches!(e, Expression::Literal(_))) {
                    let vals: Vec<Attribute> = vals
                        .into_iter()
                        .map(|e| match e {
                            Expression::Literal(l) => l,
                            _ => panic!("just checked"),
                        })
                        .collect();
                    Ok(Expression::Literal(Attribute::Array(vals.into())))
                } else {
                    Ok(Expression::Array(vals))
                }
            }
            Self::Table(exprs) => {
                let vals: HashMap<String, Expression> = exprs
                    .into_iter()
                    .map(|(k, v)| v.simplify(ft, ctx).map(|v| (k, v)))
                    .collect::<Result<_, _>>()?;
                if vals
                    .iter()
                    .all(|(_, e)| matches!(e, Expression::Literal(_)))
                {
                    let vals: HashMap<RString, Attribute> = vals
                        .into_iter()
                        .map(|(k, e)| match e {
                            Expression::Literal(l) => (k.to_string().into(), l),
                            _ => panic!("just checked"),
                        })
                        .collect();
                    Ok(Expression::Literal(Attribute::Table(vals.into())))
                } else {
                    Ok(Expression::Table(vals))
                }
            }
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
                .map(Self::MultiFunction),
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
            Self::Series(vt, ts, name) => Ok(Self::Series(vt, ts, name)),
            Self::SeriesValue(vt, ts, name, ind) => Ok(Self::SeriesValue(vt, ts, name, ind)),
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
            Self::WithContext(e) => e.resolve(e.ty.to_functiontype(), ctx, local, node),
            Self::Range(b, s, e) => {
                let b = b.resolve(ft, ctx, local, node)?;
                let e = e.resolve(ft, ctx, local, node)?;
                let s = if let Some(s) = s {
                    Some(Box::new(s.resolve(ft, ctx, local, node)?))
                } else {
                    None
                };
                Ok(Self::Range(Box::new(b), s, Box::new(e)))
            }
            Self::Array(exprs) => {
                let vals: Vec<Expression> = exprs
                    .iter()
                    .map(|v| v.resolve(ft, ctx, local, node))
                    .collect::<Result<_, _>>()?;
                Ok(Expression::Array(vals))
            }
            Self::Table(exprs) => {
                let vals: HashMap<String, Expression> = exprs
                    .iter()
                    .map(|(k, v)| v.resolve(ft, ctx, local, node).map(|v| (k.to_string(), v)))
                    .collect::<Result<_, _>>()?;
                Ok(Expression::Table(vals))
            }
            Self::Variable(vt) => vt.resolve(ft, ctx, local, node),
            Self::SetVariable(v, e) => e
                .resolve(ft, ctx, local, node)
                .map(|r| Self::SetVariable(v.clone(), Box::new(r))),
            Self::StructExpr(st) => {
                let values = st
                    .values
                    .iter()
                    .map(|Tuple2(k, v)| {
                        v.resolve(ft, ctx, local, node)
                            .map(|v| Tuple2(k.clone(), v))
                    })
                    .collect::<Result<_, EvalError>>()?;
                Ok(Self::StructExpr(NadiStructExpr {
                    name: st.name.clone(),
                    values,
                }))
            }
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
                Some(VarType::Leaves) => {
                    let fcs = ctx
                        .network
                        .leaves()
                        .map(|n| fc.resolve(ft, ctx, local, Some(n)))
                        .collect::<Result<Vec<FunctionCall>, EvalError>>()?;
                    Ok(Self::MultiFunction(fcs))
                }
                Some(VarType::Outlets) => {
                    let fcs = ctx
                        .network
                        .outlets()
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
                        .iter()
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
                    let v = match ctx.network.root() {
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
                .iter()
                .map(|fc| fc.resolve(ft, ctx, local, node))
                .collect::<Result<Vec<FunctionCall>, EvalError>>()
                .map(Self::MultiFunction),
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
            Self::Series(vt, ts, name) => match get_series(ft, ctx, node, vt, name, *ts)? {
                Series::Complete(sr) => Ok(Self::Literal(sr.to_attributes().into())),
                Series::Masked(sr, RSome(fill)) => Ok(Self::Literal(
                    sr.to_attributes()
                        .into_iter()
                        .map(|a| a.unwrap_or_else(|| fill.clone()))
                        .collect::<Vec<Attribute>>()
                        .into(),
                )),
                Series::Masked(_, RNone) => Err(EvalErrorType::AttributeError(
                    "Masked Series without Fill value".into(),
                )
                .no_pos()),
            },
            Self::SeriesValue(vt, ts, name, ind) => {
                // this is inefficient as it clones the series; but
                // the inputs type can not be obtained as ref, so
                // can't write a generic function to get this at the
                // moment
                match get_series(ft, ctx, node, vt, name, *ts)?.get_attribute(*ind) {
                    Some(Some(val)) => Ok(Self::Literal(val)),
                    Some(None) => Err(EvalErrorType::EmptySeriesValue.no_pos()),
                    None => Err(EvalErrorType::IndexError.no_pos()),
                }
            }
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
            Self::SetVariable(v, e) => {
                let expr_ctx = v.get_expr_context(ft, &ctx, node)?;
                match expr_ctx {
                    ExprContext::Local => todo!(),
                    ExprContext::Env => {
                        let val = e.eval_value(ft, ctx, local, node)?;
                        v.set_attr_nested(ctx.env.attr_map_mut(), val)?;
                        Ok(None)
                    }
                    ExprContext::Network => {
                        let val = e.eval_value(ft, ctx, local, node)?;
                        v.set_attr_nested(ctx.network.attr_map_mut(), val)?;
                        Ok(None)
                    }
                    ExprContext::Node(n) => {
                        let val = e.eval_value(ft, ctx, local, Some(&n))?;
                        v.set_attr_nested(n.lock().attr_map_mut(), val)?;
                        Ok(None)
                    }
                    ExprContext::Nodes(nds) => {
                        for n in nds {
                            let val = e.eval_value(ft, ctx, local, Some(&n))?;
                            v.set_attr_nested(n.lock().attr_map_mut(), val)?;
                        }
                        Ok(None)
                    }
                    ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
                }
            }
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
            // add error type called immutable context
            Self::SetVariable(..) => Err(EvalErrorType::UnresolvedVariable.no_pos()),
            // expression with context should be resolved to normal expression
            Self::WithContext(_) => Err(EvalErrorType::UnresolvedVariable.no_pos()),
            Self::Range(b, s, e) => {
                let b = b.eval_value(ft, ctx, local, node)?;
                let b = i64::from_attr(&b).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    b.dtype(),
                ))?;
                let e = e.eval_value(ft, ctx, local, node)?;
                let e = i64::from_attr(&e).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    e.dtype(),
                ))?;
                let s = if let Some(s) = s {
                    let s = s.eval_value(ft, ctx, local, node)?;
                    usize::from_attr(&s).ok_or(EvalErrorType::InvalidAttributeType(
                        NadiAttrType::Integer,
                        s.dtype(),
                    ))?
                } else {
                    1
                };
                let vals: Vec<_> = (b..e).step_by(s).map(Attribute::Integer).collect();
                Ok(Attribute::Array(vals.into()))
            }
            Self::Array(exprs) => {
                let vals: Vec<Attribute> = exprs
                    .iter()
                    .map(|v| v.eval_value(ft, ctx, local, node))
                    .collect::<Result<_, _>>()?;
                Ok(Attribute::Array(vals.into()))
            }
            Self::Table(exprs) => {
                let vals: HashMap<RString, Attribute> = exprs
                    .iter()
                    .map(|(k, v)| {
                        v.eval_value(ft, ctx, local, node)
                            .map(|v| (k.to_string().into(), v))
                    })
                    .collect::<Result<_, _>>()?;
                Ok(Attribute::Table(vals.into()))
            }
            Self::StructExpr(st) => {
                let values = st
                    .values
                    .iter()
                    .map(|Tuple2(k, v)| {
                        v.eval_value(ft, ctx, local, node)
                            .map(|v| Tuple2(k.clone(), v))
                    })
                    .collect::<Result<RHashMap<RString, Attribute>, EvalError>>()?;
                Ok(Attribute::Table(values))
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
                .iter()
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
            // these should be resolved away
            Self::Series(..) => todo!(),
            Self::SeriesValue(..) => todo!(),
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
    /// Positive operator (does nothing)
    Positive,
}

impl UniOperator {
    /// Evaluate the expression
    pub fn eval(&self, value: Attribute) -> Result<Attribute, EvalError> {
        match self {
            Self::Not => !value,
            Self::Negative => -value,
            Self::Positive => Ok(value),
        }
        .map_err(|e| e.no_pos())
    }
}
impl std::fmt::Display for UniOperator {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Not => write!(f, "!"),
            Self::Negative => write!(f, "-"),
            Self::Positive => write!(f, "+"),
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
    pub fn index<'b>(&self, val: &'b Attribute) -> Result<&'b Attribute, EvalErrorType> {
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

    pub fn index_mut<'b>(
        &self,
        val: &'b mut Attribute,
    ) -> Result<&'b mut Attribute, EvalErrorType> {
        match (self, val) {
            (Self::Str(s), Attribute::Table(am)) => am
                .attr_mut(s)
                .ok_or(EvalErrorType::AttributeError(format!("Key {s} not found"))),
            (Self::Int(i), Attribute::Array(ar)) => ar.get_mut(*i).ok_or(EvalErrorType::IndexError),
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
                .map(|t| format!("{}.", t))
                .unwrap_or_default(),
            self.name,
            self.indices
                .iter()
                .map(|p| format!(".{p}"))
                .collect::<Vec<String>>()
                .join(""),
            if self.check { "?" } else { Default::default() },
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

    /// Get the attribute value based on the nested indices
    pub fn attr_nested<'b, T: HasAttributes>(
        &self,
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

    /// Set the attribute to the value based on the nested indices
    pub fn set_attr_nested<'b, T: HasAttributes>(
        &self,
        attrmap: &'b mut T,
        val: Attribute,
    ) -> Result<(), EvalErrorType> {
        match self.indices.as_slice() {
            [] => {
                attrmap
                    .attr_map_mut()
                    .insert(self.name.to_string().into(), val);
            }
            [pref @ .., last] => {
                let mut at = attrmap
                    .attr_mut(&self.name)
                    .ok_or(EvalErrorType::AttributeError(format!(
                        "Attribute {} not found",
                        self.name
                    )))?;
                for ind in pref {
                    at = ind.index_mut(at)?;
                }
                match (at, last) {
                    (Attribute::Table(tb), InputVarIndex::Str(s)) => {
                        tb.insert(s.to_string().into(), val);
                    }
                    (Attribute::Array(ar), InputVarIndex::Int(i)) => {
                        *ar.get_mut(*i).ok_or(EvalErrorType::IndexError)? = val;
                    }
                    (v, i) => {
                        return Err(EvalErrorType::AttributeError(format!(
                            "{} can not be indexed by {}",
                            v.type_name(),
                            i.type_name(),
                        )));
                    }
                }
            }
        }
        Ok(())
    }

    fn get_expr_context(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<ExprContext, EvalErrorType> {
        match &self.ty {
            Some(ty) => ty.get_expr_context(ctx, node),
            None => ft.get_expr_context(node),
        }
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
        let expr_ctx = match self.get_expr_context(ft, ctx, node) {
            Ok(k) => k,
            // check also basically checks if the current vartype is valid or invalid
            Err(_) if self.check => return Ok(Expression::Literal(false.into())),
            Err(e) => return Err(e.pos(self.position())),
        };
        let attr = match expr_ctx {
            ExprContext::Local => self
                .attr_nested(local.unwrap_or(ctx.env.attr_map()))
                .cloned(),
            ExprContext::Env => self.attr_nested(&ctx.env).cloned(),
            ExprContext::Network => self.attr_nested(&ctx.network).cloned(),
            ExprContext::Node(n) => self
                .attr_nested(
                    &n.try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?,
                )
                .cloned(),
            ExprContext::Nodes(nds) => {
                if self.check {
                    let res: Vec<Attribute> = nds
                        .iter()
                        .map(|i| {
                            Ok(Attribute::Bool(
                                self.attr_nested(
                                    &i.try_lock().into_option().ok_or(
                                        EvalErrorType::MutexError(file!(), line!())
                                            .pos(self.position()),
                                    )?,
                                )
                                .is_ok(),
                            ))
                        })
                        .collect::<Result<_, EvalError>>()?;
                    return Ok(Expression::Literal(Attribute::Array(res.into())));
                } else {
                    let mut vars = Vec::new();
                    for i in nds {
                        let a = self
                            .attr_nested(&i.try_lock().into_option().ok_or(
                                EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                            )?)
                            .cloned();
                        vars.push(a.map_err(|e| e.pos(self.position()))?);
                    }
                    return Ok(Expression::Literal(Attribute::Array(vars.into())));
                }
            }
            ExprContext::NodesMap(nds) => {
                if self.check {
                    let res: HashMap<RString, Attribute> = nds
                        .iter()
                        .map(|i| {
                            let n = i.try_lock().into_option().ok_or(
                                EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                            )?;
                            Ok((
                                n.name().to_string().into(),
                                Attribute::Bool(self.attr_nested(&n).is_ok()),
                            ))
                        })
                        .collect::<Result<_, EvalError>>()?;
                    return Ok(Expression::Literal(Attribute::Table(res.into())));
                } else {
                    let mut vars = AttrMap::new();
                    for i in nds {
                        let n = i.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        let name = n.name().to_string().into();
                        let a = self.attr_nested(&n).cloned();
                        vars.insert(name, a.map_err(|e| e.pos(self.position()))?);
                    }
                    return Ok(Expression::Literal(Attribute::Table(vars.into())));
                }
            }
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
    /// Environment Variable
    Env,
    /// Node variable (only valid in a node function without explicit name)
    Node(Option<String>),
    /// Network variable
    Network,
    /// Single Input variable (only valid in a node function for a node with single input)
    Input,
    /// Inputs variable (only valid in a node function)
    Inputs,
    /// Inputs variable in map format (only valid in a node function)
    InputsMap,
    /// Output variable (only valid in a node function for a node with single output)
    Output,
    /// Outputs variable (only valid in a node function)
    Outputs,
    /// Outputs variable in map format (only valid in a node function)
    OutputsMap,
    /// Nodes variable (array of variable from each node)
    Nodes(Box<Propagation>),
    /// Nodes variable (map of variable from each node and its name)
    NodesMap(Box<Propagation>),
    /// Outlets of the network
    Outlets,
    /// Outlets of the network in map format
    OutletsMap,
    /// leaf nodes of the network
    Leaves,
    /// leaf nodes of the network in map format
    LeavesMap,
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
            TaskKeyword::Input => Some(VarType::Input),
            TaskKeyword::Inputs => Some(VarType::Inputs),
            TaskKeyword::InputsMap => Some(VarType::InputsMap),
            TaskKeyword::Output => Some(VarType::Output),
            TaskKeyword::Outputs => Some(VarType::Outputs),
            TaskKeyword::OutputsMap => Some(VarType::OutputsMap),
            TaskKeyword::Nodes => Some(VarType::Nodes(Box::new(prop.unwrap_or_default()))),
            TaskKeyword::NodesMap => Some(VarType::NodesMap(Box::new(prop.unwrap_or_default()))),
            TaskKeyword::Root => Some(VarType::Root),
            TaskKeyword::Outlets => Some(VarType::Outlets),
            TaskKeyword::OutletsMap => Some(VarType::OutletsMap),
            TaskKeyword::Leaves => Some(VarType::Leaves),
            TaskKeyword::LeavesMap => Some(VarType::LeavesMap),
            _ => None,
        }
    }

    /// Convert to [`FunctionType`]
    pub fn to_functiontype(&self) -> &'static FunctionType {
        match self {
            VarType::Node(_) => &FunctionType::Node,
            VarType::Network => &FunctionType::Network,
            VarType::Env | VarType::Local => &FunctionType::Env,
            VarType::Input
            | VarType::Inputs
            | VarType::InputsMap
            | VarType::Output
            | VarType::Outputs
            | VarType::OutputsMap
            | VarType::Nodes(_)
            | VarType::NodesMap(_)
            | VarType::Root
            | VarType::Outlets
            | VarType::OutletsMap
            | VarType::Leaves
            | VarType::LeavesMap => &FunctionType::Node,
        }
    }

    /// indicate if the variable type returns a map or not
    pub fn is_map(&self) -> bool {
        match self {
            VarType::Node(_)
            | VarType::Network
            | VarType::Env
            | VarType::Local
            | VarType::Input
            | VarType::Inputs
            | VarType::Output
            | VarType::Outputs
            | VarType::Nodes(_)
            | VarType::Root
            | VarType::Outlets
            | VarType::Leaves => false,
            VarType::NodesMap(_)
            | VarType::OutputsMap
            | VarType::OutletsMap
            | VarType::InputsMap
            | VarType::LeavesMap => true,
        }
    }

    /// Given a node we're currently working on, and the task context,
    /// this function resolves the expression context to know where we
    /// should be evaluating the expression on
    fn get_expr_context(
        &self,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<ExprContext, EvalErrorType> {
        let nodes_func = if self.is_map() {
            |nds| Ok(ExprContext::NodesMap(nds))
        } else {
            |nds| Ok(ExprContext::Nodes(nds))
        };
        match (self, node) {
            (VarType::Local, _) => Ok(ExprContext::Local),
            (VarType::Env, _) => Ok(ExprContext::Env),
            (VarType::Network, _) => Ok(ExprContext::Network),
            (VarType::Nodes(prop) | VarType::NodesMap(prop), _) => {
                nodes_func(ctx.propagation(*prop.clone()).map_err(|e| e.ty)?)
            }
            (VarType::Outlets | VarType::OutletsMap, _) => {
                nodes_func(ctx.network.outlets().cloned().collect())
            }
            (VarType::Leaves | VarType::LeavesMap, _) => {
                nodes_func(ctx.network.leaves().cloned().collect())
            }
            (VarType::Root, _) => match ctx.network.root() {
                Some(r) => Ok(ExprContext::Node(r.clone())),
                None => Err(EvalErrorType::NoRootNode),
            },
            (VarType::Node(Some(n)), _) => match ctx.network.node_by_name(&n) {
                Some(n) => Ok(ExprContext::Node(n.clone())),
                None => Err(EvalErrorType::NodeNotFound(n.to_string())),
            },
            (VarType::Node(None), Some(n)) => Ok(ExprContext::Node(n.clone())),
            (VarType::Input, Some(n)) => match n.lock().input() {
                RSome(r) => Ok(ExprContext::Node(r.clone())),
                RNone => Err(EvalErrorType::NoInputNodes),
            },
            (VarType::Output, Some(n)) => match n.lock().output() {
                RSome(r) => Ok(ExprContext::Node(r.clone())),
                RNone => Err(EvalErrorType::NoOutputNode),
            },
            (VarType::Inputs | VarType::InputsMap, Some(n)) => {
                nodes_func(n.lock().inputs().iter().cloned().collect())
            }
            (VarType::Outputs | VarType::OutputsMap, Some(n)) => {
                nodes_func(n.lock().outputs().iter().cloned().collect())
            }
            (_, None) => Err(EvalErrorType::NotANodeContext),
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
            VarType::Input => "input",
            VarType::Inputs => "inputs",
            VarType::InputsMap => "inputsmap",
            VarType::Output => "output",
            VarType::Outputs => "outputs",
            VarType::OutputsMap => "outputsmap",
            VarType::Nodes(p) => {
                return write!(f, "nodes{p}");
            }
            VarType::NodesMap(p) => {
                return write!(f, "nodesmap{p}");
            }
            VarType::Root => "root",
            VarType::Outlets => "outlets",
            VarType::OutletsMap => "outletsmap",
            VarType::Leaves => "leaves",
            VarType::LeavesMap => "leavesmap",
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
            .map(|a| format!("{} = {}", a.0, a.1))
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
        let node = match &self.ty {
            // if node name is given explicitely it overrides everything
            Some(VarType::Node(Some(n))) => Some(
                ctx.network
                    .node_by_name(n)
                    .ok_or(EvalErrorType::NodeNotFound(n.to_string()).pos(self.position()))?,
            ),
            _ => self.node.as_ref().or(node),
        };
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
    Series(Option<VarType>, bool, String),
    /// Series Mapped to a Function
    ///
    /// The functions should have same number of arguments as the
    /// number of series, they can have additional optional arguments
    SeriesMap(Option<VarType>, bool, String, MapFunction),
}

impl std::fmt::Display for SeriesExpression {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::AttrExpr(expr) => write!(f, "{expr}"),
            Self::Series(Some(vt), ts, srs) => {
                write!(f, "{vt}{}{srs}", if *ts { "$$" } else { "$" })
            }
            Self::Series(None, ts, srs) => write!(f, "{}{srs}", if *ts { "$$" } else { "$" }),
            Self::SeriesMap(Some(vt), ts, srs, mf) => {
                write!(f, "{vt}{}{srs} -> {mf}", if *ts { "$$" } else { "$" })
            }
            Self::SeriesMap(None, ts, srs, mf) => {
                write!(f, "{}{srs} -> {mf}", if *ts { "$$" } else { "$" })
            }
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
                _ => Err(EvalErrorType::NotAnArray.no_pos()),
            },
            Self::Series(ty, ts, sr) => get_series(ft, ctx, node, ty, sr, *ts),
            Self::SeriesMap(ty, ts, sr, func) => {
                let sr = get_series(ft, ctx, node, ty, sr, *ts)?;
                match func {
                    MapFunction::Defn(udf) => {
                        let func_call = |arg| {
                            // eval udf with node/network context
                            udf.eval_inline(
                                ft,
                                ctx,
                                FunctionCtx::from_arg_kwarg(vec![arg], HashMap::new()),
                                node,
                            )
                        };
                        sr.map_values(&func_call)
                    }
                    MapFunction::Pointer(name) => {
                        let func_call = |arg| {
                            let fctx = FunctionCtx::from_arg_kwarg(vec![arg], HashMap::new());
                            match ctx.udf(name).cloned() {
                                // priority for the locally defined function; evaluated in local context
                                Some(func) => func.eval(ctx, fctx),
                                _ => match ctx.functions.env(name) {
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
    ts: bool,
) -> Result<Series, EvalError> {
    let expr_ctx = match vt {
        Some(ty) => ty.get_expr_context(ctx, node)?,
        None => ft.get_expr_context(node)?,
    };
    match expr_ctx {
        ExprContext::Local => todo!(),
        ExprContext::Env => get_series_or_ts(&ctx.env, name, ts),
        ExprContext::Network => get_series_or_ts(&ctx.network, name, ts),
        ExprContext::Node(n) => {
            let n = &n
                .try_lock()
                .into_option()
                .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?;
            let n: &NodeInner = &n;
            get_series_or_ts(n, name, ts)
        }
        // Nodes series will error out on masked series with gaps, use nodesmap keywords
        ExprContext::Nodes(nds) => {
            let inp_series = nds
                .iter()
                .map(|i| get_node_series_or_ts(i, name, ts))
                .collect::<Result<Vec<Series>, EvalError>>()?;
            let sr_lengths: Vec<usize> = inp_series.iter().map(|s| s.len()).collect();
            if sr_lengths.is_empty() {
                return Err(EvalErrorType::NoInputNodes.no_pos());
            }
            if let Some(l) = sr_lengths.iter().find(|l| **l != sr_lengths[0]) {
                return Err(EvalErrorType::DifferentLength(sr_lengths[0], *l).no_pos());
            }
            let mut compl_series = Vec::with_capacity(inp_series.len());
            for ser in inp_series {
                match ser {
                    Series::Masked(ms, _) => {
                        if ms.has_gaps() {}
                        let cs = ms
                            .complete()
                            .ok_or(EvalErrorType::EmptySeriesValue.no_pos())?;
                        compl_series.push(cs.to_attributes().into_iter());
                    }
                    Series::Complete(cs) => compl_series.push(cs.to_attributes().into_iter()),
                }
            }
            let zipped_vals: Vec<_> = (0..sr_lengths[0])
                .map(|_| {
                    let mut dt = Vec::with_capacity(sr_lengths.len());
                    for s in &mut compl_series {
                        dt.push(s.next().expect("lengths already checked"));
                    }
                    Attribute::Array(dt.into())
                })
                .collect();
            Ok(CompleteSeries::attributes(zipped_vals).into())
        }
        ExprContext::NodesMap(nds) => {
            let inp_series = nds
                .iter()
                .map(|i| {
                    let sr = get_node_series_or_ts(i, name, ts)?;
                    Ok((i.lock().name().to_string(), sr))
                })
                .collect::<Result<Vec<(String, Series)>, EvalError>>()?;
            let sr_lengths: Vec<usize> = inp_series.iter().map(|(_, s)| s.len()).collect();
            if sr_lengths.is_empty() {
                return Err(EvalErrorType::NoInputNodes.no_pos());
            }
            if let Some(l) = sr_lengths.iter().find(|l| **l != sr_lengths[0]) {
                return Err(EvalErrorType::DifferentLength(sr_lengths[0], *l).no_pos());
            }
            let mut compl_series = Vec::with_capacity(inp_series.len());
            let mut masked_series = Vec::with_capacity(inp_series.len());
            for (inp, ser) in inp_series {
                match ser {
                    Series::Masked(ms, _) => {
                        masked_series.push((inp, ms.to_attributes().into_iter()))
                    }
                    Series::Complete(cs) => {
                        compl_series.push((inp, cs.to_attributes().into_iter()))
                    }
                }
            }
            let zipped_vals: Vec<_> = (0..sr_lengths[0])
                .map(|_| {
                    let mut dt = AttrMap::with_capacity(sr_lengths.len());
                    for (i, s) in &mut compl_series {
                        dt.insert(i.clone().into(), s.next().expect("lengths already checked"));
                    }
                    for (i, s) in &mut masked_series {
                        if let RSome(val) = s.next().expect("lengths already checked") {
                            dt.insert(i.clone().into(), val);
                        }
                    }
                    Attribute::Table(dt)
                })
                .collect();
            Ok(CompleteSeries::attributes(zipped_vals).into())
        }
    }
}

fn get_series_or_ts<T: HasSeries + HasTimeSeries>(
    n: &T,
    name: &str,
    ts: bool,
) -> Result<Series, EvalError> {
    if ts {
        n.try_ts(name)
            .map(|t| t.series())
            .map_err(|e| EvalErrorType::TimeSeriesNotFound(e).no_pos())
            .cloned()
    } else {
        n.try_series(name)
            .map_err(|e| EvalErrorType::SeriesNotFound(e).no_pos())
            .cloned()
    }
}

fn get_node_series_or_ts(n: &Node, name: &str, ts: bool) -> Result<Series, EvalError> {
    use std::ops::Deref;
    let node = n
        .try_lock()
        .into_option()
        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?;
    get_series_or_ts(node.deref(), name, ts).map_err(|e| e.node(node.name().to_string()))
}

/// Expression with some context
#[derive(Debug, Clone, PartialEq)]
pub struct ExprWithContext {
    pub ty: VarType,
    // TODO: make it a vec of expression later
    pub expr: Box<Expression>,
}

impl std::fmt::Display for ExprWithContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {{{}}}",
            self.ty,
            self.expr // .iter()
                      // .map(|e| e.to_string())
                      // .collect::<Vec<_>>()
                      // .join("\t\n")
        )
    }
}

impl ExprWithContext {
    pub fn new(ty: VarType, expr: Expression) -> Self {
        Self {
            ty,
            expr: Box::new(expr),
        }
    }

    pub fn resolve(
        &self,
        _ft: &FunctionType,
        ctx: &TaskContext,
        local: Option<&RHashMap<RString, Attribute>>,
        node: Option<&Node>,
    ) -> Result<Expression, EvalError> {
        let expr_ctx = self
            .ty
            .get_expr_context(ctx, node)
            .map_err(|e| e.no_pos())?;
        match expr_ctx {
            ExprContext::Local => todo!(),
            ExprContext::Node(n) => self.expr.resolve(&FunctionType::Node, ctx, local, Some(&n)),
            ExprContext::Nodes(nds) => {
                let exprs = nds
                    .iter()
                    .map(|n| self.expr.resolve(&FunctionType::Node, ctx, local, Some(&n)))
                    .collect::<Result<Vec<Expression>, EvalError>>()?;
                Ok(Expression::Array(exprs))
            }
            ExprContext::NodesMap(nds) => {
                let exprs = nds
                    .iter()
                    .map(|n| {
                        let name = n
                            .try_lock()
                            .into_option()
                            .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                            .name()
                            .to_string();
                        let expr = self
                            .expr
                            .resolve(&FunctionType::Node, ctx, local, Some(&n))?;
                        Ok((name, expr))
                    })
                    .collect::<Result<HashMap<String, Expression>, EvalError>>()?;
                Ok(Expression::Table(exprs))
            }
            ExprContext::Env => self.expr.resolve(&FunctionType::Env, ctx, local, node),
            ExprContext::Network => self.expr.resolve(&FunctionType::Network, ctx, local, node),
        }
    }
}

/// The context for an expression to be evaluated in
///
/// This is env by default, unless a keyword is used to change it
#[derive(Clone, Default)]
pub enum ExprContext {
    /// Local context
    Local,
    #[default]
    /// Environmental context
    Env,
    /// Network Context
    Network,
    /// Node context like node, input, output, root
    Node(Node),
    /// Multiple nodes context like inputs, outputs, outlets, leaves
    Nodes(Vec<Node>),
    /// Multiple nodes context with their names
    NodesMap(Vec<Node>),
}
