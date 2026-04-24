use crate::attrs::{AttrMap, Attribute};
use crate::expressions::{ExprContext, ExprResult, Position};
use crate::node::Node;
use crate::structs::NadiAttrType;
use crate::tasks::FunctionType;
use crate::tasks::TaskContext;
use crate::template::{Template, TemplateError};
use std::borrow::Cow;

#[derive(Clone, Debug)]
pub struct EvalCtx<'a> {
    pub(crate) expr_ctx: Cow<'a, ExprContext>,
}

impl<'a> PartialEq for EvalCtx<'a> {
    fn eq(&self, other: &Self) -> bool {
        self.expr_ctx.as_ref() == other.expr_ctx.as_ref()
    }
}

impl<'a> EvalCtx<'a> {
    pub fn to_owned(self) -> EvalCtx<'static> {
        EvalCtx {
            expr_ctx: match self.expr_ctx {
                Cow::Owned(o) => Cow::Owned(o),
                Cow::Borrowed(b) => Cow::Owned(b.clone()),
            },
        }
    }

    pub fn curr_node(&self) -> Option<&Node> {
        self.expr_ctx.as_ref().curr_node()
    }

    pub fn at_node(node: Node) -> EvalCtx<'static> {
        EvalCtx {
            expr_ctx: Cow::Owned(ExprContext::Node(node)),
        }
    }

    pub fn local() -> EvalCtx<'static> {
        EvalCtx {
            expr_ctx: Cow::Owned(ExprContext::Local),
        }
    }

    pub fn env() -> EvalCtx<'static> {
        EvalCtx {
            expr_ctx: Cow::Owned(ExprContext::Env),
        }
    }

    pub fn network() -> EvalCtx<'static> {
        EvalCtx {
            expr_ctx: Cow::Owned(ExprContext::Network),
        }
    }

    pub fn expr_ctx(expr_ctx: Cow<'a, ExprContext>) -> EvalCtx<'a> {
        EvalCtx { expr_ctx }
    }
}

impl Eval for Template {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let map_res = |res: Result<String, TemplateError>| match res {
            Ok(s) => Ok(ExprResult::Val(s.into())),
            Err(e) => Err(EvalErrorType::RenderError(e.to_string()).no_pos()),
        };
        match ectx.expr_ctx.as_ref() {
            ExprContext::Local => map_res(self.render(loc)),
            ExprContext::Env => map_res(self.render(&ctx.env)),
            ExprContext::Network => map_res(self.render(&ctx.network)),
            ExprContext::Node(n) => map_res(self.render(&n.lock())),
            ExprContext::Nodes(nds) => nds
                .iter()
                .map(|n| map_res(self.render(&n.lock())))
                .collect::<Result<Vec<ExprResult>, _>>()
                .map(ExprResult::Arr),
            ExprContext::NodesMap(nds) => nds
                .iter()
                .map(|n| {
                    let name = n.lock().name().to_string();
                    map_res(self.render(&n.lock())).map(|v| (name, v))
                })
                .collect::<Result<Vec<(String, ExprResult)>, _>>()
                .map(ExprResult::Map),
        }
    }
}

impl Default for EvalCtx<'_> {
    fn default() -> Self {
        Self {
            expr_ctx: Cow::Owned(ExprContext::default()),
        }
    }
}

pub trait Eval: Clone {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError>;

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        self.eval(ctx, ectx, loc)
    }

    fn eval_value(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<Attribute, EvalError> {
        self.eval(ctx, ectx, loc)?
            .to_attribute()
            .ok_or(EvalErrorType::EmptyValue(None).no_pos())
    }

    fn eval_mut_value(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<Attribute, EvalError> {
        self.eval_mut(ctx, ectx, loc)?
            .to_attribute()
            .ok_or(EvalErrorType::EmptyValue(None).no_pos())
    }

    fn nested(&self) -> bool {
        false
    }
}

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
                "{} {node} at Line {} Column {}: {}",
                self.ty.name(),
                pos.0,
                pos.1,
                self.ty.message()
            )
        } else {
            write!(f, "{} {node}: {}", self.ty.name(), self.ty.message())
        }
    }
}

impl From<EvalErrorType> for EvalError {
    fn from(val: EvalErrorType) -> EvalError {
        val.no_pos()
    }
}

impl EvalErrorType {
    pub fn at<T: Position>(self, obj: T) -> EvalError {
        EvalError {
            ty: self,
            position: vec![obj.position()],
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
    /// Function  didn't return a value to be used in expression
    NoReturnValue(String),
    /// The context is invalid
    InvalidContext(&'static str),
    /// Return Statement that returns a value, but if it's outside function this is error
    InvalidReturn(ExprResult),
    /// Break statement outside of for or while loop
    InvalidBreak(ExprResult),
    /// Continue statement outside of for or while loop
    InvalidContinue,
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
    /// The value was empty
    EmptyValue(Option<String>),
    /// TimeSeries with name doesn't exist
    TimeSeriesNotFound(String),
    /// Key not found in the table
    KeyError(String),
    /// Index out of range for the array
    IndexError,
    // AttributeNotFound(Option<String>, String),
    /// The node doesn't have input nodes (only used when not having inputs is a problem)
    NoInputNodes,
    /// The node doesn't have output node
    NoOutputNode,
    /// The node doesn't have edge node
    NoEdgeNode,
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
    pub fn name(&self) -> &'static str {
        match self {
            Self::UserError(_) => "UserError",
            Self::UnresolvedVariable => "UnresolvedVariableError",
            Self::FunctionNotFound(..) => "FunctionNotFoundError",
            Self::FunctionError(..) => "FunctionError",
            Self::UnknownFunctionType => "UnknownFunctionTypeError",
            Self::NoReturnValue(_) => "NoReturnValueError",
            Self::InvalidContext(_) => "InvalidContextError",
            Self::InvalidReturn(_) => "InvalidReturnError",
            Self::InvalidBreak(_) => "InvalidBreakError",
            Self::InvalidContinue => "InvalidContinueError",
            Self::NodeNotFound(_) => "NodeNotFoundError",
            Self::NotANodeContext => "NotANodeContextError",
            Self::PathNotFound(..) => "PathNotFoundError",
            Self::AttributeNotFound => "AttributeNotFoundError",
            Self::SeriesNotFound(_) => "SeriesNotFoundError",
            Self::TimeSeriesNotFound(_) => "TimeSeriesNotFoundError",
            Self::EmptyValue(_) => "EmptyValueError",
            Self::KeyError(_) => "KeyError",
            Self::IndexError => "IndexError",
            Self::NoInputNodes => "NoInputNodesError",
            Self::NoOutputNode => "NoOutputNodeError",
            Self::NoEdgeNode => "NoEdgeNodeError",
            Self::NoRootNode => "NoRootNodeError",
            Self::AttributeError(_) => "AttributeError",
            Self::NodeAttributeError(..) => "NodeAttributeError",
            Self::InvalidOperation => "InvalidOperationError",
            Self::InvalidVariableType => "InvalidVariableTypeError",
            Self::InvalidAttributeType(..) => "InvalidAttributeTypeError",
            Self::NotAnArray => "NotAnArrayError",
            Self::NotANumber => "NotANumberError",
            Self::NotABool => "NotABoolError",
            Self::DifferentLength(..) => "DifferentLengthError",
            Self::DivideByZero => "DivideByZeroError",
            Self::MaxIteratorError(_) => "MaxIteratorError",
            Self::RenderError(_) => "RenderError",
            Self::RegexError(_) => "RegexError",
            Self::ParseError(_) => "ParseError",
            Self::LogicalError(_) => "LogicalError",
            Self::NotImplementedError(_) => "NotImplementedError",
            Self::MutexError(..) => "MutexError",
        }
    }

    /// Format the error into a message using the values
    pub fn message(&self) -> String {
        match self {
            Self::UserError(s) => return s.to_string(),
            Self::UnresolvedVariable => "Could not resolve variable in expression",
            Self::FunctionNotFound(t, n) => {
                return format!(
                    "{} function named {n:?} not found",
                    t.as_ref().map(|t| t.name()).unwrap_or("Any")
                );
            }
            Self::FunctionError(n, s) => return format!("function {n}: {s}"),
            Self::UnknownFunctionType => "Unknown function type",
            Self::NoReturnValue(n) => return format!("Function {n} did not return a value"),
            Self::InvalidContext(s) => return s.to_string(),
            // if return is inside a function it is caught and the value is returned
            Self::InvalidReturn(_) => "Return statement outside of function",
            Self::InvalidBreak(_) => "Break statement outside of loop",
            Self::InvalidContinue => "Continue statement outside of loop",
            Self::NodeNotFound(n) => return format!("Node {n:?} not found"),
            Self::NotANodeContext => "currently not inside a node context",
            Self::PathNotFound(s, e, t) => {
                return format!("No path found between Nodes {s:?} and {t:?}, path ends at {e:?}");
            }
            Self::AttributeNotFound => "Attribute not found",
            Self::SeriesNotFound(msg) => return msg.to_string(),
            Self::TimeSeriesNotFound(msg) => return msg.to_string(),
            Self::EmptyValue(Some(v)) => return format!("Value for {v:?} is not set"),
            Self::EmptyValue(None) => "the expression resulted in empty value",
            Self::KeyError(k) => return format!("Key {k:?} not found"),
            Self::IndexError => "Array index out of range",
            Self::NoInputNodes => "Node doesn't have an input node or has multiple",
            Self::NoOutputNode => "Node doesn't have an output node or has multiple",
            Self::NoEdgeNode => "Node doesn't have an edge node or has multiple",
            Self::NoRootNode => "Network doesn't have a single root node",
            Self::AttributeError(s) => return s.to_string(),
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
            Self::DivideByZero => "Division by Zero not supported for integers, use float",
            Self::MaxIteratorError(n) => {
                return format!("Loop did not exit after {n} iterations, could be infinite loop");
            }
            Self::RenderError(e) => return e.to_string(),
            Self::RegexError(e) => return e.to_string(),
            Self::ParseError(e) => return e.to_string(),
            Self::LogicalError(s) => {
                return format!("{s}. Contact developer, this should be fixed.");
            }
            Self::NotImplementedError(s) => {
                return format!("{s}, this feature is planned for future versions");
            }
            Self::MutexError(f, l) => {
                return format!("file: {f}::{l}, multiple mutation attempted at the same time. If you think this should not happen, contact developer");
            }
        }
        .to_string()
    }
}
