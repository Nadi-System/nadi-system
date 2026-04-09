use crate::attrs::{AttrMap, Attribute};
use crate::expressions::{EvalError, EvalErrorType, ExprResult, Expression};
use crate::functions::FunctionCtx;
use crate::node::Node;
use crate::tasks::{FunctionType, TaskContext};
use abi_stable::std_types::{RNone, RSome, RVec};

#[derive(Clone, PartialEq, Debug)]
pub enum LocalExpr {
    Expr(Expression, bool),
    Assign(String, Expression, bool),
}

impl std::fmt::Display for LocalExpr {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Expr(e, b) => write!(f, "{e}{}", if *b { ";" } else { "" }),
            Self::Assign(v, e, b) => write!(f, "{v} = {e}{}", if *b { ";" } else { "" }),
        }
    }
}

impl LocalExpr {
    pub fn expr(&self) -> &Expression {
        match self {
            Self::Expr(expr, _) => expr,
            Self::Assign(_, expr, _) => expr,
        }
    }

    pub fn var(&self) -> Option<&str> {
        match self {
            Self::Expr(_, _) => None,
            Self::Assign(var, _, _) => Some(var.as_str()),
        }
    }

    pub fn eval(
        &self,
        ft: &FunctionType,
        locals: &mut AttrMap,
        ctx: &TaskContext,
        node: Option<&Node>,
    ) -> Result<ExprResult, EvalError> {
        match self {
            Self::Expr(expr, quiet) => {
                // evaluate it even if we don't return a value because
                // it could be a function that has some side effect
                let res = expr.resolve_eval(ft, ctx, Some(locals), node)?;
                if *quiet {
                    Ok(ExprResult::None)
                } else {
                    Ok(res)
                }
            }
            Self::Assign(var, expr, quiet) => {
                let val = expr.resolve_eval_value(ft, ctx, Some(locals), node)?;
                locals.insert(var.to_string().into(), val.clone());
                if *quiet {
                    Ok(ExprResult::None)
                } else {
                    Ok(ExprResult::Val(val))
                }
            }
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct UserFunction {
    pub(crate) name: Option<String>,
    args: Vec<String>,
    kwargs: Vec<(String, Expression)>,
    exprs: Vec<LocalExpr>,
}

impl std::fmt::Display for UserFunction {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let args: Vec<String> = self
            .args
            .iter()
            .map(|a| a.to_string())
            .chain(self.kwargs.iter().map(|(k, v)| format!("{k}={v}")))
            .collect();
        write!(
            f,
            "function {}({}) {{\n{}}}",
            self.name.as_deref().unwrap_or_default(),
            args.join(","),
            self.exprs
                .iter()
                .map(|e| { format!("\t{}\n", e) })
                .collect::<Vec<String>>()
                .join("")
        )
    }
}

impl UserFunction {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn name_or_unknown(&self) -> &str {
        self.name.as_deref().unwrap_or("Unknown")
    }

    pub fn new(
        name: Option<String>,
        args: Vec<String>,
        kwargs: Vec<(String, Expression)>,
        exprs: Vec<LocalExpr>,
    ) -> Self {
        Self {
            name,
            args,
            kwargs,
            exprs,
        }
    }

    pub fn eval(&self, ctx: &TaskContext, fctx: FunctionCtx) -> Result<ExprResult, EvalError> {
        self.eval_inline(&FunctionType::Env, ctx, fctx, None)
    }

    // TODO: check if running this with ft: env can make it work even if there is "node" variable, because node is Some(_)
    // TODO: Need to resolve in a way that even if function type is Env, if variable type is Node, and node is Some node
    // This is so that we don't need the loc.x part here
    // node$t2 = $t -> func (x) {loc.x + node.ORDER}

    pub fn eval_inline(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
    ) -> Result<ExprResult, EvalError> {
        let mut locals = self.resolve_locals(ctx, fctx.args, fctx.kwargs)?;
        let mut ret_expr = ExprResult::None;
        for expr in &self.exprs {
            match expr.eval(ft, &mut locals, ctx, node) {
                Ok(v) => {
                    ret_expr = v;
                }
                // early return is returned as an error so it can be
                // caught here
                Err(e) => {
                    if let EvalErrorType::InvalidReturn(val) = e.ty {
                        return Ok(val);
                    } else {
                        return Err(e);
                    }
                }
            };
        }
        Ok(ret_expr)
    }

    pub fn eval_val(&self, ctx: &TaskContext, fctx: FunctionCtx) -> Result<Attribute, EvalError> {
        self.eval(ctx, fctx)?.to_attribute().ok_or(
            EvalErrorType::NoReturnValue(self.name.as_deref().unwrap_or("Anonymous").to_string())
                .no_pos(),
        )
    }

    /// Resolve the local variables available inside the function.
    ///
    /// This resolves the positional and keyword argument values based
    /// on the function call and the function definition. The keyword
    /// arguments in the function definition is evaluated each time
    /// the function is called, so that you can put environmental
    /// variables there that can change.
    pub fn resolve_locals(
        &self,
        ctx: &TaskContext,
        args: RVec<Attribute>,
        mut kwargs: AttrMap,
    ) -> Result<AttrMap, EvalError> {
        let mut locals = AttrMap::new();
        let args_len = args.len();
        if (args_len + kwargs.len()) > (self.args.len() + self.kwargs.len()) {
            return Err(EvalErrorType::FunctionError(
                self.name_or_unknown().to_string(),
                "Too many arguments".into(),
            )
            .no_pos());
        }
        if args_len >= self.args.len() {
            // When more positional parameters are provided than in
            // function definition: we need to use those values for
            // some of the keyword arguments from the definition, and
            // take remaining keyword arguments from the function
            // call, or default
            self.args
                .iter()
                .chain(self.kwargs.iter().map(|v| &v.0))
                .zip(args)
                .for_each(|(k, v)| {
                    locals.insert(k.to_string().into(), v);
                });
            for (k, expr) in self.kwargs.iter().skip(args_len - self.args.len()) {
                match kwargs.remove(k.as_str()) {
                    RSome(v) => locals.insert(k.to_string().into(), v),
                    RNone => locals.insert(
                        k.to_string().into(),
                        expr.resolve_eval_value(&FunctionType::Env, ctx, Some(&locals), None)?,
                    ),
                };
            }
        } else {
            // When when there are not enough positional parameters in
            // the functioncall, we need to have all the remaining
            // positional parameters provided in the keyword arguments
            // of the function call
            self.args.iter().zip(args).for_each(|(k, v)| {
                locals.insert(k.to_string().into(), v);
            });
            for arg in self.args.iter().skip(args_len) {
                match kwargs.remove(arg.as_str()) {
                    RSome(v) => {
                        locals.insert(arg.to_string().into(), v);
                    }
                    RNone => {
                        return Err(EvalErrorType::FunctionError(
                            self.name_or_unknown().to_string(),
                            format!("Parameter {arg:?} not provided"),
                        )
                        .no_pos());
                    }
                }
            }
            for (k, expr) in self.kwargs.iter() {
                match kwargs.remove(k.as_str()) {
                    RSome(v) => locals.insert(k.to_string().into(), v),
                    RNone => locals.insert(
                        k.to_string().into(),
                        expr.resolve_eval_value(&FunctionType::Env, ctx, Some(&locals), None)?,
                    ),
                };
            }
        }
        // TODO also check for args len is valid
        // BUG: tried running with all kwargs in functioncall with one pos arg in def, but it triggered this
        // Write tests
        if !kwargs.is_empty() {
            return Err(EvalErrorType::FunctionError(
                self.name_or_unknown().to_string(),
                format!(
                    "Unused Arguments {:?}",
                    kwargs.keys().map(|s| s.as_str()).collect::<Vec<&str>>()
                ),
            )
            .no_pos());
        }
        Ok(locals)
    }
}
