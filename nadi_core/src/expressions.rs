use crate::attrs::{AttrMap, Attribute, FromAttribute, HasAttributes};
use crate::eval::{Eval, EvalCtx, EvalError, EvalErrorType};
use crate::functions::FunctionCtx;
use crate::network::Propagation;
use crate::node::{Node, NodeInner};
use crate::structs::NadiAttrType;
use crate::tasks::{FunctionType, Task, TaskContext, TaskCtxConsts, TaskKeyword, TaskMessage};
use crate::template::Template;
use crate::timeseries::{CompleteSeries, HasSeries, HasTimeSeries, Series};
use crate::udf::UserFunction;
use abi_stable::std_types::{RNone, RSome, RString};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;

pub static NONE_VALUE: &str = "<None>";

pub trait Position {
    fn position(&self) -> (usize, usize);
}

#[derive(Debug, Clone, PartialEq)]
pub struct RawExpr {
    expr: ExprType<RawExpr>,
    position: (usize, usize),
}

impl Eval for RawExpr {
    fn nested(&self) -> bool {
        self.expr.is_nested()
    }

    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let res = self
            .clone()
            .resolve(ctx, ectx.clone())
            .map_err(|e| e.pos(self.position()))?;
        res.eval(ctx, ectx).map_err(|e| e.pos(self.position()))
    }

    fn eval_mut(&self, ctx: &mut TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let res = self
            .clone()
            .resolve(ctx, ectx.clone())
            .map_err(|e| e.pos(self.position()))?;
        res.eval_mut(ctx, ectx).map_err(|e| e.pos(self.position()))
    }
}

impl std::fmt::Display for RawExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.expr, f)
    }
}

impl RawExpr {
    pub fn resolve<'a>(
        self,
        ctx: &TaskContext,
        ectx: EvalCtx<'a>,
    ) -> Result<ResolvedExpr<'a>, EvalError> {
        let expr: ExprType<ResolvedExpr> = match self.expr {
            ExprType::None => ExprType::None,
            ExprType::Value(r) => ExprType::Value(r),
            ExprType::Result(r) => ExprType::Result(r),
            ExprType::Progress(p) => ExprType::Progress(ExprProgress::new(
                p.label.resolve(ctx, ectx.clone())?,
                p.prog.resolve(ctx, ectx.clone())?,
                p.total.resolve(ctx, ectx.clone())?,
            )),
            ExprType::Function(fc) => {
                ExprType::Function(resolve_function_calls(fc, ctx, ectx.clone())?)
            }
            ExprType::Var(vt) => ExprType::Var(vt),
            ExprType::SetVar(sv) => return resolve_set_variable(sv, ctx, ectx.clone()),
            ExprType::IfElse(cond, expr1, Some(expr2)) => ExprType::IfElse(
                Box::new(cond.resolve(ctx, ectx.clone())?),
                Box::new(expr1.resolve(ctx, ectx.clone())?),
                Some(Box::new(expr2.resolve(ctx, ectx.clone())?)),
            ),
            ExprType::IfElse(cond, expr1, None) => ExprType::IfElse(
                Box::new(cond.resolve(ctx, ectx.clone())?),
                Box::new(expr1.resolve(ctx, ectx.clone())?),
                None,
            ),
            ExprType::While(cond, expr1) => ExprType::While(
                Box::new(cond.resolve(ctx, ectx.clone())?),
                Box::new(expr1.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Loop(expr1) => ExprType::Loop(Box::new(expr1.resolve(ctx, ectx.clone())?)),
            ExprType::TryCatch(expr1, expr2) => ExprType::TryCatch(
                Box::new(expr1.resolve(ctx, ectx.clone())?),
                Box::new(expr2.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Multi(exprs) => ExprType::Multi(
                exprs
                    .into_iter()
                    .map(|e| e.resolve(ctx, ectx.clone()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ExprType::Array(exprs) => ExprType::Array(
                exprs
                    .into_iter()
                    .map(|e| e.resolve(ctx, ectx.clone()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ExprType::Map(exprs) => ExprType::Map(
                exprs
                    .into_iter()
                    .map(|(k, e)| e.resolve(ctx, ectx.clone()).map(|v| (k, v)))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ExprType::Silent(e) => ExprType::Silent(Box::new(e.resolve(ctx, ectx.clone())?)),
            ExprType::Check(e) => match e.resolve(ctx, ectx.clone()) {
                Ok(r) => ExprType::Check(Box::new(r)),
                Err(_) => ExprType::Result(ExprResult::Val(false.into())),
            },
            ExprType::WithContext(e) => return resolve_expr_w_ctx(e, ctx, ectx.clone()),
            ExprType::Range(b, Some(s), e) => ExprType::Range(
                Box::new(b.resolve(ctx, ectx.clone())?),
                Some(Box::new(s.resolve(ctx, ectx.clone())?)),
                Box::new(e.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Range(b, None, e) => ExprType::Range(
                Box::new(b.resolve(ctx, ectx.clone())?),
                None,
                Box::new(e.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Render(t) => ExprType::Render(t),
            ExprType::UserError(s) => ExprType::UserError(s),
            ExprType::UniOp(op, expr) => {
                ExprType::UniOp(op, Box::new(expr.resolve(ctx, ectx.clone())?))
            }
            ExprType::BiOp(op, expr1, expr2) => ExprType::BiOp(
                op,
                Box::new(expr1.resolve(ctx, ectx.clone())?),
                Box::new(expr2.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Return(None) => ExprType::Return(None),
            ExprType::Return(Some(expr)) => {
                ExprType::Return(Some(Box::new(expr.resolve(ctx, ectx.clone())?)))
            }
            ExprType::Break(None) => ExprType::Break(None),
            ExprType::Break(Some(expr)) => {
                ExprType::Break(Some(Box::new(expr.resolve(ctx, ectx.clone())?)))
            }
            ExprType::Continue => ExprType::Continue,
            _ => {
                return Err(EvalErrorType::NotImplementedError(
                    "resolving logic not implemented yet for these",
                )
                .no_pos());
            }
        };
        Ok(ResolvedExpr {
            expr,
            position: self.position,
            context: ectx.clone(),
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedExpr<'a> {
    expr: ExprType<ResolvedExpr<'a>>,
    position: (usize, usize),
    context: EvalCtx<'a>,
}

impl std::fmt::Display for ResolvedExpr<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.expr, f)
    }
}

impl RawExpr {
    pub fn new(expr: ExprType<Self>, position: (usize, usize)) -> Self {
        Self { expr, position }
    }
}

impl Position for RawExpr {
    fn position(&self) -> (usize, usize) {
        self.position
    }
}

impl ResolvedExpr<'_> {
    pub fn err_ctx(&self, err: EvalError) -> EvalError {
        err.pos(self.position)
    }

    // pub fn to_owned(self) -> ResolvedExpr<'static> {
    //     ResolvedExpr {
    //         expr: self.expr.to_owned(),
    //         position: self.position,
    //         context: self.context.to_owned(),
    //     }
    // }
}

impl Position for ResolvedExpr<'_> {
    fn position(&self) -> (usize, usize) {
        self.position
    }
}

impl Eval for ResolvedExpr<'_> {
    fn eval_mut(&self, ctx: &mut TaskContext, _ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        self.expr
            .eval_mut(ctx, &self.context)
            .map_err(|e| self.err_ctx(e))
    }

    fn eval(&self, ctx: &TaskContext, _ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        self.expr
            .eval(ctx, &self.context)
            .map_err(|e| self.err_ctx(e))
    }

    fn nested(&self) -> bool {
        self.expr.is_nested()
    }
}

/// Result of an expression, because it could involve None values
#[derive(Debug, Clone, PartialEq)]
pub enum ExprResult {
    /// Empty value
    None,
    /// Empty due to error, show as None, function as error
    ///
    /// If a value is to be used somewhere, then it will be treated as
    /// error, but if simply shown in the top most expression this
    /// acts as None
    NoneErr(Box<EvalError>),
    /// Valid value
    Val(Attribute),
    /// Image output
    Image(String),
    /// Multiple Images output
    Images(Vec<String>),
    /// File output
    File(String),
    /// Documentation
    Doc(String),
    /// Array of results
    Arr(Vec<ExprResult>),
    /// Map of results
    Map(Vec<(String, ExprResult)>),
}

impl From<Option<Attribute>> for ExprResult {
    fn from(val: Option<Attribute>) -> Self {
        match val {
            Some(a) => Self::Val(a),
            None => Self::None,
        }
    }
}

impl std::fmt::Display for ExprResult {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", NONE_VALUE),
            Self::NoneErr(_) => write!(f, "{}", NONE_VALUE),
            Self::Val(a) => write!(f, "{a}"),
            Self::Image(s) => write!(f, "<image:{s:?}>"),
            Self::Images(s) => write!(f, "<images:{s:?}>"),
            Self::File(s) => write!(f, "<file:{s:?}>"),
            Self::Doc(s) => write!(f, "<doc:{s:?}>"),
            Self::Arr(ar) => write!(
                f,
                "[{}]",
                ar.iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::Map(am) => write!(
                f,
                "{{{}}}",
                am.iter()
                    .map(|(k, v)| format!("{k} = {v}"))
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
        }
    }
}

impl ExprResult {
    pub fn to_attribute(self) -> Option<Attribute> {
        match self {
            Self::None | Self::NoneErr(_) => None,
            Self::Val(a) => Some(a),
            Self::Image(s) => Some(s.into()),
            Self::Images(s) => Some(s.into()),
            Self::File(s) => Some(s.into()),
            Self::Doc(s) => Some(s.into()),
            Self::Arr(ar) => ar
                .into_iter()
                .map(|a| a.to_attribute())
                .collect::<Option<Vec<Attribute>>>()
                .map(|a| Attribute::Array(a.into())),
            Self::Map(am) => am
                .into_iter()
                .map(|(k, a)| a.to_attribute().map(|a| (k.into(), a)))
                .collect::<Option<HashMap<RString, Attribute>>>()
                .map(|a| Attribute::Table(a.into())),
        }
    }

    pub fn value(self) -> Result<Attribute, EvalError> {
        if let Self::NoneErr(e) = self {
            Err(*e)
        } else {
            self.to_attribute()
                .ok_or(EvalErrorType::EmptyValue(None).no_pos())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum ExprType<T: std::fmt::Display + std::fmt::Debug + Clone + PartialEq> {
    /// Empty Value
    None,
    /// Literal Value
    Value(Attribute),
    /// Result of another expression
    Result(ExprResult),
    /// Progress signal
    Progress(ExprProgress<T>),
    /// get variable
    Var(InputVar),
    /// set variable (assignment)
    SetVar(SetVariable<T>),
    /// render a string template
    Render(Template),
    /// unary operator
    UniOp(UniOperator, Box<T>),
    /// Binary operator
    BiOp(BiOperator, Box<T>, Box<T>),
    #[cfg(feature = "parser")]
    /// import other tasks file
    Import(ImportExpr),
    /// if else statemnet
    IfElse(Box<T>, Box<T>, Option<Box<T>>),
    /// while loop
    While(Box<T>, Box<T>),
    /// generic loop
    Loop(Box<T>),
    /// for loop
    ForEach(String, Box<T>, Box<T>),
    /// array expression
    Array(Vec<T>),
    /// array generator expression
    ArrayGen(Box<T>, String, Box<T>),
    /// map expression
    Map(Vec<(String, T)>),
    /// map generator expression
    MapGen(Box<T>, Box<T>, String, String, Box<T>),
    /// context block
    WithContext(ExprWithContext<T>),
    /// integer range
    Range(Box<T>, Option<Box<T>>, Box<T>),
    /// user error raising
    UserError(String),
    /// function calls
    Function(FunctionCall<T>),
    /// silenced expression
    Silent(Box<T>),
    /// Check if expression returns a valid value
    Check(Box<T>),
    /// try catch block
    TryCatch(Box<T>, Box<T>),
    /// return statement
    Return(Option<Box<T>>),
    /// break statement
    Break(Option<Box<T>>),
    /// continue statement
    Continue,
    /// multiple expressions
    Multi(Vec<T>),
    /// Series as Attributes
    Series(Option<VarType>, bool, String),
    /// Value from a series
    SeriesValue(Option<VarType>, bool, String, usize),
}

impl<T: std::fmt::Display + std::fmt::Debug + Clone + PartialEq + Eval> std::fmt::Display
    for ExprType<T>
{
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::None => write!(f, "{}", NONE_VALUE),
            Self::Value(p) => std::fmt::Display::fmt(p, f),
            Self::Result(r) => std::fmt::Display::fmt(r, f),
            Self::Progress(p) => std::fmt::Display::fmt(p, f),
            Self::Var(v) => std::fmt::Display::fmt(v, f),
            Self::SetVar(v) => std::fmt::Display::fmt(v, f),
            Self::Render(v) => write!(f, "r{v:?}"),
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
            #[cfg(feature = "parser")]
            Self::Import(i) => std::fmt::Display::fmt(i, f),
            Self::IfElse(cond, expr1, expr2) => {
                if let Some(expr2) = expr2 {
                    write!(f, "if ({}) {{{}}} else {{{}}}", cond, expr1, expr2)
                } else {
                    write!(f, "if ({}) {{{}}}", cond, expr1)
                }
            }
            Self::While(cond, expr) => {
                write!(f, "while ({}) {{{}}}", cond, expr)
            }
            Self::Loop(expr) => {
                write!(f, "loop {{{}}}", expr)
            }
            Self::ForEach(var, expr1, expr2) => {
                write!(
                    f,
                    "for {var} in {} {{{}}}",
                    if expr1.nested() {
                        format!("({})", expr1)
                    } else {
                        expr1.to_string()
                    },
                    expr2
                )
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
            Self::ArrayGen(expr1, v, expr2) => write!(f, "[{} for {} in {}]", expr1, v, expr2),
            Self::Map(exprs) => {
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
            Self::MapGen(expr1, expr2, v1, v2, expr) => {
                write!(f, "{{{}={} for {},{} in {}}}", expr1, expr2, v1, v2, expr)
            }
            Self::WithContext(e) => write!(f, "{e}"),
            Self::Range(b, s, e) => {
                if let Some(s) = s {
                    write!(f, "{b}:{s}:{e}")
                } else {
                    write!(f, "{b}:{e}")
                }
            }
            Self::Function(fc) => std::fmt::Display::fmt(fc, f),
            Self::UserError(e) => write!(f, "error {:?}", e),
            Self::Silent(e) => write!(f, "{e};"),
            Self::Check(e) => {
                if e.nested() {
                    write!(f, "({e})?")
                } else {
                    write!(f, "{e}?")
                }
            }
            Self::TryCatch(expr1, expr2) => write!(f, "try {{{}}} catch {{{}}}", expr1, expr2),
            Self::Return(None) => write!(f, "return"),
            Self::Return(Some(expr)) => write!(f, "return {expr}"),
            Self::Break(None) => write!(f, "break"),
            Self::Break(Some(expr)) => write!(f, "break {expr}"),
            Self::Continue => write!(f, "continue"),
            Self::Multi(exprs) => {
                write!(
                    f,
                    "{}",
                    exprs
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            }
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

impl Eval for ExprType<ResolvedExpr<'_>> {
    fn eval_mut(&self, ctx: &mut TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        match self {
            Self::Function(fc) => Ok(fc.eval_mut(ctx, ectx)?),
            Self::SetVar(sv) => {
                sv.eval_mut(ctx, ectx)?;
                Ok(ExprResult::None)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ctx, ectx)?;
                let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                if cond {
                    expr1.eval_mut(ctx, ectx)
                } else if let Some(expr2) = expr2 {
                    expr2.eval_mut(ctx, ectx)
                } else {
                    Ok(ExprResult::None)
                }
            }
            Self::While(cond, expr) => {
                let max_it = TaskCtxConsts::max_iterations(ctx);
                let mut it = 0;
                loop {
                    let cond = cond.eval_value(ctx, ectx)?;
                    let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                    if cond {
                        match expr.eval_mut(ctx, ectx) {
                            Ok(_) => (),
                            Err(e) => match e.ty {
                                EvalErrorType::InvalidBreak(b) => return Ok(b),
                                EvalErrorType::InvalidContinue => continue,
                                _ => return Err(e),
                            },
                        }
                    } else {
                        break;
                    }
                    it += 1;
                    if it > max_it {
                        // basically to prevent infinite loop from user
                        return Err(EvalErrorType::MaxIteratorError(it).no_pos());
                    }
                }
                Ok(ExprResult::None)
            }
            Self::Loop(expr) => {
                let max_it = TaskCtxConsts::max_iterations(ctx);
                let mut it = 0;
                loop {
                    // only breaks through the break statement in code
                    match expr.eval_mut(ctx, ectx) {
                        Ok(_) => (),
                        Err(e) => match e.ty {
                            EvalErrorType::InvalidBreak(b) => return Ok(b),
                            EvalErrorType::InvalidContinue => continue,
                            _ => return Err(e),
                        },
                    }
                    it += 1;
                    if it > max_it {
                        // basically to prevent infinite loop from user
                        return Err(EvalErrorType::MaxIteratorError(it).no_pos());
                    }
                }
            }
            Self::TryCatch(expr1, expr2) => match expr1.eval_mut(ctx, ectx) {
                Ok(val) => Ok(val),
                _ => expr2.eval_mut(ctx, ectx),
            },
            Self::Multi(exprs) => {
                let mut res = ExprResult::None;
                for e in exprs {
                    res = e.eval_mut(ctx, ectx)?;
                }
                Ok(res)
            }
            Self::Array(exprs) => {
                let vals: Vec<ExprResult> = exprs
                    .iter()
                    .map(|v| v.eval_mut(ctx, ectx))
                    .collect::<Result<_, _>>()?;
                Ok(ExprResult::Arr(vals))
            }
            Self::Map(exprs) => {
                let vals: Vec<(String, ExprResult)> = exprs
                    .iter()
                    .map(|(k, v)| v.eval_mut(ctx, ectx).map(|v| (k.to_string(), v)))
                    .collect::<Result<_, _>>()?;
                Ok(ExprResult::Map(vals))
            }
            Self::Silent(e) => {
                e.eval_mut(ctx, ectx)?;
                Ok(ExprResult::None)
            }
            Self::Check(e) => Ok(ExprResult::Val(
                match e.eval_mut(ctx, ectx) {
                    Ok(ExprResult::None) | Ok(ExprResult::NoneErr(_)) | Err(_) => false,
                    Ok(_) => true,
                }
                .into(),
            )),
            Self::WithContext(e) => e.expr.eval_mut(ctx, &e.expr.context),
            _ => self.eval(ctx, ectx),
        }
    }

    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        match self {
            Self::None => Ok(ExprResult::None),
            Self::Value(r) => Ok(ExprResult::Val(r.clone())),
            Self::Result(r) => Ok(r.clone()),
            Self::Progress(p) => {
                p.eval(ctx, ectx)?;
                Ok(ExprResult::None)
            }
            Self::Function(fc) => Ok(fc.eval(ctx, ectx)?),
            Self::Var(vt) => Ok(vt.eval(ctx, ectx)?),
            Self::SetVar(sv) => {
                sv.eval(ctx, ectx)?;
                Ok(ExprResult::None)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ctx, ectx)?;
                let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                if cond {
                    expr1.eval(ctx, ectx)
                } else if let Some(expr2) = expr2 {
                    expr2.eval(ctx, ectx)
                } else {
                    Ok(ExprResult::None)
                }
            }
            Self::While(_, _) | Self::Loop(_) => Err(EvalErrorType::LogicalError(
                "while and loop in immutable context leads into an infinite loop",
            )
            .no_pos()),
            Self::TryCatch(expr1, expr2) => match expr1.eval(ctx, ectx) {
                Ok(val) => Ok(val),
                _ => expr2.eval(ctx, ectx),
            },
            Self::Multi(exprs) => {
                let mut res = ExprResult::None;
                for e in exprs {
                    res = e.eval(ctx, ectx)?;
                }
                Ok(res)
            }
            Self::Array(exprs) => {
                let vals: Vec<ExprResult> = exprs
                    .iter()
                    .map(|v| v.eval(ctx, ectx))
                    .collect::<Result<_, _>>()?;
                Ok(ExprResult::Arr(vals))
            }
            Self::Map(exprs) => {
                let vals: Vec<(String, ExprResult)> = exprs
                    .iter()
                    .map(|(k, v)| v.eval(ctx, ectx).map(|v| (k.to_string(), v)))
                    .collect::<Result<_, _>>()?;
                Ok(ExprResult::Map(vals))
            }
            Self::Silent(e) => {
                e.eval(ctx, ectx)?;
                Ok(ExprResult::None)
            }
            Self::Check(e) => Ok(ExprResult::Val(
                match e.eval(ctx, ectx) {
                    Ok(ExprResult::None) | Ok(ExprResult::NoneErr(_)) | Err(_) => false,
                    Ok(_) => true,
                }
                .into(),
            )),
            // This is already resolved context, so it is fine to just evaluate
            Self::WithContext(e) => e.expr.eval(ctx, &e.expr.context),
            Self::Range(b, s, e) => {
                let b = b.eval_value(ctx, ectx)?;
                let b = i64::from_attr(&b).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    b.dtype(),
                ))?;
                let e = e.eval_value(ctx, ectx)?;
                let e = i64::from_attr(&e).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    e.dtype(),
                ))?;
                let s = if let Some(s) = s {
                    let s = s.eval_value(ctx, ectx)?;
                    usize::from_attr(&s).ok_or(EvalErrorType::InvalidAttributeType(
                        NadiAttrType::Integer,
                        s.dtype(),
                    ))?
                } else {
                    1
                };
                let vals: Vec<_> = (b..=e).step_by(s).map(Attribute::Integer).collect();
                Ok(ExprResult::Val(Attribute::Array(vals.into())))
            }
            Self::Render(t) => t.eval(ctx, ectx),
            Self::UserError(s) => Err(EvalErrorType::UserError(s.clone()).no_pos()),
            Self::UniOp(op, expr) => op.eval(expr.eval_value(ctx, ectx)?).map(ExprResult::Val),
            Self::BiOp(op, expr1, expr2) => {
                let first = expr1.eval_value(ctx, ectx)?;
                // short circuit logical operations to prevent eval error
                match (op, &first) {
                    (BiOperator::And, Attribute::Bool(false)) => {
                        return Ok(ExprResult::Val(false.into()));
                    }
                    (BiOperator::Or, Attribute::Bool(true)) => {
                        return Ok(ExprResult::Val(true.into()));
                    }
                    _ => (),
                }
                op.eval(first, expr2.eval_value(ctx, ectx)?)
                    .map(ExprResult::Val)
            }
            Self::Return(None) => Err(EvalErrorType::InvalidReturn(ExprResult::None).no_pos()),
            Self::Return(Some(expr)) => {
                let ret = expr.eval(ctx, ectx)?;
                Err(EvalErrorType::InvalidReturn(ret).no_pos())
            }
            Self::Break(None) => Err(EvalErrorType::InvalidBreak(ExprResult::None).no_pos()),
            Self::Break(Some(expr)) => {
                let ret = expr.eval(ctx, ectx)?;
                Err(EvalErrorType::InvalidBreak(ret).no_pos())
            }
            Self::Continue => Err(EvalErrorType::InvalidContinue.no_pos()),
            _ => Err(
                EvalErrorType::NotImplementedError("this expression is not implemented").no_pos(),
            ),
        }
    }

    /// check if the expression is nested (needs parenthesis)
    fn nested(&self) -> bool {
        self.is_nested()
    }
}

impl<T: std::fmt::Display + std::fmt::Debug + Clone + PartialEq + Eval> ExprType<T> {
    /// check if the expression is nested (needs parenthesis)
    fn is_nested(&self) -> bool {
        match self {
            Self::Silent(e) => e.nested(),
            Self::UniOp(_, _) => true,
            Self::BiOp(_, _, _) => true,
            Self::IfElse(_, _, _) => true,
            Self::TryCatch(_, _) => true,
            Self::Multi(_) => true,
            _ => false,
        }
    }
}

#[cfg(feature = "parser")]
/// Expression that is an import statement
#[derive(Clone, PartialEq, Debug)]
pub struct ImportExpr {
    /// name of the plugin/nadi file
    pub name: String,
    /// path to the plugin/nadi code
    pub path: Option<PathBuf>,
    // /// Functions to import
    // functions: ImportFunctions,
    /// Execute tasks while importing functions
    pub tasks: bool,
}

#[cfg(feature = "parser")]
impl std::fmt::Display for ImportExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let key = if self.tasks { "exec" } else { "import" };
        if let Some(p) = &self.path {
            write!(f, "{key} {} from {p:?}", self.name)
        } else {
            write!(f, "{key} {}", self.name)
        }
    }
}

#[cfg(feature = "parser")]
impl ImportExpr {
    pub fn path(&self) -> Option<PathBuf> {
        if let Some(p) = &self.path {
            return Some(p.clone());
        }
        let name = &self.name;
        let path = PathBuf::from(format!("{name}.tasks"));
        if path.exists() {
            return Some(path);
        }
        None
    }
}

#[cfg(feature = "parser")]
impl Eval for ImportExpr {
    fn eval(&self, _ctx: &TaskContext, _ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        Err(EvalErrorType::InvalidContext("this needs to be run in mutable context").no_pos())
    }
    fn eval_mut(&self, ctx: &mut TaskContext, _ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        if let Some(path) = self.path() {
            let txt = std::fs::read_to_string(path).unwrap();
            let tokens = crate::parser::tokenizer::get_tokens(&txt);
            let tasks = crate::parser::tasks::parse(tokens)
                .map_err(|e| EvalErrorType::ParseError(e.to_string()).no_pos())?;
            if self.tasks {
                for fc in tasks {
                    ctx.execute(fc)?;
                }
            } else {
                for mut fc in tasks {
                    let mut exec = false;
                    if let Task::Function(fc) = &mut fc {
                        if let Some(name) = &mut fc.name {
                            *name = format!("{}.{}", self.name, name);
                            exec = true;
                        }
                    }
                    if exec {
                        ctx.execute(fc)?;
                    }
                }
            }
            Ok(ExprResult::None)
        } else {
            // In this case look at the available plugins and
            // load the functions from there to this context.
            Err(EvalErrorType::NotImplementedError("import other than tasks").no_pos())
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExprProgress<T> {
    label: Box<T>,
    prog: Box<T>,
    total: Box<T>,
}

impl<T> ExprProgress<T> {
    pub fn new(label: T, prog: T, total: T) -> Self {
        Self {
            label: Box::new(label),
            prog: Box::new(prog),
            total: Box::new(total),
        }
    }
}

impl<T: Clone + Eval> Eval for ExprProgress<T> {
    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let label = self.label.eval_value(ctx, ectx)?;
        let prog = self.prog.eval_value(ctx, ectx)?;
        let total = self.total.eval_value(ctx, ectx)?;

        let label = String::try_from_attr(&label)
            .map_err(EvalErrorType::AttributeError)
            .map_err(EvalErrorType::no_pos)?;
        let prog = usize::try_from_attr(&prog)
            .map_err(EvalErrorType::AttributeError)
            .map_err(EvalErrorType::no_pos)?;
        let total = usize::try_from_attr(&total)
            .map_err(EvalErrorType::AttributeError)
            .map_err(EvalErrorType::no_pos)?;
        _ = ctx.channel.send(TaskMessage::Progress(label, prog, total));
        Ok(ExprResult::None)
    }
}

impl<T: std::fmt::Display> std::fmt::Display for ExprProgress<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "progress {} = {} in {}",
            self.label, self.prog, self.total
        )
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
    pub fn expr_from_stack(self, stack: &mut Vec<RawExpr>) -> Option<RawExpr> {
        let right = stack.pop()?;
        let left = stack.pop()?;
        let pos = left.position();
        Some(RawExpr::new(
            ExprType::BiOp(self, Box::new(left), Box::new(right)),
            pos,
        ))
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
            (Self::Str(s), Attribute::Table(am)) => {
                am.attr(s).ok_or(EvalErrorType::KeyError(s.to_string()))
            }
            (Self::Int(i), Attribute::Array(ar)) => ar.get(*i).ok_or(EvalErrorType::IndexError),
            (Self::Str(_), a) => Err(EvalErrorType::InvalidAttributeType(
                NadiAttrType::Table,
                a.dtype(),
            )),
            (Self::Int(_), a) => Err(EvalErrorType::InvalidAttributeType(
                NadiAttrType::Array,
                a.dtype(),
            )),
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
            (Self::Str(_), a) => Err(EvalErrorType::InvalidAttributeType(
                NadiAttrType::Table,
                a.dtype(),
            )),
            (Self::Int(_), a) => Err(EvalErrorType::InvalidAttributeType(
                NadiAttrType::Array,
                a.dtype(),
            )),
        }
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Self::Str(_) => "string",
            Self::Int(_) => "integer",
        }
    }

    pub fn name(&self) -> String {
        match self {
            Self::Str(s) => s.to_string(),
            Self::Int(i) => i.to_string(),
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
    /// start position of the variable
    pub start: (usize, usize),
}

impl Position for InputVar {
    fn position(&self) -> (usize, usize) {
        self.start
    }
}

impl std::fmt::Display for InputVar {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
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
        )
    }
}

impl InputVar {
    /// new input variable
    pub fn new(
        ty: Option<VarType>,
        name: String,
        indices: Vec<InputVarIndex>,
        start: (usize, usize),
    ) -> Self {
        Self {
            ty,
            name,
            indices,
            start,
        }
    }

    /// Get the attribute value based on the nested indices
    pub fn attr_nested<T: HasAttributes>(
        &self,
        attrmap: &T,
    ) -> Result<Option<Attribute>, EvalErrorType> {
        let mut at = match attrmap.attr(&self.name) {
            None if self.indices.is_empty() => return Ok(None),
            None => {
                return Err(EvalErrorType::AttributeError(format!(
                    "Attribute {} not found",
                    self.name
                )));
            }
            Some(a) => a,
        };
        match self.indices.as_slice() {
            [] => Ok(Some(at.clone())),
            [pre @ .., last] => {
                for ind in pre {
                    at = ind.index(at)?;
                }
                match last.index(at) {
                    Ok(a) => Ok(Some(a.clone())),
                    Err(EvalErrorType::KeyError(_)) | Err(EvalErrorType::IndexError) => Ok(None),
                    Err(e) => Err(e),
                }
            }
        }
    }

    /// Set the attribute to the value based on the nested indices
    pub fn set_attr_nested<T: HasAttributes>(
        &self,
        attrmap: &mut T,
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
}

impl Eval for InputVar {
    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        // if type is none then first priotize local
        if self.ty.is_none() {
            if let Some(l) = &ectx.local {
                if let Ok(Some(v)) = self.attr_nested(l.as_ref()) {
                    return Ok(ExprResult::Val(v));
                }
            }
        }
        let expr_ctx = if let Some(s) = &self.ty {
            Cow::Owned(s.get_expr_context(ctx, ectx.curr_node())?)
        } else {
            ectx.expr_ctx.clone()
        };
        let attr = match expr_ctx.as_ref() {
            ExprContext::Local => {
                let am: &AttrMap = match ectx.local.as_ref() {
                    Some(l) => l.as_ref(),
                    None => ctx.env.attr_map(),
                };
                self.attr_nested(am)
            }
            ExprContext::Env => self.attr_nested(&ctx.env),
            ExprContext::Network => self.attr_nested(&ctx.network),
            ExprContext::Node(n) => self.attr_nested(
                &n.try_lock()
                    .into_option()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?,
            ),
            ExprContext::Nodes(nds) => {
                let mut vars = Vec::<ExprResult>::with_capacity(nds.len());
                for i in nds {
                    let a =
                        self.attr_nested(&i.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?);
                    vars.push(a.map_err(|e| e.pos(self.position()))?.into());
                }
                return Ok(ExprResult::Arr(vars));
            }
            ExprContext::NodesMap(nds) => {
                let res: Vec<(String, ExprResult)> = nds
                    .iter()
                    .map(|i| {
                        let n = i.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        let a = self.attr_nested(&n);
                        Ok((
                            n.name().to_string(),
                            ExprResult::from(a.map_err(|e| e.pos(self.position()))?),
                        ))
                    })
                    .collect::<Result<_, EvalError>>()?;
                return Ok(ExprResult::Map(res));
            }
        };

        match attr {
            Ok(Some(v)) => Ok(ExprResult::Val(v)),
            Ok(None) => Ok(ExprResult::None),
            Err(e) => Err(e.pos(self.position())),
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
    /// Roots of the network
    Roots,
    /// Roots of the network in map format
    RootsMap,
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
            TaskKeyword::Roots => Some(VarType::Roots),
            TaskKeyword::RootsMap => Some(VarType::RootsMap),
            TaskKeyword::Leaves => Some(VarType::Leaves),
            TaskKeyword::LeavesMap => Some(VarType::LeavesMap),
            _ => None,
        }
    }

    /// Convert to [`FunctionType`]
    pub fn to_functiontype(&self) -> &FunctionType {
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
            | VarType::Roots
            | VarType::RootsMap
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
            | VarType::Roots
            | VarType::Leaves => false,
            VarType::NodesMap(_)
            | VarType::OutputsMap
            | VarType::RootsMap
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
            (VarType::Roots | VarType::RootsMap, _) => {
                nodes_func(ctx.network.outlets().cloned().collect())
            }
            (VarType::Leaves | VarType::LeavesMap, _) => {
                nodes_func(ctx.network.leaves().cloned().collect())
            }
            (VarType::Root, _) => match ctx.network.root() {
                Some(r) => Ok(ExprContext::Node(r.clone())),
                None => Err(EvalErrorType::NoRootNode),
            },
            (VarType::Node(Some(n)), _) => match ctx.network.node_by_name(n) {
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
                nodes_func(n.lock().inputs().to_vec())
            }
            (VarType::Outputs | VarType::OutputsMap, Some(n)) => {
                nodes_func(n.lock().outputs().to_vec())
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
            VarType::Roots => "outlets",
            VarType::RootsMap => "outletsmap",
            VarType::Leaves => "leaves",
            VarType::LeavesMap => "leavesmap",
        };
        write!(f, "{ty}")
    }
}

/// A function call in the task system
#[derive(Clone)]
pub struct FunctionCall<T> {
    /// Type of the function (nodes, inputs, and output are node function)
    pub ty: Option<VarType>,
    /// Name of the function
    pub name: String,
    /// Positional Arguments
    pub args: Vec<T>,
    /// Keyword Arguments
    pub kwargs: HashMap<String, T>,
    /// start position of the function call
    pub start: (usize, usize),
}

impl<T> Position for FunctionCall<T> {
    fn position(&self) -> (usize, usize) {
        self.start
    }
}

impl<T: PartialEq> PartialEq for FunctionCall<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ty == other.ty
            && self.name == other.name
            && self.args == other.args
            && self.kwargs == other.kwargs
    }
}

impl<T: std::fmt::Debug> std::fmt::Debug for FunctionCall<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_struct("FunctionCall")
            .field("ty", &self.ty)
            .field("name", &self.name)
            .field("args", &self.args)
            .field("kwargs", &self.kwargs)
            .finish()
    }
}

impl<T: std::fmt::Display> std::fmt::Display for FunctionCall<T> {
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

/// Resolve the variables in the functioncall
///
/// Recursively resolves the expressions in the function arguments
pub fn resolve_function_calls<'b>(
    fc: FunctionCall<RawExpr>,
    ctx: &TaskContext,
    ectx: EvalCtx<'b>,
) -> Result<FunctionCall<ResolvedExpr<'b>>, EvalError> {
    let mut args = Vec::with_capacity(fc.args.len());
    let pos = fc.position();
    for a in fc.args {
        args.push(a.resolve(ctx, ectx.clone()).map_err(|e| e.pos(pos))?);
    }
    let mut kwargs = HashMap::with_capacity(fc.kwargs.len());
    for (k, a) in fc.kwargs {
        kwargs.insert(
            k.clone(),
            a.resolve(ctx, ectx.clone()).map_err(|e| e.pos(pos))?,
        );
    }
    Ok(FunctionCall {
        ty: fc.ty.clone(),
        name: fc.name.clone(),
        args,
        kwargs,
        start: fc.start,
    })
}

impl<T> FunctionCall<T> {
    /// New functioncall
    pub fn new(
        ty: Option<VarType>,
        name: String,
        args: Vec<T>,
        kwargs: HashMap<String, T>,
        start: (usize, usize),
    ) -> Self {
        Self {
            ty,
            name,
            args,
            kwargs,
            start,
        }
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
}

impl<'a> FunctionCall<ResolvedExpr<'a>> {
    pub fn function_ctx(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
    ) -> Result<FunctionCtx, EvalError> {
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            args.push(
                a.eval_value(ctx, ectx)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            kwargs.insert(
                k.clone(),
                a.eval_value(ctx, ectx)
                    .map_err(|e| e.pos(self.position()))?,
            );
        }
        Ok(FunctionCtx::from_arg_kwarg(args, kwargs))
    }
}

impl Eval for FunctionCall<ResolvedExpr<'_>> {
    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let new_ectx = self.get_expr_context(
            ectx.expr_ctx.as_ref().function_type(),
            ctx,
            ectx.curr_node(),
        )?;
        let ectx = ectx.with_expr_ctx(Cow::Borrowed(&new_ectx));
        match &new_ectx {
            ExprContext::Local | ExprContext::Env => {
                let func_ctx = self.function_ctx(ctx, &ectx)?;
                match ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    Some(func) => Ok(func.eval(ctx, &ectx, func_ctx)?),
                    _ => match ctx.functions.env(&self.name) {
                        Some(f) => f.call(&func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                        }),
                        None => Err(EvalErrorType::FunctionNotFound(
                            Some(ectx.expr_ctx.as_ref().function_type().clone()),
                            self.name.to_string(),
                        )
                        .pos(self.position())),
                    },
                }
            }
            ExprContext::Network => {
                let func_ctx = self.function_ctx(ctx, &ectx)?;
                match ctx.functions.network(&self.name) {
                    Some(f) => f.call(&ctx.network, &func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval(ctx, &ectx.as_env()),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Node(node) => {
                let func_ctx = self.function_ctx(ctx, &ectx.at_node(node.clone()))?;
                match ctx.functions.node(&self.name) {
                    Some(f) => {
                        let n = node.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        f.call(&n, &func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                        })
                    }
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval(ctx, &ectx.as_env()),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Nodes(_nds) | ExprContext::NodesMap(_nds) => {
                Err(EvalErrorType::NotImplementedError("nodes not implemented")
                    .pos(self.position()))
            }
        }
    }

    fn eval_mut(&self, ctx: &mut TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let new_ectx = self.get_expr_context(
            ectx.expr_ctx.as_ref().function_type(),
            ctx,
            ectx.curr_node(),
        )?;
        let ectx = ectx.with_expr_ctx(Cow::Borrowed(&new_ectx));
        let res = match ectx.expr_ctx.as_ref() {
            ExprContext::Local | ExprContext::Env => {
                let func_ctx = self.function_ctx(ctx, &ectx)?;
                match ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    Some(func) => Ok(func.eval(ctx, &ectx, func_ctx)?),
                    _ => match ctx.functions.env(&self.name) {
                        Some(f) => f.call(&func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                        }),
                        None => Err(EvalErrorType::FunctionNotFound(
                            Some(ectx.expr_ctx.as_ref().function_type().clone()),
                            self.name.to_string(),
                        )
                        .pos(self.position())),
                    },
                }
            }
            ExprContext::Network => {
                let func_ctx = self.function_ctx(ctx, &ectx)?;
                match ctx.functions.network(&self.name) {
                    Some(f) => f.call_mut(&mut ctx.network, &func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval_mut(ctx, &ectx.as_env()),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Node(node) => {
                let func_ctx = self.function_ctx(ctx, &ectx.at_node(node.clone()))?;
                match ctx.functions.node(&self.name) {
                    Some(f) => {
                        let mut n = node.try_lock().into_option().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        f.call_mut(&mut n, &func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                        })
                    }
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval_mut(ctx, &ectx.as_env()),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Nodes(_nds) | ExprContext::NodesMap(_nds) => {
                Err(EvalErrorType::NotImplementedError("nodes not implemented")
                    .pos(self.position()))
            }
        };
        _ = ctx.channel.send(TaskMessage::Changed);
        res
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
    AttrExpr(RawExpr),
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
        _local: Option<&AttrMap>,
        node: Option<&Node>,
    ) -> Result<Series, EvalError> {
        match self {
            Self::AttrExpr(_) => todo!(),
            // Self::AttrExpr(expr) => match expr.resolve_eval(ft, ctx, local, node)? {
            //     ExprResult::Arr(ar) => Ok(MaskedSeries::from(
            //         ar.into_iter()
            //             .map(ExprResult::to_attribute)
            //             .collect::<Vec<Option<Attribute>>>(),
            //     )
            //     .retype()
            //     .into()),
            //     _ => Err(EvalErrorType::NotAnArray.no_pos()),
            // },
            Self::Series(ty, ts, sr) => get_series(ft, ctx, node, ty, sr, *ts),
            Self::SeriesMap(ty, ts, sr, func) => {
                let sr = get_series(ft, ctx, node, ty, sr, *ts)?;
                match func {
                    MapFunction::Defn(udf) => {
                        let func_call = |arg| {
                            let fctx = if let Some(a) = arg {
                                FunctionCtx::from_arg_kwarg(vec![a], HashMap::new())
                            } else {
                                FunctionCtx::from_arg_kwarg(vec![], HashMap::new())
                            };
                            // eval udf with node/network context
                            udf.eval(ctx, &EvalCtx::default(), fctx)
                        };
                        sr.map_values(&func_call)
                    }
                    MapFunction::Pointer(name) => {
                        let func_call = |arg| {
                            let fctx = if let Some(a) = arg {
                                FunctionCtx::from_arg_kwarg(vec![a], HashMap::new())
                            } else {
                                FunctionCtx::from_arg_kwarg(vec![], HashMap::new())
                            };
                            match ctx.udf(name).cloned() {
                                // priority for the locally defined function; evaluated in local context
                                Some(func) => func.eval(ctx, &EvalCtx::default(), fctx),
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
            let n: &NodeInner = n;
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
                        let cs = ms
                            .complete()
                            .ok_or(EvalErrorType::EmptyValue(None).no_pos())?;
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
pub struct ExprWithContext<T: std::fmt::Debug + Clone + PartialEq> {
    pub ty: VarType,
    // TODO: make it a vec of expression later
    pub expr: Box<T>,
}

impl<T: std::fmt::Display + std::fmt::Debug + Clone + PartialEq> std::fmt::Display
    for ExprWithContext<T>
{
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

// FIX: when there is setvariable inside the context expression, then we can not resolve in advance, or resolve into array and tables
// e.g.: nodes { node.x = 8 }

fn resolve_expr_w_ctx<'b>(
    expr: ExprWithContext<RawExpr>,
    ctx: &TaskContext,
    ectx: EvalCtx<'b>,
) -> Result<ResolvedExpr<'b>, EvalError> {
    let expr_ctx = expr
        .ty
        .get_expr_context(ctx, ectx.curr_node())
        .map_err(|e| e.no_pos())?;
    let mut context = ectx.clone();
    context.expr_ctx = Cow::Owned(expr_ctx.clone());
    match expr_ctx {
        ExprContext::Local => todo!(),
        ExprContext::Node(n) => expr.expr.resolve(ctx, ectx.at_node(n).to_owned()),
        ExprContext::Nodes(nds) => {
            let exprs = nds
                .into_iter()
                .map(|n| expr.expr.clone().resolve(ctx, ectx.at_node(n).to_owned()))
                .collect::<Result<Vec<ResolvedExpr>, EvalError>>()?;
            // FIX: how to know whether the user is looking for array or statement?
            let et = ExprType::Array(exprs);
            Ok(ResolvedExpr {
                position: expr.expr.position(),
                expr: et,
                context,
            })
        }
        ExprContext::NodesMap(nds) => {
            let exprs = nds
                .into_iter()
                .map(|n| {
                    let name = n
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                        .name()
                        .to_string();
                    let expr = expr.expr.clone().resolve(ctx, ectx.at_node(n).to_owned())?;
                    Ok((name, expr))
                })
                .collect::<Result<Vec<(String, ResolvedExpr)>, EvalError>>()?;
            let et = ExprType::Map(exprs);
            Ok(ResolvedExpr {
                position: expr.expr.position(),
                expr: et,
                context,
            })
        }
        ExprContext::Env => expr.expr.resolve(ctx, ectx.as_env().to_owned()),
        ExprContext::Network => expr.expr.resolve(ctx, ectx.as_network().to_owned()),
    }
}

impl<T: std::fmt::Display + std::fmt::Debug + Clone + PartialEq + Position> ExprWithContext<T> {
    pub fn new(ty: VarType, expr: T) -> Self {
        Self {
            ty,
            expr: Box::new(expr),
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

impl ExprContext {
    pub fn function_type(&self) -> &FunctionType {
        match self {
            Self::Local | Self::Env => &FunctionType::Env,
            Self::Network => &FunctionType::Network,
            Self::Node(_) | Self::Nodes(_) | Self::NodesMap(_) => &FunctionType::Node,
        }
    }

    pub fn curr_node(&self) -> Option<&Node> {
        if let ExprContext::Node(n) = self {
            Some(n)
        } else {
            None
        }
    }
}

impl PartialEq for ExprContext {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local, Self::Local) => true,
            (Self::Env, Self::Env) => true,
            (Self::Network, Self::Network) => true,
            (Self::Node(n1), Self::Node(n2)) => {
                let n1 = n1.lock().name().to_string();
                let n2 = n2.lock().name().to_string();
                n1 == n2
            }
            // TODO
            (Self::Nodes(_nds1), Self::Nodes(_nds2)) => false,
            (Self::NodesMap(_nds1), Self::NodesMap(_nds2)) => false,
            _ => false,
        }
    }
}

impl std::fmt::Debug for ExprContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Local => write!(f, "local"),
            Self::Env => write!(f, "env"),
            Self::Network => write!(f, "network"),
            Self::Node(_) => write!(f, "node"),
            // Self::Node(n) => write!(f, "node {}", n.lock().name()),
            Self::Nodes(_) => write!(f, "nodes"),
            Self::NodesMap(_) => write!(f, "nodesmap"),
        }
    }
}

#[derive(Clone)]
pub struct SetVariable<T> {
    var: InputVar,
    expr: Box<T>,
    silent: bool,
    ctx: Option<ExprContext>,
}

impl<T: std::fmt::Debug> std::fmt::Debug for SetVariable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SetVariable")
            .field("var", &self.var)
            .field("expr", &self.expr)
            .field("silent", &self.silent)
            .field("has_context", &self.ctx.is_some())
            .finish()
    }
}

impl<T: PartialEq> PartialEq for SetVariable<T> {
    fn eq(&self, other: &Self) -> bool {
        self.var == other.var && self.expr == other.expr && self.silent == other.silent
    }
}

impl<T: std::fmt::Display> std::fmt::Display for SetVariable<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} = {}", self.var, self.expr)
    }
}

fn resolve_set_variable<'b>(
    expr: SetVariable<RawExpr>,
    ctx: &TaskContext,
    ectx: EvalCtx<'b>,
) -> Result<ResolvedExpr<'b>, EvalError> {
    let e = expr.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref());
    let new_expr_ctx = expr
        .var
        .get_expr_context(e.function_type(), ctx, e.curr_node())?;
    let new_ectx = ectx.with_expr_ctx(Cow::Owned(new_expr_ctx.clone()));
    let etc = match new_expr_ctx {
        ExprContext::Local => todo!(),
        ExprContext::Env => new_ectx.as_env().to_owned(),
        ExprContext::Network => new_ectx.as_network().to_owned(),
        ExprContext::Node(n) => new_ectx.at_node(n).to_owned(),
        ExprContext::Nodes(nds) | ExprContext::NodesMap(nds) => {
            let exprs = nds
                .into_iter()
                .map(|n| {
                    let context = new_ectx.at_node(n.clone()).to_owned();
                    let mut vt = expr.var.clone();
                    _ = vt.ty.replace(VarType::Node(None));
                    Ok(ResolvedExpr {
                        expr: ExprType::SetVar(SetVariable::new(
                            vt,
                            expr.expr.clone().resolve(ctx, context.clone())?,
                            expr.silent,
                        )),
                        position: expr.expr.position(),
                        context,
                    })
                })
                .collect::<Result<Vec<ResolvedExpr>, EvalError>>()?;
            // FIX: how to know whether the user is looking for array or statement?
            let et = ExprType::Multi(exprs);
            return Ok(ResolvedExpr {
                position: expr.expr.position(),
                expr: et,
                context: new_ectx,
            });
        }
    };

    Ok(ResolvedExpr {
        expr: ExprType::SetVar(SetVariable::new(
            expr.var,
            expr.expr.clone().resolve(ctx, etc.clone())?,
            expr.silent,
        )),
        position: expr.expr.position(),
        context: etc,
    })
}

impl<T: Clone> SetVariable<T> {
    pub fn new(var: InputVar, expr: T, silent: bool) -> Self {
        Self {
            var,
            expr: Box::new(expr),
            silent,
            ctx: None,
        }
    }

    pub fn with_ctx(&self, ctx: ExprContext) -> Result<Self, EvalError> {
        // eprintln!("Resolving: {self:?}");
        let mut e = self.clone();
        e.ctx = Some(ctx);
        // eprintln!("Found: {:?}", e.ctx);
        Ok(e)
    }
}

impl<T: Clone + Eval> Eval for SetVariable<T> {
    fn eval(&self, ctx: &TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        // TODO: look into why this is setting in env context, we need to make it take the context from variable type we are setting.
        let e = self.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref());
        let new_expr_ctx = self
            .var
            .get_expr_context(e.function_type(), ctx, e.curr_node())?;
        let new_ectx = ectx.with_expr_ctx(Cow::Borrowed(&new_expr_ctx));

        match &new_expr_ctx {
            ExprContext::Local => Err(EvalErrorType::InvalidVariableType.no_pos()),
            ExprContext::Env | ExprContext::Network => {
                // env and network can only be modified if the context is in mutable state
                Err(EvalErrorType::InvalidVariableType.no_pos())
            }
            ExprContext::Node(n) => {
                let val = self.expr.eval_value(ctx, &new_ectx.at_node(n.clone()))?;
                self.var.set_attr_nested(n.lock().attr_map_mut(), val)?;
                _ = ctx.channel.send(TaskMessage::Changed);
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val = self.expr.eval_value(ctx, &new_ectx.at_node(n.clone()))?;
                    self.var.set_attr_nested(n.lock().attr_map_mut(), val)?;
                }
                _ = ctx.channel.send(TaskMessage::Changed);

                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }

    fn eval_mut(&self, ctx: &mut TaskContext, ectx: &EvalCtx) -> Result<ExprResult, EvalError> {
        let e = self.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref());
        let new_expr_ctx = self
            .var
            .get_expr_context(e.function_type(), ctx, e.curr_node())?;
        let new_ectx = ectx.with_expr_ctx(Cow::Borrowed(&new_expr_ctx));

        match self.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref()) {
            ExprContext::Local => Err(EvalErrorType::InvalidVariableType.no_pos()),
            ExprContext::Env => {
                let val = self.expr.eval_value(ctx, &new_ectx)?;
                self.var.set_attr_nested(ctx.env.attr_map_mut(), val)?;
                _ = ctx.channel.send(TaskMessage::Changed);
                Ok(ExprResult::None)
            }
            ExprContext::Network => {
                let val = self.expr.eval_value(ctx, &new_ectx)?;
                self.var.set_attr_nested(ctx.network.attr_map_mut(), val)?;
                _ = ctx.channel.send(TaskMessage::Changed);
                Ok(ExprResult::None)
            }
            ExprContext::Node(n) => {
                let val = self.expr.eval_value(ctx, &new_ectx.at_node(n.clone()))?;
                self.var.set_attr_nested(n.lock().attr_map_mut(), val)?;
                _ = ctx.channel.send(TaskMessage::Changed);
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val = self.expr.eval_value(ctx, &new_ectx.at_node(n.clone()))?;
                    self.var.set_attr_nested(n.lock().attr_map_mut(), val)?;
                }
                _ = ctx.channel.send(TaskMessage::Changed);

                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }
}
