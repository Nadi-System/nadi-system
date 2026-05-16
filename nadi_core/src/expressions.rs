use crate::attrs::{AttrMap, Attribute, FromAttribute, HasAttributes};
use crate::eval::{Eval, EvalCtx, EvalError, EvalErrorType};
use crate::functions::{FunctionCtx, FunctionInput};
use crate::network::Propagation;
use crate::node::{Node, NodeInner};
use crate::structs::NadiAttrType;
use crate::tasks::{FunctionType, TaskContext, TaskCtxConsts, TaskKeyword, TaskMessage};
use crate::template::Template;
use crate::timeseries::{
    CompleteSeries, HasSeries, HasTimeSeries, MaskedSeries, Series, TimeLine, TimeSeries,
};
use crate::udf::UserFunction;
use abi_stable::std_types::{RNone, ROption, RSome, RString, Tuple2};
use std::borrow::Cow;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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

    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let res = self
            .clone()
            .resolve(ctx, ectx.clone())
            .map_err(|e| e.pos(self.position()))?;
        res.eval(ctx, ectx, loc).map_err(|e| e.pos(self.position()))
    }

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let res = self
            .clone()
            .resolve(ctx, ectx.clone())
            .map_err(|e| e.pos(self.position()))?;
        res.eval_mut(ctx, ectx, loc)
            .map_err(|e| e.pos(self.position()))
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
            ExprType::Args(vt) => ExprType::Args(vt),
            ExprType::KwArgs(vt) => ExprType::KwArgs(vt),
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
            ExprType::ForEach(var, parent, expr) => ExprType::ForEach(
                var,
                Box::new(parent.resolve(ctx, ectx.clone())?),
                Box::new(expr.resolve(ctx, ectx.clone())?),
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
            ExprType::Multi(exprs, b) => ExprType::Multi(
                exprs
                    .into_iter()
                    .map(|e| e.resolve(ctx, ectx.clone()))
                    .collect::<Result<Vec<_>, _>>()?,
                b,
            ),
            ExprType::Array(exprs) => ExprType::Array(
                exprs
                    .into_iter()
                    .map(|e| e.resolve(ctx, ectx.clone()))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ExprType::ArrayGen(expr, var, parent) => ExprType::ArrayGen(
                Box::new(expr.resolve(ctx, ectx.clone())?),
                var,
                Box::new(parent.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Map(exprs) => ExprType::Map(
                exprs
                    .into_iter()
                    .map(|(k, e)| e.resolve(ctx, ectx.clone()).map(|v| (k, v)))
                    .collect::<Result<Vec<_>, _>>()?,
            ),
            ExprType::MapGen(key, val, var1, var2, parent) => ExprType::MapGen(
                Box::new(key.resolve(ctx, ectx.clone())?),
                Box::new(val.resolve(ctx, ectx.clone())?),
                var1,
                var2,
                Box::new(parent.resolve(ctx, ectx.clone())?),
            ),
            ExprType::Silent(e) => ExprType::Silent(Box::new(e.resolve(ctx, ectx.clone())?)),
            ExprType::Check(e) => match e.resolve(ctx, ectx.clone()) {
                Ok(r) => ExprType::Check(Box::new(r)),
                Err(_) => ExprType::Result(ExprResult::Val(false.into())),
            },
            ExprType::WithContext(e) => ExprType::WithContext(e),
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
            #[cfg(feature = "parser")]
            ExprType::Import(i) => ExprType::Import(i),
            ExprType::Series(s) => ExprType::Series(s),
            ExprType::MapSeries(s) => ExprType::MapSeries(s),
            ExprType::SetSeries(ss) => return resolve_set_series(ss, ctx, ectx.clone()),
            // _ => {
            //     return Err(EvalErrorType::NotImplementedError(
            //         "resolving logic not implemented yet for these",
            //     )
            //     .no_pos());
            // }
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
    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        _ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        self.expr
            .eval_mut(ctx, &self.context, loc)
            .map_err(|e| self.err_ctx(e))
    }

    fn eval(
        &self,
        ctx: &TaskContext,
        _ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        self.expr
            .eval(ctx, &self.context, loc)
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
    /// Series value
    Series(Series),
    /// Timeseries value
    TimeSeries(TimeSeries),
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

impl<'a> ExprResult {
    fn to_function_input(self) -> Result<FunctionInput<'a>, EvalError> {
        match self {
            Self::None => Ok(FunctionInput::None),
            Self::Series(s) => Ok(FunctionInput::SeriesOwn(s)),
            Self::TimeSeries(s) => Ok(FunctionInput::TsOwn(s)),
            Self::NoneErr(e) => Err(*e.clone()),
            a => a
                .to_attribute()
                .map(FunctionInput::AttrOwn)
                .ok_or(EvalErrorType::EmptyValue(None).no_pos()),
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
            Self::Series(s) => write!(f, "{s}"),
            Self::TimeSeries(s) => write!(f, "{s}"),
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
            Self::Series(s) => s.to_attributes().map(|v| Attribute::Array(v.into())),
            Self::TimeSeries(ts) => ts
                .series()
                .clone()
                .to_attributes()
                .map(|v| Attribute::Array(v.into())),
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

    pub fn to_attributes(self) -> Option<Vec<Attribute>> {
        match self {
            Self::Val(a) => Vec::<Attribute>::from_attr(&a),
            Self::Series(s) => s.to_attributes(),
            // disregards timeline
            Self::TimeSeries(ts) => ts.series().clone().to_attributes(),
            Self::Arr(ar) => ar
                .into_iter()
                .map(|a| a.to_attribute())
                .collect::<Option<Vec<Attribute>>>(),
            _ => None,
        }
    }

    pub fn to_opt_attributes(self) -> Option<Vec<ROption<Attribute>>> {
        Some(match self {
            Self::Val(a) => Vec::<Attribute>::from_attr(&a)?
                .into_iter()
                .map(RSome)
                .collect(),
            Self::Series(s) => s.to_opt_attributes(),
            // disregards timeline
            Self::TimeSeries(ts) => ts.series().clone().to_opt_attributes(),
            Self::Arr(ar) => ar
                .into_iter()
                .map(|a| a.to_attribute().into())
                .collect::<Vec<ROption<Attribute>>>(),
            _ => return None,
        })
    }

    pub fn to_series(self) -> Option<Series> {
        match self {
            Self::Val(a) => Some(
                CompleteSeries::attributes(Vec::<Attribute>::from_attr(&a)?)
                    .retype()
                    .into(),
            ),
            Self::Series(s) => Some(s),
            // disregards timeline
            Self::TimeSeries(ts) => Some(ts.series().clone()),
            Self::Arr(ar) => Some(
                MaskedSeries::attributes(
                    ar.into_iter()
                        .map(|a| a.to_attribute().into())
                        .collect::<Vec<ROption<Attribute>>>(),
                )
                .retype()
                .into(),
            ),
            _ => None,
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
    /// args expression
    Args(InputVar),
    /// kwargs expression
    KwArgs(InputVar),
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
    WithContext(ExprWithContext),
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
    Multi(Vec<T>, bool),
    /// Series
    Series(GetSeries),
    /// Set Series to a value
    SetSeries(SetSeries<T>),
    /// Series mapped by a function
    MapSeries(MapSeries),
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
            Self::Args(v) => write!(f, "*{v}"),
            Self::KwArgs(v) => write!(f, "**{v}"),
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
            Self::Multi(exprs, b) => {
                write!(
                    f,
                    "{}{}",
                    exprs
                        .iter()
                        .map(|e| e.to_string())
                        .collect::<Vec<_>>()
                        .join("\n"),
                    if *b { ";" } else { "" }
                )
            }
            Self::Series(s) => std::fmt::Display::fmt(s, f),
            Self::SetSeries(s) => std::fmt::Display::fmt(s, f),
            Self::MapSeries(s) => std::fmt::Display::fmt(s, f),
        }
    }
}

impl Eval for ExprType<ResolvedExpr<'_>> {
    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        match self {
            Self::Function(fc) => Ok(fc.eval_mut(ctx, ectx, loc)?),
            Self::SetVar(sv) => {
                sv.eval_mut(ctx, ectx, loc)?;
                Ok(ExprResult::None)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ctx, ectx, loc)?;
                let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                if cond {
                    expr1.eval_mut(ctx, ectx, loc)
                } else if let Some(expr2) = expr2 {
                    expr2.eval_mut(ctx, ectx, loc)
                } else {
                    Ok(ExprResult::None)
                }
            }
            Self::ForEach(var, parent, expr) => {
                let mut local = loc.clone();
                // should we extend to existing local? prob not
                let parent =
                    Vec::<Attribute>::from_attr(&parent.eval_value(ctx, ectx, &mut local)?)
                        .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                for val in parent {
                    local.insert(var.to_string().into(), val);
                    match expr.eval_mut(ctx, ectx, &mut local) {
                        Ok(_) => (),
                        Err(e) => match *e.ty {
                            EvalErrorType::InvalidBreak(b) => return Ok(b),
                            EvalErrorType::InvalidContinue => continue,
                            _ => return Err(e),
                        },
                    }
                }
                Ok(ExprResult::None)
            }
            Self::While(cond, expr) => {
                let max_it = TaskCtxConsts::max_iterations(ctx);
                let mut it = 0;
                loop {
                    let cond = cond.eval_value(ctx, ectx, loc)?;
                    let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                    if cond {
                        match expr.eval_mut(ctx, ectx, loc) {
                            Ok(_) => (),
                            Err(e) => match *e.ty {
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
                    match expr.eval_mut(ctx, ectx, loc) {
                        Ok(_) => (),
                        Err(e) => match *e.ty {
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
            Self::TryCatch(expr1, expr2) => match expr1.eval_mut(ctx, ectx, loc) {
                Ok(ExprResult::NoneErr(_)) | Err(_) => expr2.eval_mut(ctx, ectx, loc),
                Ok(val) => Ok(val),
            },
            Self::Multi(exprs, silent) => {
                let mut res = ExprResult::None;
                for e in exprs {
                    res = e.eval_mut(ctx, ectx, loc)?;
                }
                if *silent {
                    Ok(ExprResult::None)
                } else {
                    Ok(res)
                }
            }
            Self::Array(exprs) => {
                let vals: Vec<ExprResult> =
                    if exprs.iter().any(|e| matches!(&e.expr, ExprType::Args(_))) {
                        exprs
                            .iter()
                            .map(|v| match &v.expr {
                                ExprType::Args(vt) => {
                                    // eval, take an array, and insert it as expression
                                    let parent = Vec::<Attribute>::from_attr(
                                        &vt.eval_value(ctx, &v.context, loc)?,
                                    )
                                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                                    Ok(parent
                                        .into_iter()
                                        .map(ExprResult::Val)
                                        .collect::<Vec<ExprResult>>())
                                }
                                e => Ok(vec![e.eval_mut(ctx, &v.context, loc)?]),
                            })
                            .collect::<Result<Vec<_>, EvalError>>()?
                            .into_iter()
                            .flatten()
                            .collect()
                    } else {
                        exprs
                            .iter()
                            .map(|v| v.eval_mut(ctx, ectx, loc))
                            .collect::<Result<_, _>>()?
                    };
                Ok(ExprResult::Arr(vals))
            }
            Self::ArrayGen(expr, var, parent) => {
                let parent = Vec::<Attribute>::from_attr(&parent.eval_value(ctx, ectx, loc)?)
                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                let mut locals = loc.clone();
                let vals = parent
                    .into_iter()
                    .map(|v| {
                        locals.insert(var.to_string().into(), v);
                        expr.eval_mut_value(ctx, ectx, &mut locals)
                    })
                    .collect::<Result<Vec<Attribute>, _>>()?;
                Ok(ExprResult::Val(vals.into()))
            }
            Self::Map(exprs) => {
                let vals: Vec<(String, ExprResult)> = exprs
                    .iter()
                    .map(|(k, v)| v.eval_mut(ctx, ectx, loc).map(|v| (k.to_string(), v)))
                    .collect::<Result<_, _>>()?;
                Ok(ExprResult::Map(vals))
            }
            Self::MapGen(key, val, var1, var2, parent) => {
                let parent = AttrMap::from_attr(&parent.eval_value(ctx, ectx, loc)?)
                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                let mut locals = loc.clone();
                let vals = parent
                    .into_iter()
                    .map(|Tuple2(k, v)| -> Result<(RString, Attribute), EvalError> {
                        locals.insert(var1.to_string().into(), k.into());
                        locals.insert(var2.to_string().into(), v);
                        let k = key.eval_value(ctx, ectx, &mut locals)?;
                        let key = RString::from_attr(&k).ok_or(
                            EvalErrorType::InvalidAttributeType(NadiAttrType::String, k.dtype())
                                .no_pos(),
                        )?;
                        let val = val.eval_mut_value(ctx, ectx, &mut locals)?;
                        Ok((key, val))
                    })
                    .collect::<Result<HashMap<RString, Attribute>, _>>()?;
                Ok(ExprResult::Val(vals.into()))
            }
            Self::Silent(e) => {
                e.eval_mut(ctx, ectx, loc)?;
                Ok(ExprResult::None)
            }
            Self::Check(e) => Ok(ExprResult::Val(
                match e.eval_mut(ctx, ectx, loc) {
                    Ok(ExprResult::None) | Ok(ExprResult::NoneErr(_)) | Err(_) => false,
                    Ok(_) => true,
                }
                .into(),
            )),
            Self::WithContext(e) => e.eval_mut(ctx, ectx, loc),
            #[cfg(feature = "parser")]
            Self::Import(i) => i.eval_mut(ctx, ectx, loc),
            Self::UniOp(op, expr) => op
                .eval(expr.eval_mut_value(ctx, ectx, loc)?)
                .map(ExprResult::Val),
            Self::BiOp(op, expr1, expr2) => op
                .eval(
                    expr1.eval_mut_value(ctx, ectx, loc)?,
                    expr2.eval_mut_value(ctx, ectx, loc)?,
                )
                .map(ExprResult::Val),
            Self::SetSeries(s) => s.eval_mut(ctx, ectx, loc),
            Self::None
            | Self::Value(_)
            | Self::Result(_)
            | Self::Progress(_)
            | Self::Var(_)
            | Self::Args(_)
            | Self::KwArgs(_)
            | Self::Render(_)
            | Self::UserError(_)
            | Self::Return(_)
            | Self::Break(_)
            | Self::Continue
            | Self::Range(..)
            | Self::Series(_)
            | Self::MapSeries(_) => self.eval(ctx, ectx, loc),
        }
    }

    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        match self {
            Self::None => Ok(ExprResult::None),
            Self::Value(r) => Ok(ExprResult::Val(r.clone())),
            Self::Result(r) => Ok(r.clone()),
            Self::Progress(p) => {
                p.eval(ctx, ectx, loc)?;
                Ok(ExprResult::None)
            }
            Self::Function(fc) => Ok(fc.eval(ctx, ectx, loc)?),
            Self::Var(vt) => Ok(vt.eval(ctx, ectx, loc)?),
            // these two shouldn't be evaluated directly
            Self::Args(_) => {
                Err(EvalErrorType::LogicalError("shouldn't be evaluated directly").no_pos())
            }
            Self::KwArgs(_) => {
                Err(EvalErrorType::LogicalError("shouldn't be evaluated directly").no_pos())
            }
            Self::SetVar(sv) => {
                sv.eval(ctx, ectx, loc)?;
                Ok(ExprResult::None)
            }
            Self::IfElse(cond, expr1, expr2) => {
                let cond = cond.eval_value(ctx, ectx, loc)?;
                let cond = bool::from_attr(&cond).ok_or(EvalErrorType::NotABool.no_pos())?;
                if cond {
                    expr1.eval(ctx, ectx, loc)
                } else if let Some(expr2) = expr2 {
                    expr2.eval(ctx, ectx, loc)
                } else {
                    Ok(ExprResult::None)
                }
            }
            Self::While(_, _) | Self::Loop(_) => Err(EvalErrorType::LogicalError(
                "while and loop in immutable context leads into an infinite loop",
            )
            .no_pos()),
            Self::TryCatch(expr1, expr2) => match expr1.eval(ctx, ectx, loc) {
                // returning none (e.g. silent expression) is fine
                Ok(ExprResult::NoneErr(_)) | Err(_) => expr2.eval(ctx, ectx, loc),
                Ok(val) => Ok(val),
            },
            Self::Multi(exprs, silent) => {
                let mut res = ExprResult::None;
                for e in exprs {
                    res = e.eval(ctx, ectx, loc)?;
                }
                if *silent {
                    Ok(ExprResult::None)
                } else {
                    Ok(res)
                }
            }
            Self::Array(exprs) => {
                let vals: Vec<ExprResult> =
                    if exprs.iter().any(|e| matches!(&e.expr, ExprType::Args(_))) {
                        exprs
                            .iter()
                            .map(|v| match &v.expr {
                                ExprType::Args(vt) => {
                                    // eval, take an array, and insert it as expression
                                    let parent = Vec::<Attribute>::from_attr(
                                        &vt.eval_value(ctx, &v.context, loc)?,
                                    )
                                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                                    Ok(parent
                                        .into_iter()
                                        .map(ExprResult::Val)
                                        .collect::<Vec<ExprResult>>())
                                }
                                e => Ok(vec![e.eval(ctx, &v.context, loc)?]),
                            })
                            .collect::<Result<Vec<_>, EvalError>>()?
                            .into_iter()
                            .flatten()
                            .collect()
                    } else {
                        exprs
                            .iter()
                            .map(|v| v.eval(ctx, ectx, loc))
                            .collect::<Result<_, _>>()?
                    };
                Ok(ExprResult::Arr(vals))
            }
            Self::ArrayGen(expr, var, parent) => {
                let parent = Vec::<Attribute>::from_attr(&parent.eval_value(ctx, ectx, loc)?)
                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                let vals = parent
                    .into_iter()
                    .map(|v| {
                        let mut locals = loc.clone();
                        locals.insert(var.to_string().into(), v);
                        expr.eval_value(ctx, ectx, &mut locals)
                    })
                    .collect::<Result<Vec<Attribute>, _>>()?;
                Ok(ExprResult::Val(vals.into()))
            }
            Self::Map(exprs) => {
                let vals: Vec<(String, ExprResult)> = if exprs
                    .iter()
                    .any(|e| matches!(&e.1.expr, ExprType::KwArgs(_)))
                {
                    exprs
                        .iter()
                        .map(|(k, v)| match &v.expr {
                            ExprType::KwArgs(vt) => {
                                // eval, take an array, and insert it as expression
                                let res = vt.eval_value(ctx, &v.context, loc)?;
                                let parent = AttrMap::from_attr(&res).ok_or(
                                    EvalErrorType::InvalidAttributeType(
                                        NadiAttrType::Table,
                                        res.dtype(),
                                    )
                                    .no_pos(),
                                )?;
                                Ok(parent
                                    .into_iter()
                                    .map(|Tuple2(k, v)| (k.to_string(), ExprResult::Val(v)))
                                    .collect::<Vec<(String, ExprResult)>>())
                            }
                            e => Ok(vec![(k.to_string(), e.eval(ctx, &v.context, loc)?)]),
                        })
                        .collect::<Result<Vec<_>, EvalError>>()?
                        .into_iter()
                        .flatten()
                        .collect()
                } else {
                    exprs
                        .iter()
                        .map(|(k, v)| v.eval(ctx, ectx, loc).map(|v| (k.to_string(), v)))
                        .collect::<Result<_, _>>()?
                };
                Ok(ExprResult::Map(vals))
            }
            Self::MapGen(key, val, var1, var2, parent) => {
                let parent = AttrMap::from_attr(&parent.eval_value(ctx, ectx, loc)?)
                    .ok_or(EvalErrorType::NotAnArray.no_pos())?;
                let vals = parent
                    .into_iter()
                    .map(|Tuple2(k, v)| -> Result<(RString, Attribute), EvalError> {
                        let mut locals = loc.clone();
                        locals.insert(var1.to_string().into(), k.into());
                        locals.insert(var2.to_string().into(), v);
                        let k = key.eval_value(ctx, ectx, &mut locals)?;
                        let key = RString::from_attr(&k).ok_or(
                            EvalErrorType::InvalidAttributeType(NadiAttrType::String, k.dtype())
                                .no_pos(),
                        )?;
                        let val = val.eval_value(ctx, ectx, &mut locals)?;
                        Ok((key, val))
                    })
                    .collect::<Result<HashMap<RString, Attribute>, _>>()?;
                Ok(ExprResult::Val(vals.into()))
            }
            Self::Silent(e) => {
                e.eval(ctx, ectx, loc)?;
                Ok(ExprResult::None)
            }
            Self::Check(e) => Ok(ExprResult::Val(
                match e.eval(ctx, ectx, loc) {
                    Ok(ExprResult::None) | Ok(ExprResult::NoneErr(_)) | Err(_) => false,
                    Ok(_) => true,
                }
                .into(),
            )),
            // This is already resolved context, so it is fine to just evaluate
            Self::WithContext(e) => e.eval(ctx, ectx, loc),
            Self::Range(b, s, e) => {
                let b = b.eval_value(ctx, ectx, loc)?;
                let b = i64::from_attr(&b).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    b.dtype(),
                ))?;
                let e = e.eval_value(ctx, ectx, loc)?;
                let e = i64::from_attr(&e).ok_or(EvalErrorType::InvalidAttributeType(
                    NadiAttrType::Integer,
                    e.dtype(),
                ))?;
                let s = if let Some(s) = s {
                    let s = s.eval_value(ctx, ectx, loc)?;
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
            Self::Render(t) => t.eval(ctx, ectx, loc),
            Self::UserError(s) => Err(EvalErrorType::UserError(s.clone()).no_pos()),
            Self::UniOp(op, expr) => op
                .eval(expr.eval_value(ctx, ectx, loc)?)
                .map(ExprResult::Val),
            Self::BiOp(op, expr1, expr2) => {
                let first = expr1.eval_value(ctx, ectx, loc)?;
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
                op.eval(first, expr2.eval_value(ctx, ectx, loc)?)
                    .map(ExprResult::Val)
            }
            Self::Return(None) => Err(EvalErrorType::InvalidReturn(ExprResult::None).no_pos()),
            Self::Return(Some(expr)) => {
                let ret = expr.eval(ctx, ectx, loc)?;
                Err(EvalErrorType::InvalidReturn(ret).no_pos())
            }
            Self::Break(None) => Err(EvalErrorType::InvalidBreak(ExprResult::None).no_pos()),
            Self::Break(Some(expr)) => {
                let ret = expr.eval(ctx, ectx, loc)?;
                Err(EvalErrorType::InvalidBreak(ret).no_pos())
            }
            Self::Continue => Err(EvalErrorType::InvalidContinue.no_pos()),
            #[cfg(feature = "parser")]
            Self::Import(i) => i.eval(ctx, ectx, loc),
            Self::Series(s) => s.eval(ctx, ectx, loc),
            Self::SetSeries(s) => s.eval(ctx, ectx, loc),
            Self::MapSeries(s) => s.eval(ctx, ectx, loc),
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
            Self::Multi(..) => true,
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
    fn eval(
        &self,
        _ctx: &TaskContext,
        _ectx: &EvalCtx,
        _loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        Err(EvalErrorType::InvalidContext("this needs to be run in mutable context").no_pos())
    }
    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        _ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        use crate::tasks::Task;

        if let Some(path) = self.path() {
            let txt = std::fs::read_to_string(path).unwrap();
            let tokens = crate::parser::tokenizer::get_tokens(&txt);
            let tasks = crate::parser::tasks::parse(tokens)
                .map_err(|e| EvalErrorType::ParseError(e.to_string()).no_pos())?;
            if self.tasks {
                for fc in tasks {
                    ctx.execute(fc, loc)?;
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
                        ctx.execute(fc, loc)?;
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
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let label = self.label.eval_value(ctx, ectx, loc)?;
        let prog = self.prog.eval_value(ctx, ectx, loc)?;
        let total = self.total.eval_value(ctx, ectx, loc)?;

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
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprContext, EvalErrorType> {
        match &self.ty {
            Some(ty) => ty.get_expr_context(ctx, ectx, loc),
            None => Ok(ectx.expr_ctx.as_ref().clone()),
        }
    }
}

impl Eval for InputVar {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        // if type is none then first priotize local
        if self.ty.is_none() {
            if let Ok(Some(v)) = self.attr_nested(loc) {
                return Ok(ExprResult::Val(v));
            }
        }
        let expr_ctx = if let Some(s) = &self.ty {
            Cow::Owned(s.get_expr_context(ctx, ectx, loc)?)
        } else {
            ectx.expr_ctx.clone()
        };
        let attr = match expr_ctx.as_ref() {
            ExprContext::Local(_) => self.attr_nested(loc)?,
            ExprContext::Env(_) => self.attr_nested(&ctx.env)?,
            ExprContext::Network(_) => self.attr_nested(&ctx.network)?,
            ExprContext::Node(n) => {
                let name = n.name().to_string();
                let n = &n
                    .try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?;
                self.attr_nested(n)
                    .map_err(|e| e.pos(self.position()).node(name))?
            }
            ExprContext::Nodes(nds) => {
                let mut vars = Vec::<ExprResult>::with_capacity(nds.len());
                for i in nds {
                    let name = i.name().to_string();
                    let n = &i
                        .try_lock()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?;
                    let a = self.attr_nested(n);
                    vars.push(a.map_err(|e| e.pos(self.position()).node(name))?.into());
                }
                return Ok(ExprResult::Arr(vars));
            }
            ExprContext::NodesMap(nds) => {
                let res: Vec<(String, ExprResult)> = nds
                    .iter()
                    .map(|i| {
                        let a = self.attr_nested(&i.try_lock().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?);
                        Ok((
                            i.name().to_string(),
                            ExprResult::from(a.map_err(|e| e.pos(self.position()))?),
                        ))
                    })
                    .collect::<Result<_, EvalError>>()?;
                return Ok(ExprResult::Map(res));
            }
        };

        match attr {
            Some(v) => Ok(ExprResult::Val(v)),
            None => Ok(ExprResult::NoneErr(Box::new(
                EvalErrorType::AttributeError(format!("Attribute {self} not found"))
                    .pos(self.position()),
            ))),
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
    /// Node variable with variable to get the node name from
    NodeVar(Box<InputVar>),
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
    /// Edges variable (only valid in a node function for a node with single input or output, i.e. leaf and root)
    Edge,
    /// Edges variable (only valid in a node function)
    Edges,
    /// Edges variable in map format (only valid in a node function)
    EdgesMap,
    /// Nodes variable (array of variable from each node)
    Nodes(Box<Propagation>),
    /// Nodes variable (map of variable from each node and its name)
    NodesMap(Box<Propagation>),
    /// variable for the Root node of the network
    Root,
    /// Roots of the network
    Roots,
    /// Roots of the network in map format
    RootsMap,
    /// leaf nodes of the network
    Leaf,
    /// leaf nodes of the network
    Leaves,
    /// leaf nodes of the network in map format
    LeavesMap,
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
            TaskKeyword::Edge => Some(VarType::Edge),
            TaskKeyword::Edges => Some(VarType::Edges),
            TaskKeyword::EdgesMap => Some(VarType::EdgesMap),
            TaskKeyword::Nodes => Some(VarType::Nodes(Box::new(prop.unwrap_or_default()))),
            TaskKeyword::NodesMap => Some(VarType::NodesMap(Box::new(prop.unwrap_or_default()))),
            TaskKeyword::Root => Some(VarType::Root),
            TaskKeyword::Roots => Some(VarType::Roots),
            TaskKeyword::RootsMap => Some(VarType::RootsMap),
            TaskKeyword::Leaf => Some(VarType::Leaf),
            TaskKeyword::Leaves => Some(VarType::Leaves),
            TaskKeyword::LeavesMap => Some(VarType::LeavesMap),
            _ => None,
        }
    }

    /// Convert to [`FunctionType`]
    pub fn to_functiontype(&self) -> &FunctionType {
        match self {
            VarType::Node(_) | VarType::NodeVar(_) => &FunctionType::Node,
            VarType::Network => &FunctionType::Network,
            VarType::Env | VarType::Local => &FunctionType::Env,
            VarType::Input
            | VarType::Inputs
            | VarType::InputsMap
            | VarType::Output
            | VarType::Outputs
            | VarType::OutputsMap
            | VarType::Edge
            | VarType::Edges
            | VarType::EdgesMap
            | VarType::Nodes(_)
            | VarType::NodesMap(_)
            | VarType::Root
            | VarType::Roots
            | VarType::RootsMap
            | VarType::Leaf
            | VarType::Leaves
            | VarType::LeavesMap => &FunctionType::Node,
        }
    }

    /// indicate if the variable type returns a map or not
    pub fn is_map(&self) -> bool {
        match self {
            VarType::Node(_)
            | VarType::NodeVar(_)
            | VarType::Network
            | VarType::Env
            | VarType::Local
            | VarType::Input
            | VarType::Inputs
            | VarType::Output
            | VarType::Outputs
            | VarType::Edge
            | VarType::Edges
            | VarType::Nodes(_)
            | VarType::Root
            | VarType::Roots
            | VarType::Leaf
            | VarType::Leaves => false,
            VarType::NodesMap(_)
            | VarType::OutputsMap
            | VarType::EdgesMap
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
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprContext, EvalErrorType> {
        let nodes_func = if self.is_map() {
            |nds| Ok(ExprContext::NodesMap(nds))
        } else {
            |nds| Ok(ExprContext::Nodes(nds))
        };

        let node: Option<&Node> = ectx.curr_node();
        match (self, node) {
            (VarType::Local, _) => Ok(ExprContext::Local(node.cloned())),
            (VarType::Env, _) => Ok(ExprContext::Env(node.cloned())),
            (VarType::Network, _) => Ok(ExprContext::Network(node.cloned())),
            (VarType::Nodes(prop) | VarType::NodesMap(prop), _) => nodes_func(
                ctx.propagation(*prop.clone(), ectx, loc)
                    .map_err(|e| *e.ty)?,
            ),
            (VarType::Roots | VarType::RootsMap, _) => {
                nodes_func(ctx.network.roots().cloned().collect())
            }
            (VarType::Leaves | VarType::LeavesMap, _) => {
                nodes_func(ctx.network.leaves().cloned().collect())
            }
            (VarType::Leaf, _) => match ctx.network.leaf() {
                Some(r) => Ok(ExprContext::Node(r.clone())),
                None => Err(EvalErrorType::NoRootNode),
            },
            (VarType::Root, _) => match ctx.network.root() {
                Some(r) => Ok(ExprContext::Node(r.clone())),
                None => Err(EvalErrorType::NoRootNode),
            },
            (VarType::Node(Some(n)), _) => match ctx.network.node_by_name(n) {
                Some(n) => Ok(ExprContext::Node(n.clone())),
                None => Err(EvalErrorType::NodeNotFound(n.to_string())),
            },
            (VarType::Node(None), Some(n)) => Ok(ExprContext::Node(n.clone())),
            (VarType::NodeVar(var), _) => {
                let val = var.eval_value(ctx, ectx, loc).map_err(|e| *e.ty)?;
                let nd =
                    String::try_from_attr(&val).map_err(|e| EvalErrorType::AttributeError(e))?;
                let n = ctx
                    .network
                    .node_by_name(&nd)
                    .ok_or(EvalErrorType::NodeNotFound(nd))?;
                Ok(ExprContext::Node(n.clone()))
            }
            (VarType::Input, Some(n)) => match n
                .try_lock()
                .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                .input()
            {
                RSome(r) => Ok(ExprContext::Node(r.clone())),
                RNone => Err(EvalErrorType::NoInputNodes),
            },
            (VarType::Output, Some(n)) => match n
                .try_lock()
                .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                .output()
            {
                RSome(r) => Ok(ExprContext::Node(r.clone())),
                RNone => Err(EvalErrorType::NoOutputNode),
            },
            (VarType::Edge, Some(n)) => match n
                .try_lock()
                .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                .edge()
            {
                Some(r) => Ok(ExprContext::Node(r.clone())),
                None => Err(EvalErrorType::NoEdgeNode),
            },
            (VarType::Inputs | VarType::InputsMap, Some(n)) => nodes_func(
                n.try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                    .inputs()
                    .to_vec(),
            ),
            (VarType::Outputs | VarType::OutputsMap, Some(n)) => nodes_func(
                n.try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                    .outputs()
                    .to_vec(),
            ),
            (VarType::Edges | VarType::EdgesMap, Some(n)) => nodes_func(
                n.try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()))?
                    .edges(),
            ),
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
            VarType::NodeVar(var) => {
                return write!(f, "node[*{}]", var);
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
            VarType::Edge => "edge",
            VarType::Edges => "edges",
            VarType::EdgesMap => "edgesmap",
            VarType::Nodes(p) => {
                return write!(f, "nodes{p}");
            }
            VarType::NodesMap(p) => {
                return write!(f, "nodesmap{p}");
            }
            VarType::Root => "root",
            VarType::Roots => "outlets",
            VarType::RootsMap => "outletsmap",
            VarType::Leaf => "leaf",
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

    fn get_eval_context<'a, 'b>(
        &self,
        ctx: &TaskContext,
        expr: &EvalCtx<'a>,
        loc: &'b mut AttrMap,
    ) -> Result<EvalCtx<'a>, EvalErrorType> {
        match &self.ty {
            Some(ty) => ty
                .get_expr_context(ctx, expr, loc)
                .map(|e| EvalCtx::expr_ctx(Cow::Owned(e))),
            None => Ok(expr.clone()),
        }
    }
}

impl<'a> FunctionCall<ResolvedExpr<'a>> {
    pub fn function_ctx(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<FunctionCtx<'static>, EvalError> {
        let mut args = Vec::with_capacity(self.args.len());
        for a in &self.args {
            match &a.expr {
                ExprType::Args(vt) => {
                    // eval, take an array, and insert it
                    let parent = Vec::<Attribute>::from_attr(&vt.eval_value(ctx, &a.context, loc)?)
                        .ok_or(EvalErrorType::NotAnArray.pos(a.position()))?;
                    for v in parent {
                        args.push(v.into());
                    }
                }
                v => args.push(
                    v.eval(ctx, &a.context, loc)
                        .map_err(|e| e.pos(a.position()))?
                        .to_function_input()?,
                ),
            }
        }
        let mut kwargs = HashMap::with_capacity(self.kwargs.len());
        for (k, a) in &self.kwargs {
            match &a.expr {
                ExprType::KwArgs(vt) => {
                    // eval, take an array, and insert it
                    let parent = AttrMap::from_attr(&vt.eval_value(ctx, &a.context, loc)?)
                        .ok_or(EvalErrorType::NotAnArray.pos(a.position()))?;
                    for Tuple2(k, v) in parent {
                        kwargs.insert(k.to_string(), v.into());
                    }
                }
                v => {
                    kwargs.insert(
                        k.clone(),
                        v.eval(ctx, ectx, loc)
                            .map_err(|e| e.pos(self.position()))?
                            .to_function_input()?,
                    );
                }
            }
        }
        Ok(FunctionCtx::from_arg_kwarg(args, kwargs))
    }
}

impl Eval for FunctionCall<ResolvedExpr<'_>> {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let ectx = self.get_eval_context(ctx, ectx, loc)?;
        match &ectx.expr_ctx.as_ref() {
            ExprContext::Local(_) | ExprContext::Env(_) => {
                let func_ctx = self.function_ctx(ctx, &ectx, loc)?;
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func.eval(ctx, &ectx, func_ctx);
                }
                match ctx.functions.env(&self.name) {
                    Some(f) => f.call(&func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Network(n) => {
                let func_ctx = self.function_ctx(ctx, &ectx, loc)?;
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func.eval(ctx, &ectx, func_ctx);
                }
                match ctx.functions.network(&self.name) {
                    Some(f) => f.call(&ctx.network, &func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval(ctx, &EvalCtx::env(n.clone()), loc),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Node(node) => {
                let name = node
                    .try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(self.position()))?
                    .name()
                    .to_string();
                let etc = EvalCtx::at_node(node.clone());
                let func_ctx = self
                    .function_ctx(ctx, &etc, loc)
                    .map_err(|e| e.node(name.clone()))?;
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func
                        .eval(ctx, &etc, func_ctx)
                        .map_err(|e| e.node(name.clone()));
                }
                match ctx.functions.node(&self.name) {
                    Some(f) => {
                        let n = node.try_lock().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        f.call(&n, &func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                                .node(name.clone())
                        })
                    }
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self
                        .eval(ctx, &EvalCtx::env(Some(node.clone())), loc)
                        .map_err(|e| e.node(name.clone())),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Nodes(nds) => {
                let res: Vec<ExprResult> = nds
                    .iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let etc = EvalCtx::at_node(n.clone());
                        let func_ctx = self
                            .function_ctx(ctx, &etc, loc)
                            .map_err(|e| e.node(name.clone()))?;
                        if let Some(func) = ctx.udf(&self.name).cloned() {
                            // priority for the locally defined function
                            return func
                                .eval(ctx, &etc, func_ctx)
                                .map_err(|e| e.node(name.clone()));
                        }

                        match ctx.functions.node(&self.name) {
                            Some(f) => {
                                let n = n.try_lock().ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?;
                                f.call(&n, &func_ctx).res().map_err(|s| {
                                    EvalErrorType::FunctionError(self.name.to_string(), s)
                                        .pos(self.position())
                                        .node(name.clone())
                                })
                            }
                            // if the function is not called by explicit type then also test environment function
                            None if self.ty.is_none() => {
                                self.eval(ctx, &EvalCtx::env(Some(n.clone())), loc)
                            }
                            None => Err(EvalErrorType::FunctionNotFound(
                                Some(ectx.expr_ctx.as_ref().function_type().clone()),
                                self.name.to_string(),
                            )
                            .pos(self.position())),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExprResult::Arr(res))
            }
            ExprContext::NodesMap(nds) => {
                let res: Vec<(String, ExprResult)> = nds
                    .iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let etc = EvalCtx::at_node(n.clone());
                        let func_ctx = self
                            .function_ctx(ctx, &etc, loc)
                            .map_err(|e| e.node(name.clone()))?;
                        if let Some(func) = ctx.udf(&self.name).cloned() {
                            // priority for the locally defined function
                            return func
                                .eval(ctx, &etc, func_ctx)
                                .map(|v| (name.clone(), v))
                                .map_err(|e| e.node(name.clone()));
                        }

                        match ctx.functions.node(&self.name) {
                            Some(f) => {
                                let n = n.try_lock().ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?;
                                f.call(&n, &func_ctx).res().map_err(|s| {
                                    EvalErrorType::FunctionError(self.name.to_string(), s)
                                        .pos(self.position())
                                        .node(name.clone())
                                })
                            }
                            // if the function is not called by explicit type then also test environment function
                            None if self.ty.is_none() => {
                                self.eval(ctx, &EvalCtx::env(Some(n.clone())), loc)
                            }
                            None => Err(EvalErrorType::FunctionNotFound(
                                Some(ectx.expr_ctx.as_ref().function_type().clone()),
                                self.name.to_string(),
                            )
                            .pos(self.position())),
                        }
                        .map(|v| (name, v))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(ExprResult::Map(res))
            }
        }
    }

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let ectx = self.get_eval_context(ctx, ectx, loc)?;
        match &ectx.expr_ctx.as_ref() {
            ExprContext::Local(_) | ExprContext::Env(_) => {
                let func_ctx = self.function_ctx(ctx, &ectx, loc)?;
                ctx.mark_change();
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func.eval_mut(ctx, &ectx, func_ctx);
                }
                match ctx.functions.env(&self.name) {
                    Some(f) => f.call(&func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Network(n) => {
                let func_ctx = self.function_ctx(ctx, &ectx, loc)?;
                ctx.mark_change();
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func.eval_mut(ctx, &ectx, func_ctx);
                }
                match ctx.functions.network(&self.name) {
                    Some(f) => f.call_mut(&mut ctx.network, &func_ctx).res().map_err(|s| {
                        EvalErrorType::FunctionError(self.name.to_string(), s).pos(self.position())
                    }),
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self.eval_mut(ctx, &EvalCtx::env(n.clone()), loc),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Node(node) => {
                let name = node.name().to_string();
                let etc = EvalCtx::at_node(node.clone());
                let func_ctx = self
                    .function_ctx(ctx, &etc, loc)
                    .map_err(|e| e.node(name.clone()))?;
                ctx.mark_change();
                if let Some(func) = ctx.udf(&self.name).cloned() {
                    // priority for the locally defined function
                    return func
                        .eval_mut(ctx, &etc, func_ctx)
                        .map_err(|e| e.node(name.clone()));
                }
                match ctx.functions.node(&self.name) {
                    Some(f) => {
                        let n = &mut node.try_lock().ok_or(
                            EvalErrorType::MutexError(file!(), line!()).pos(self.position()),
                        )?;
                        f.call_mut(n, &func_ctx).res().map_err(|s| {
                            EvalErrorType::FunctionError(self.name.to_string(), s)
                                .pos(self.position())
                                .node(name.clone())
                        })
                    }
                    // if the function is not called by explicit type then also test environment function
                    None if self.ty.is_none() => self
                        .eval_mut(ctx, &EvalCtx::env(Some(node.clone())), loc)
                        .map_err(|e| e.node(name.clone())),
                    None => Err(EvalErrorType::FunctionNotFound(
                        Some(ectx.expr_ctx.as_ref().function_type().clone()),
                        self.name.to_string(),
                    )
                    .pos(self.position())),
                }
            }
            ExprContext::Nodes(nds) => {
                let res: Vec<ExprResult> = nds
                    .iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let etc = EvalCtx::at_node(n.clone());
                        let func_ctx = self
                            .function_ctx(ctx, &etc, loc)
                            .map_err(|e| e.node(name.clone()))?;
                        if let Some(func) = ctx.udf(&self.name).cloned() {
                            // priority for the locally defined function
                            return func
                                .eval_mut(ctx, &etc, func_ctx)
                                .map_err(|e| e.node(name.clone()));
                        }

                        match ctx.functions.node(&self.name) {
                            Some(f) => {
                                let n = &mut n.try_lock().ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?;
                                f.call_mut(n, &func_ctx).res().map_err(|s| {
                                    EvalErrorType::FunctionError(self.name.to_string(), s)
                                        .pos(self.position())
                                        .node(name.clone())
                                })
                            }
                            // if the function is not called by explicit type then also test environment function
                            None if self.ty.is_none() => self
                                .eval_mut(ctx, &EvalCtx::env(Some(n.clone())), loc)
                                .map_err(|e| e.node(name.clone())),
                            None => Err(EvalErrorType::FunctionNotFound(
                                Some(ectx.expr_ctx.as_ref().function_type().clone()),
                                self.name.to_string(),
                            )
                            .pos(self.position())),
                        }
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ctx.mark_change();
                Ok(ExprResult::Arr(res))
            }
            ExprContext::NodesMap(nds) => {
                let res: Vec<(String, ExprResult)> = nds
                    .iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let etc = EvalCtx::at_node(n.clone());
                        let func_ctx = self
                            .function_ctx(ctx, &etc, loc)
                            .map_err(|e| e.node(name.clone()))?;
                        if let Some(func) = ctx.udf(&self.name).cloned() {
                            // priority for the locally defined function
                            return func
                                .eval_mut(ctx, &etc, func_ctx)
                                .map(|v| (name.clone(), v))
                                .map_err(|e| e.node(name.clone()));
                        }

                        match ctx.functions.node(&self.name) {
                            Some(f) => {
                                let n = &mut n.try_lock().ok_or(
                                    EvalErrorType::MutexError(file!(), line!())
                                        .pos(self.position()),
                                )?;
                                f.call_mut(n, &func_ctx).res().map_err(|s| {
                                    EvalErrorType::FunctionError(self.name.to_string(), s)
                                        .pos(self.position())
                                        .node(name.clone())
                                })
                            }
                            // if the function is not called by explicit type then also test environment function
                            None if self.ty.is_none() => self
                                .eval_mut(ctx, &EvalCtx::env(Some(n.clone())), loc)
                                .map_err(|e| e.node(name.clone())),
                            None => Err(EvalErrorType::FunctionNotFound(
                                Some(ectx.expr_ctx.as_ref().function_type().clone()),
                                self.name.to_string(),
                            )
                            .pos(self.position())),
                        }
                        .map(|v| (name, v))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                ctx.mark_change();
                Ok(ExprResult::Map(res))
            }
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
pub struct GetSeries {
    ty: Option<VarType>,
    is_ts: bool,
    name: String,
    index: Option<Box<RawExpr>>,
    pub(crate) check: bool,
}

impl std::fmt::Display for GetSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}{}{}",
            self.ty
                .as_ref()
                .map(|t| format!("{}", t))
                .unwrap_or_default(),
            if self.is_ts { "$$" } else { "$" },
            self.name,
            self.index
                .as_ref()
                .map(|i| format!("{i}"))
                .unwrap_or_default(),
            if self.check { "?" } else { "" },
        )
    }
}

impl GetSeries {
    pub fn new(
        ty: Option<VarType>,
        is_ts: bool,
        name: String,
        index: Option<RawExpr>,
        check: bool,
    ) -> Self {
        Self {
            ty,
            is_ts,
            name,
            index: index.map(Box::new),
            check,
        }
    }

    fn get_expr_context(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprContext, EvalErrorType> {
        match &self.ty {
            Some(ty) => ty.get_expr_context(ctx, ectx, loc),
            None => Ok(ectx.expr_ctx.as_ref().clone()),
        }
    }
}

enum SeriesIndex {
    None,
    One(usize),
    Multi(Vec<usize>),
}

impl SeriesIndex {
    fn subset(&self, sr: EvalSeriesRes) -> Result<ExprResult, EvalError> {
        match self {
            Self::None => Ok(sr.into()),
            Self::One(i) => match sr.series().get_attribute(*i) {
                Some(Some(a)) => Ok(ExprResult::Val(a)),
                Some(None) => Ok(ExprResult::None),
                None => Ok(ExprResult::NoneErr(Box::new(
                    EvalErrorType::IndexError.no_pos(),
                ))),
            },
            Self::Multi(inds) => {
                let mut vals = Vec::new();
                let sr = sr.series(); // TODO index timeseries to make another timeseries
                for i in inds {
                    match sr.get_attribute(*i) {
                        Some(Some(a)) => vals.push(RSome(a)),
                        Some(None) => vals.push(RNone),
                        None => return Err(EvalErrorType::IndexError.no_pos()),
                    }
                }
                Ok(ExprResult::Series(
                    MaskedSeries::attributes(vals).retype().into(),
                ))
            }
        }
    }
}

impl Eval for GetSeries {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let expr_ctx = if let Some(s) = &self.ty {
            Cow::Owned(s.get_expr_context(ctx, ectx, loc)?)
        } else {
            ectx.expr_ctx.clone()
        };
        let index = match &self.index {
            Some(i) => match i.eval_value(ctx, ectx, loc)? {
                Attribute::Array(ar) => {
                    let inds = ar
                        .iter()
                        .map(usize::try_from_attr)
                        .collect::<Result<Vec<usize>, String>>()
                        .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?;
                    SeriesIndex::Multi(inds)
                }
                Attribute::Integer(i) => SeriesIndex::One(
                    usize::try_from(i).map_err(|_| EvalErrorType::IndexError.no_pos())?,
                ),
                a => {
                    return Err(EvalErrorType::InvalidAttributeType(
                        NadiAttrType::Integer,
                        a.dtype(),
                    )
                    .no_pos());
                }
            },
            None => SeriesIndex::None,
        };
        // TODO: take values out based on index
        match expr_ctx.as_ref() {
            ExprContext::Local(_) => {
                Err(EvalErrorType::NotImplementedError("local series not supported").no_pos())
            }
            ExprContext::Env(_) => {
                index.subset(get_series_or_ts(&ctx.env, &self.name, self.is_ts)?)
            }
            ExprContext::Network(_) => {
                index.subset(get_series_or_ts(&ctx.network, &self.name, self.is_ts)?)
            }
            ExprContext::Node(n) => {
                let n = &n
                    .try_lock()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?;
                let n: &NodeInner = n;
                index.subset(get_series_or_ts(n, &self.name, self.is_ts)?)
            }
            // Nodes series will error out on masked series with gaps, use nodesmap keywords
            ExprContext::Nodes(nds) => {
                let inp_series = nds
                    .iter()
                    .map(|i| get_node_series_or_ts(i, &self.name, self.is_ts))
                    .collect::<Result<Vec<EvalSeriesRes>, EvalError>>()?;
                let sr_lengths: Vec<usize> = inp_series.iter().map(|s| s.len()).collect();
                if sr_lengths.is_empty() {
                    // we don't have a identity series when we need an empty one
                    return Ok(ExprResult::None);
                }
                if let Some(l) = sr_lengths.iter().find(|l| **l != sr_lengths[0]) {
                    return Err(EvalErrorType::DifferentLength(sr_lengths[0], *l).no_pos());
                }
                let mut compl_series = Vec::with_capacity(inp_series.len());
                let timeline = inp_series
                    .iter()
                    .filter_map(|s| match s {
                        EvalSeriesRes::Ts(ts) => Some(ts.timeline().clone()),
                        _ => None,
                    })
                    .next();
                for ser in inp_series {
                    compl_series.push(
                        ser.series()
                            .to_attributes()
                            .ok_or(EvalErrorType::EmptyValue(None).no_pos())?
                            .into_iter(),
                    );
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
                // FIX: remove extra calculation if we are indexing anyway
                index.subset(EvalSeriesRes::new(
                    CompleteSeries::attributes(zipped_vals).into(),
                    timeline,
                ))
            }
            ExprContext::NodesMap(nds) => {
                let inp_series = nds
                    .iter()
                    .map(|i| {
                        let sr = get_node_series_or_ts(i, &self.name, self.is_ts)?;
                        Ok((i.name().to_string(), sr))
                    })
                    .collect::<Result<Vec<(String, EvalSeriesRes)>, EvalError>>()?;
                let sr_lengths: Vec<usize> = inp_series.iter().map(|(_, s)| s.len()).collect();
                if sr_lengths.is_empty() {
                    return Ok(ExprResult::None);
                }
                if let Some(l) = sr_lengths.iter().find(|l| **l != sr_lengths[0]) {
                    return Err(EvalErrorType::DifferentLength(sr_lengths[0], *l).no_pos());
                }
                let timeline = inp_series
                    .iter()
                    .filter_map(|(_, s)| match s {
                        EvalSeriesRes::Ts(ts) => Some(ts.timeline().clone()),
                        _ => None,
                    })
                    .next();
                let mut compl_series = Vec::with_capacity(inp_series.len());
                let mut masked_series = Vec::with_capacity(inp_series.len());
                for (inp, ser) in inp_series {
                    match ser.series() {
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
                index.subset(EvalSeriesRes::new(
                    CompleteSeries::attributes(zipped_vals).into(),
                    timeline,
                ))
            }
        }
    }
}

/// Series (es) with a function to map a value
#[derive(Clone, Debug, PartialEq)]
pub struct MapSeries {
    /// one or multiple series
    series: Vec<GetSeries>,
    /// function pointer or function definition
    func: MapFunction,
    /// accumulate instead of independent map
    accum: bool,
}

impl std::fmt::Display for MapSeries {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self.series.as_slice() {
            [v] => write!(
                f,
                "{v} {}-> {}",
                if self.accum { "+" } else { "" },
                self.func
            ),
            m => write!(
                f,
                "({}) {}-> {}",
                m.iter()
                    .map(|s| format!("{s}"))
                    .collect::<Vec<String>>()
                    .join(", "),
                if self.accum { "+" } else { "" },
                self.func
            ),
        }
    }
}

impl MapSeries {
    pub fn new(series: Vec<GetSeries>, func: MapFunction, accum: bool) -> Self {
        Self {
            series,
            func,
            accum,
        }
    }
}

impl Eval for MapSeries {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let mut srs: Vec<Vec<ROption<Attribute>>> = Vec::with_capacity(self.series.len());
        let mut timeline = None;
        for s in &self.series {
            let res = s.eval(ctx, ectx, loc)?;
            if let ExprResult::TimeSeries(ts) = &res {
                // keep the first arg's timeline for later
                timeline = Some(timeline.unwrap_or(ts.timeline().clone()));
            }
            let opt = res.to_opt_attributes();
            srs.push(opt.ok_or(EvalErrorType::NotAnArray.no_pos())?);
        }
        let sr_lengths: Vec<usize> = srs.iter().map(|s| s.len()).collect();
        if sr_lengths.is_empty() {
            return Ok(ExprResult::None);
        }
        if let Some(l) = sr_lengths.iter().find(|l| **l != sr_lengths[0]) {
            return Err(EvalErrorType::DifferentLength(sr_lengths[0], *l).no_pos());
        }
        match &self.func {
            MapFunction::Defn(udf) => {
                // udf uses kwargs to support empty values; pointer doesn't
                let func_call = |kwargs| {
                    let fctx = FunctionCtx::from_arg_kwarg(vec![], kwargs);
                    // eval udf with node/network context
                    udf.eval(ctx, &EvalCtx::default(), fctx)
                };
                let names = udf.arg_names();
                let has_cum_arg = names.iter().skip(sr_lengths.len()).any(|n| n == &"LAST");
                // next extra arg after all the positional args for series tuple
                if has_cum_arg {
                    let mut last_res: Option<Attribute> = None;
                    let mut all_res: Vec<ROption<Attribute>> = if self.accum {
                        Vec::with_capacity(0)
                    } else {
                        Vec::with_capacity(sr_lengths[0])
                    };
                    for i in 0..sr_lengths[0] {
                        let mut kwargs: HashMap<String, FunctionInput> = srs
                            .iter()
                            .zip(&names)
                            .filter_map(|(s, n)| match &s[i] {
                                RSome(v) => Some((n.to_string(), FunctionInput::Attr(v))),
                                RNone => None,
                            })
                            .collect();
                        if let Some(ref v) = last_res {
                            kwargs.insert("LAST".to_string(), FunctionInput::AttrOwn(v.clone()));
                        }
                        let res = func_call(kwargs)?.to_attribute();
                        if !self.accum {
                            all_res.push(res.clone().into());
                        }
                        // update last_res if function returned a
                        // value, otherwise keep the last value
                        // instead of the default value: this
                        // allows users to skip values by
                        // returning None from the function
                        if let Some(v) = res {
                            last_res = Some(v);
                        }
                    }
                    if self.accum {
                        Ok(last_res.into())
                    } else {
                        let sr: Series = MaskedSeries::attributes(all_res).retype().into();
                        Ok(if let Some(tl) = timeline {
                            ExprResult::TimeSeries(TimeSeries::new(tl, sr))
                        } else {
                            ExprResult::Series(sr)
                        })
                    }
                } else if self.accum {
                    Err(EvalErrorType::FunctionError(
                        udf.name().unwrap_or("series_accum").to_string(),
                        "not enough arguments to accumulate values".into(),
                    )
                    .no_pos())
                } else {
                    let vals = (0..sr_lengths[0])
                        .map(|i| {
                            let kwargs = srs
                                .iter()
                                .zip(&names)
                                .filter_map(|(s, n)| match &s[i] {
                                    RSome(v) => Some((n.to_string(), FunctionInput::Attr(v))),
                                    RNone => None,
                                })
                                .collect();
                            Ok(func_call(kwargs)?.to_attribute().into())
                        })
                        .collect::<Result<Vec<ROption<Attribute>>, EvalError>>()?;
                    let sr: Series = MaskedSeries::attributes(vals).retype().into();
                    Ok(if let Some(tl) = timeline {
                        ExprResult::TimeSeries(TimeSeries::new(tl, sr))
                    } else {
                        ExprResult::Series(sr)
                    })
                }
            }
            MapFunction::Pointer(name) => {
                let func_call = |args: Vec<FunctionInput>| {
                    let fctx = FunctionCtx::from_arg_kwarg(args, HashMap::new());
                    match ctx.udf(name).cloned() {
                        // priority for the locally defined function; evaluated in local context
                        Some(func) => func.eval(ctx, &EvalCtx::default(), fctx),
                        _ => match ctx.functions.env(name) {
                            Some(f) => f.call(&fctx).res().map_err(|e| {
                                EvalErrorType::FunctionError(name.to_string(), e.to_string())
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
                let vals = (0..sr_lengths[0])
                    .map(|i| {
                        let args = srs.iter().map(|s| FunctionInput::from(&s[i])).collect();
                        Ok(func_call(args)?.to_attribute().into())
                    })
                    .collect::<Result<Vec<ROption<Attribute>>, EvalError>>()?;
                let sr: Series = MaskedSeries::attributes(vals).retype().into();
                Ok(if let Some(tl) = timeline {
                    ExprResult::TimeSeries(TimeSeries::new(tl, sr))
                } else {
                    ExprResult::Series(sr)
                })
            }
        }
    }
}

/// Set a series from results of an expression
///
/// the expression should result in a series, timeseries, or array of attributes
#[derive(Clone, PartialEq, Debug)]
pub struct SetSeries<T> {
    inp: GetSeries,
    ty: Option<NadiAttrType>,
    expr: Box<T>,
    ctx: Option<ExprContext>,
}

impl<T> SetSeries<T> {
    pub fn new(inp: GetSeries, ty: Option<NadiAttrType>, expr: T) -> Self {
        Self {
            inp,
            ty,
            expr: Box::new(expr),
            ctx: None,
        }
    }
}

impl<T: std::fmt::Display> std::fmt::Display for SetSeries<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{} = {}",
            self.inp,
            self.ty
                .as_ref()
                .map(|t| format!(": {t}"))
                .unwrap_or_default(),
            self.expr
        )
    }
}

fn resolve_set_series<'b>(
    expr: SetSeries<RawExpr>,
    ctx: &TaskContext,
    ectx: EvalCtx<'b>,
) -> Result<ResolvedExpr<'b>, EvalError> {
    let e = expr.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref());
    let new_expr_ctx = expr.inp.get_expr_context(
        ctx,
        &EvalCtx::expr_ctx(Cow::Borrowed(e)),
        // Same as before, you can't use local variables to set a series, but you can get it
        &mut AttrMap::new(),
    )?;
    let etc = match new_expr_ctx {
        ExprContext::Local(n) => EvalCtx::local(n.clone()),
        ExprContext::Env(n) => EvalCtx::env(n.clone()),
        ExprContext::Network(n) => EvalCtx::network(n.clone()),
        ExprContext::Node(n) => EvalCtx::at_node(n),
        ExprContext::Nodes(nds) | ExprContext::NodesMap(nds) => {
            let exprs = nds
                .into_iter()
                .map(|n| {
                    let context = EvalCtx::at_node(n.clone()).to_owned();
                    let mut inp = expr.inp.clone();
                    _ = inp.ty.replace(VarType::Node(None));
                    Ok(ResolvedExpr {
                        expr: ExprType::SetSeries(SetSeries::new(
                            inp,
                            expr.ty.clone(),
                            expr.expr.clone().resolve(ctx, context.clone())?,
                        )),
                        position: expr.expr.position(),
                        context,
                    })
                })
                .collect::<Result<Vec<ResolvedExpr>, EvalError>>()?;
            // FIX: how to know whether the user is looking for array or statement?
            let et = ExprType::Multi(exprs, true);
            return Ok(ResolvedExpr {
                position: expr.expr.position(),
                expr: et,
                context: EvalCtx::env(None),
            });
        }
    };

    Ok(ResolvedExpr {
        expr: ExprType::SetSeries(SetSeries::new(
            expr.inp,
            expr.ty.clone(),
            expr.expr.clone().resolve(ctx, etc.clone())?,
        )),
        position: expr.expr.position(),
        context: etc,
    })
}

enum EvalSeriesRes {
    Sr(Series),
    Ts(TimeSeries),
}

impl From<EvalSeriesRes> for ExprResult {
    fn from(val: EvalSeriesRes) -> ExprResult {
        match val {
            EvalSeriesRes::Sr(s) => ExprResult::Series(s),
            EvalSeriesRes::Ts(s) => ExprResult::TimeSeries(s),
        }
    }
}

impl EvalSeriesRes {
    fn new(sr: Series, tl: Option<TimeLine>) -> Self {
        if let Some(tl) = tl {
            Self::Ts(TimeSeries::new(tl, sr))
        } else {
            Self::Sr(sr)
        }
    }

    fn len(&self) -> usize {
        match self {
            Self::Sr(s) => s.len(),
            Self::Ts(ts) => ts.len(),
        }
    }

    fn series(self) -> Series {
        match self {
            Self::Sr(s) => s,
            Self::Ts(ts) => ts.into(),
        }
    }

    fn set_to_any<T: HasSeries + HasTimeSeries>(
        self,
        n: &mut T,
        name: &str,
        is_ts: bool,
    ) -> Result<(), EvalError> {
        if is_ts {
            match self {
                EvalSeriesRes::Ts(ts) => {
                    n.set_ts(name, ts);
                }
                EvalSeriesRes::Sr(sr) => {
                    // to assign a series result as a
                    // timeseries, we'll try to overwrite a
                    // timeseries values with the series is it
                    // exists, otherwise it is an error
                    let oldts = n.ts_mut(name).ok_or_else(|| {
                        EvalErrorType::TimeSeriesNotFound(name.to_string()).no_pos()
                    })?;
                    oldts
                        .replace_series(sr)
                        .map_err(|(a, b)| EvalErrorType::DifferentLength(a, b).no_pos())?;
                }
            }
        } else {
            n.set_series(name, self.series());
        }
        Ok(())
    }

    fn set_to_node(self, n: &Node, name: &str, is_ts: bool) -> Result<(), EvalError> {
        use std::ops::DerefMut;
        let n = &mut n
            .try_lock()
            .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?;
        self.set_to_any(n.deref_mut(), name, is_ts)
    }
}

fn eval_series<T: Eval>(
    expr: &T,
    ctx: &TaskContext,
    ectx: &EvalCtx,
    loc: &mut AttrMap,
) -> Result<EvalSeriesRes, EvalError> {
    let res = expr.eval(ctx, ectx, loc)?;
    Ok(match res {
        ExprResult::Series(s) => EvalSeriesRes::Sr(s),
        ExprResult::TimeSeries(ts) => EvalSeriesRes::Ts(ts),
        ExprResult::Val(a) => EvalSeriesRes::Sr(Series::Complete(
            CompleteSeries::attributes(
                Vec::<Attribute>::from_attr(&a).ok_or(EvalErrorType::NotAnArray.no_pos())?,
            )
            .retype(),
        )),
        ExprResult::Arr(ar) => EvalSeriesRes::Sr(
            MaskedSeries::attributes(ar.into_iter().map(|a| a.to_attribute().into()).collect())
                .retype()
                .into(),
        ),
        _ => return Err(EvalErrorType::NotAnArray.no_pos()),
    })
}

impl<T: Eval> Eval for SetSeries<T> {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        // get ectx based on resolved ctx
        match ectx.expr_ctx.as_ref() {
            ExprContext::Local(_) => {
                Err(EvalErrorType::NotImplementedError("local series not supported").no_pos())
            }
            ExprContext::Env(_) | ExprContext::Network(_) => {
                // env and network can only be modified if the context is in mutable state
                Err(EvalErrorType::InvalidVariableType.no_pos())
            }
            ExprContext::Node(n) => {
                let val = eval_series(self.expr.as_ref(), ctx, &EvalCtx::at_node(n.clone()), loc)?;
                // self.ty.validate_series(val)?;
                val.set_to_node(n, &self.inp.name, self.inp.is_ts)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val =
                        eval_series(self.expr.as_ref(), ctx, &EvalCtx::at_node(n.clone()), loc)?;
                    // self.ty.validate_series(val)?;
                    val.set_to_node(n, &self.inp.name, self.inp.is_ts)?;
                }
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        // get ectx based on resolved ctx
        match ectx.expr_ctx.as_ref() {
            ExprContext::Local(_) => {
                Err(EvalErrorType::NotImplementedError("local series not supported").no_pos())
            }
            ExprContext::Env(_) => {
                let val = eval_series(self.expr.as_ref(), ctx, ectx, loc)?;
                // self.ty.validate_series(val)?;
                val.set_to_any(&mut ctx.env, &self.inp.name, self.inp.is_ts)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Network(_) => {
                let val = eval_series(self.expr.as_ref(), ctx, ectx, loc)?;
                // self.ty.validate_series(val)?;
                val.set_to_any(&mut ctx.network, &self.inp.name, self.inp.is_ts)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Node(n) => {
                let val = eval_series(self.expr.as_ref(), ctx, ectx, loc)?;
                // self.ty.validate_series(val)?;
                val.set_to_node(n, &self.inp.name, self.inp.is_ts)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val =
                        eval_series(self.expr.as_ref(), ctx, &EvalCtx::at_node(n.clone()), loc)?;
                    // self.ty.validate_series(val)?;
                    val.set_to_node(n, &self.inp.name, self.inp.is_ts)?;
                }
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }

    // TODO: add eval mut where you can change it for env and network as well
}

fn get_series_or_ts<T: HasSeries + HasTimeSeries>(
    n: &T,
    name: &str,
    ts: bool,
) -> Result<EvalSeriesRes, EvalError> {
    if ts {
        n.try_ts(name)
            .cloned()
            .map(EvalSeriesRes::Ts)
            .map_err(|e| EvalErrorType::TimeSeriesNotFound(e).no_pos())
    } else {
        n.try_series(name)
            .cloned()
            .map(EvalSeriesRes::Sr)
            .map_err(|e| EvalErrorType::SeriesNotFound(e).no_pos())
    }
}

fn get_node_series_or_ts(n: &Node, name: &str, ts: bool) -> Result<EvalSeriesRes, EvalError> {
    use std::ops::Deref;
    let node = n
        .try_lock()
        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?;
    get_series_or_ts(node.deref(), name, ts).map_err(|e| e.node(node.name().to_string()))
}

/// Expression with some context
#[derive(Debug, Clone, PartialEq)]
pub struct ExprWithContext {
    pub ty: VarType,
    pub expr: Box<RawExpr>,
    pub silent: bool,
    pub parallel: bool,
}

impl std::fmt::Display for ExprWithContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (self.silent, self.parallel) {
            (true, false) => write!(f, "{} do {{{}}}", self.ty, self.expr),
            (true, true) => write!(f, "{} dopar {{{}}}", self.ty, self.expr),
            (false, true) => write!(f, "{} par {{{}}}", self.ty, self.expr),
            (false, false) => write!(f, "{} {{{}}}", self.ty, self.expr),
        }
    }
}

impl Eval for ExprWithContext {
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let expr_ctx = self
            .ty
            .get_expr_context(ctx, ectx, loc)
            .map_err(|e| e.no_pos())?;
        let mut context = ectx.clone();
        context.expr_ctx = Cow::Owned(expr_ctx.clone());
        match expr_ctx {
            ExprContext::Local(_) => self.expr.eval(ctx, &context, loc),
            ExprContext::Env(_) => self.expr.eval(ctx, &context, loc),
            ExprContext::Network(_) => self.expr.eval(ctx, &context, loc),
            ExprContext::Node(n) => self.expr.eval(ctx, &EvalCtx::at_node(n).to_owned(), loc),
            ExprContext::Nodes(nds) => {
                if self.silent {
                    let parallel = TaskCtxConsts::parallize_nodes(ctx);
                    if parallel | self.parallel {
                        run_nodes_in_parallel(nds, &ctx, &self.expr)
                    } else {
                        nds.into_iter().try_for_each(|n| {
                            self.expr
                                .eval(ctx, &EvalCtx::at_node(n).to_owned(), loc)
                                .map(|_| ())
                        })?;

                        Ok(ExprResult::None)
                    }
                } else {
                    let exprs = nds
                        .into_iter()
                        .map(|n| self.expr.eval(ctx, &EvalCtx::at_node(n).to_owned(), loc))
                        .collect::<Result<Vec<ExprResult>, EvalError>>()?;
                    Ok(ExprResult::Arr(exprs))
                }
            }
            ExprContext::NodesMap(nds) => {
                let exprs = nds
                    .into_iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let expr = self.expr.eval(ctx, &EvalCtx::at_node(n).to_owned(), loc)?;
                        Ok((name, expr))
                    })
                    .collect::<Result<Vec<(String, ExprResult)>, EvalError>>()?;
                if self.silent {
                    Ok(ExprResult::None)
                } else {
                    Ok(ExprResult::Map(exprs))
                }
            }
        }
    }

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        let expr_ctx = self
            .ty
            .get_expr_context(ctx, ectx, loc)
            .map_err(|e| e.no_pos())?;
        let mut context = ectx.clone();
        context.expr_ctx = Cow::Owned(expr_ctx.clone());
        match expr_ctx {
            ExprContext::Local(_) => self.expr.eval_mut(ctx, &context, loc),
            ExprContext::Env(_) => self.expr.eval_mut(ctx, &context, loc),
            ExprContext::Network(_) => self.expr.eval_mut(ctx, &context, loc),
            ExprContext::Node(n) => self
                .expr
                .eval_mut(ctx, &EvalCtx::at_node(n).to_owned(), loc),
            ExprContext::Nodes(nds) => {
                if self.silent {
                    let parallel = TaskCtxConsts::parallize_nodes(ctx);
                    if parallel | self.parallel {
                        run_nodes_in_parallel(nds, &ctx, &self.expr)
                    } else {
                        nds.into_iter().try_for_each(|n| {
                            self.expr
                                .eval_mut(ctx, &EvalCtx::at_node(n).to_owned(), loc)
                                .map(|_| ())
                        })?;
                        Ok(ExprResult::None)
                    }
                } else {
                    let exprs = nds
                        .into_iter()
                        .map(|n| {
                            self.expr
                                .eval_mut(ctx, &EvalCtx::at_node(n).to_owned(), loc)
                        })
                        .collect::<Result<Vec<ExprResult>, EvalError>>()?;
                    Ok(ExprResult::Arr(exprs))
                }
            }
            ExprContext::NodesMap(nds) => {
                let exprs = nds
                    .into_iter()
                    .map(|n| {
                        let name = n.name().to_string();
                        let expr = self
                            .expr
                            .eval_mut(ctx, &EvalCtx::at_node(n).to_owned(), loc)?;
                        Ok((name, expr))
                    })
                    .collect::<Result<Vec<(String, ExprResult)>, EvalError>>()?;
                if self.silent {
                    Ok(ExprResult::None)
                } else {
                    Ok(ExprResult::Map(exprs))
                }
            }
        }
    }
}

fn run_nodes_in_parallel(
    nodes: Vec<Node>,
    ctx: &TaskContext,
    expr: &RawExpr,
) -> Result<ExprResult, EvalError> {
    {
        let num_jobs = TaskCtxConsts::parallel_cores(ctx);
        let nodes = Arc::new(Mutex::new(nodes));
        let error = Arc::new(Mutex::new(None));
        let ctx = &ctx;
        std::thread::scope(|s| {
            let handles = (0..num_jobs)
                .map(|_| {
                    let nds = nodes.clone();
                    let err = error.clone();
                    s.spawn(move || {
                        loop {
                            // if some other thread errored out, then stop others
                            if err.lock().unwrap().is_some() {
                                break;
                            }
                            let n = nds.lock().unwrap().pop();
                            if let Some(n) = n {
                                let mut new_loc = AttrMap::new();
                                if let Err(e) =
                                    expr.eval(ctx, &EvalCtx::at_node(n).to_owned(), &mut new_loc)
                                {
                                    err.lock().unwrap().replace(e);
                                }
                            } else {
                                break;
                            }
                        }
                    })
                })
                .collect::<Vec<_>>();
            for h in handles {
                _ = h.join();
            }
        });
        let err: &Option<EvalError> = &error.lock().unwrap();
        if let Some(e) = err {
            Err(e.clone())
        } else {
            Ok(ExprResult::None)
        }
    }
}

impl ExprWithContext {
    pub fn new(ty: VarType, expr: RawExpr, silent: bool, parallel: bool) -> Self {
        Self {
            ty,
            expr: Box::new(expr),
            silent,
            parallel,
        }
    }
}

/// The context for an expression to be evaluated in
///
/// This is env by default, unless a keyword is used to change it
#[derive(Clone)]
pub enum ExprContext {
    /// Local context
    Local(Option<Node>),
    /// Environmental context
    Env(Option<Node>),
    /// Network Context
    Network(Option<Node>),
    /// Node context like node, input, output, root
    Node(Node),
    /// Multiple nodes context like inputs, outputs, outlets, leaves
    Nodes(Vec<Node>),
    /// Multiple nodes context with their names
    NodesMap(Vec<Node>),
}

impl Default for ExprContext {
    fn default() -> Self {
        Self::Local(None)
    }
}

impl ExprContext {
    pub fn function_type(&self) -> &FunctionType {
        match self {
            Self::Local(_) | Self::Env(_) => &FunctionType::Env,
            Self::Network(_) => &FunctionType::Network,
            Self::Node(_) | Self::Nodes(_) | Self::NodesMap(_) => &FunctionType::Node,
        }
    }

    pub fn curr_node(&self) -> Option<&Node> {
        match self {
            ExprContext::Node(n)
            | ExprContext::Local(Some(n))
            | ExprContext::Env(Some(n))
            | ExprContext::Network(Some(n)) => Some(n),
            _ => None,
        }
    }
}

impl PartialEq for ExprContext {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Local(None), Self::Local(None)) => true,
            (Self::Local(Some(n1)), Self::Local(Some(n2))) => n1.name() == n2.name(),
            (Self::Env(None), Self::Env(None)) => true,
            (Self::Env(Some(n1)), Self::Env(Some(n2))) => n1.name() == n2.name(),
            (Self::Network(None), Self::Network(None)) => true,
            (Self::Network(Some(n1)), Self::Network(Some(n2))) => n1.name() == n2.name(),
            (Self::Node(n1), Self::Node(n2)) => n1.name() == n2.name(),
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
            Self::Local(_) => write!(f, "local"),
            Self::Env(_) => write!(f, "env"),
            Self::Network(_) => write!(f, "network"),
            Self::Node(n) => write!(f, "node[{:?}]", n.name()),
            Self::Nodes(_) => write!(f, "nodes[...]"),
            Self::NodesMap(_) => write!(f, "nodesmap[...]"),
        }
    }
}

#[derive(Clone)]
pub struct SetVariable<T> {
    var: InputVar,
    ty: Option<NadiAttrType>,
    expr: Box<T>,
    silent: bool,
    ctx: Option<ExprContext>,
}

impl<T> SetVariable<T> {
    fn assert_type(&self, val: &Attribute) -> Result<(), EvalError> {
        if let Some(t) = &self.ty {
            if t != &val.dtype() {
                return Err(EvalErrorType::InvalidAttributeType(t.clone(), val.dtype()).no_pos());
            }
        }
        Ok(())
    }
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
    let new_expr_ctx = expr.var.get_expr_context(
        ctx,
        &EvalCtx::expr_ctx(Cow::Borrowed(e)),
        // TEST: HACK: Check if this is ok, without locals resolve might be incorrect, when is the resolve called?
        &mut AttrMap::new(),
    )?;
    let etc = match new_expr_ctx {
        ExprContext::Local(n) => EvalCtx::local(n.clone()),
        ExprContext::Env(n) => EvalCtx::env(n.clone()),
        ExprContext::Network(n) => EvalCtx::network(n.clone()),
        ExprContext::Node(n) => EvalCtx::at_node(n),
        ExprContext::Nodes(nds) | ExprContext::NodesMap(nds) => {
            let exprs = nds
                .into_iter()
                .map(|n| {
                    let context = EvalCtx::at_node(n.clone()).to_owned();
                    let mut vt = expr.var.clone();
                    _ = vt.ty.replace(VarType::Node(None));
                    Ok(ResolvedExpr {
                        expr: ExprType::SetVar(SetVariable::new(
                            vt,
                            expr.ty.clone(),
                            expr.expr.clone().resolve(ctx, context.clone())?,
                            expr.silent,
                        )),
                        position: expr.expr.position(),
                        context,
                    })
                })
                .collect::<Result<Vec<ResolvedExpr>, EvalError>>()?;
            // FIX: how to know whether the user is looking for array or statement?
            let et = ExprType::Multi(exprs, true);
            return Ok(ResolvedExpr {
                position: expr.expr.position(),
                expr: et,
                context: EvalCtx::env(None),
            });
        }
    };

    Ok(ResolvedExpr {
        expr: ExprType::SetVar(SetVariable::new(
            expr.var,
            expr.ty.clone(),
            expr.expr.clone().resolve(ctx, etc.clone())?,
            expr.silent,
        )),
        position: expr.expr.position(),
        context: etc,
    })
}

impl<T: Clone> SetVariable<T> {
    pub fn new(var: InputVar, ty: Option<NadiAttrType>, expr: T, silent: bool) -> Self {
        Self {
            var,
            ty,
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
    fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        // TODO: look into why this is setting in env context, we need to make it take the context from variable type we are setting.
        let e = self.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref());
        let new_expr_ctx =
            self.var
                .get_expr_context(ctx, &EvalCtx::expr_ctx(Cow::Borrowed(e)), loc)?;

        match new_expr_ctx {
            ExprContext::Local(n) => {
                let val = self.expr.eval_value(ctx, &EvalCtx::local(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(loc, val)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Env(_) | ExprContext::Network(_) => {
                // env and network can only be modified if the context is in mutable state
                Err(EvalErrorType::InvalidVariableType.no_pos())
            }
            ExprContext::Node(n) => {
                let val = self
                    .expr
                    .eval_value(ctx, &EvalCtx::at_node(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(
                    n.try_lock()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                        .attr_map_mut(),
                    val,
                )?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val = self
                        .expr
                        .eval_value(ctx, &EvalCtx::at_node(n.clone()), loc)?;
                    self.assert_type(&val)?;
                    self.var.set_attr_nested(
                        n.try_lock()
                            .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                            .attr_map_mut(),
                        val,
                    )?;
                }
                ctx.mark_change();

                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }

    fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        loc: &mut AttrMap,
    ) -> Result<ExprResult, EvalError> {
        match self.ctx.as_ref().unwrap_or(ectx.expr_ctx.as_ref()) {
            ExprContext::Local(n) => {
                let val = self.expr.eval_value(ctx, &EvalCtx::local(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(loc, val)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Env(n) => {
                let val = self.expr.eval_value(ctx, &EvalCtx::env(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(ctx.env.attr_map_mut(), val)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Network(n) => {
                let val = self
                    .expr
                    .eval_value(ctx, &EvalCtx::network(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(ctx.network.attr_map_mut(), val)?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Node(n) => {
                let val = self
                    .expr
                    .eval_value(ctx, &EvalCtx::at_node(n.clone()), loc)?;
                self.assert_type(&val)?;
                self.var.set_attr_nested(
                    n.try_lock()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                        .attr_map_mut(),
                    val,
                )?;
                ctx.mark_change();
                Ok(ExprResult::None)
            }
            ExprContext::Nodes(nds) => {
                for n in nds {
                    let val = self
                        .expr
                        .eval_value(ctx, &EvalCtx::at_node(n.clone()), loc)?;
                    self.assert_type(&val)?;
                    self.var.set_attr_nested(
                        n.try_lock()
                            .ok_or(EvalErrorType::MutexError(file!(), line!()).no_pos())?
                            .attr_map_mut(),
                        val,
                    )?;
                }
                ctx.mark_change();

                Ok(ExprResult::None)
            }
            ExprContext::NodesMap(_) => Err(EvalErrorType::InvalidVariableType.no_pos()),
        }
    }
}
