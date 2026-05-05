use crate::attrs::{AttrMap, Attribute};
use crate::eval::{Eval, EvalCtx, EvalError, EvalErrorType};
use crate::expressions::{ExprResult, RawExpr};
use crate::functions::FunctionCtx;
use crate::tasks::TaskContext;
use abi_stable::std_types::{RNone, RSome, RVec};

#[derive(Clone, PartialEq, Debug)]
pub struct UserFunction {
    pub(crate) name: Option<String>,
    args: Vec<String>,
    kwargs: Vec<(String, RawExpr)>,
    exprs: Box<RawExpr>,
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
        kwargs: Vec<(String, RawExpr)>,
        exprs: RawExpr,
    ) -> Self {
        Self {
            name,
            args,
            kwargs,
            exprs: Box::new(exprs),
        }
    }

    // TODO: check if running this with ft: env can make it work even if there is "node" variable, because node is Some(_)
    // TODO: Need to resolve in a way that even if function type is Env, if variable type is Node, and node is Some node
    // This is so that we don't need the loc.x part here
    // node$t2 = $t -> func (x) {loc.x + node.ORDER}

    pub fn eval(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        fctx: FunctionCtx,
    ) -> Result<ExprResult, EvalError> {
        let mut locals = self.resolve_locals(ctx, ectx, fctx.args, fctx.kwargs)?;
        match self
            .exprs
            .clone()
            .resolve(ctx, ectx.clone())?
            .eval(ctx, &ectx, &mut locals)
        {
            Ok(v) => Ok(v),
            // early return is returned as an error so it can be
            // caught here
            Err(e) => {
                if let EvalErrorType::InvalidReturn(val) = e.ty {
                    Ok(val)
                } else {
                    Err(e)
                }
            }
        }
    }

    pub fn eval_mut(
        &self,
        ctx: &mut TaskContext,
        ectx: &EvalCtx,
        fctx: FunctionCtx,
    ) -> Result<ExprResult, EvalError> {
        let mut locals = self.resolve_locals(ctx, ectx, fctx.args, fctx.kwargs)?;
        let res = self.exprs.clone().resolve(ctx, ectx.clone())?;
        match res.eval_mut(ctx, &ectx, &mut locals) {
            Ok(v) => return Ok(v),
            // early return is returned as an error so it can be
            // caught here
            Err(e) => {
                if let EvalErrorType::InvalidReturn(val) = e.ty {
                    return Ok(val);
                } else {
                    return Err(e);
                }
            }
        }
    }

    pub fn eval_val(
        &self,
        ctx: &TaskContext,
        ectx: &EvalCtx,
        fctx: FunctionCtx,
    ) -> Result<Attribute, EvalError> {
        self.eval(ctx, ectx, fctx)?.to_attribute().ok_or(
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
        ectx: &EvalCtx,
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
                    RNone => {
                        let v = expr.clone().resolve(ctx, ectx.clone())?.eval_value(
                            ctx,
                            ectx,
                            &mut locals,
                        )?;
                        locals.insert(k.to_string().into(), v)
                    }
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
                    RNone => {
                        let val = expr.clone().resolve(ctx, ectx.clone())?.eval_value(
                            ctx,
                            ectx,
                            &mut locals,
                        )?;
                        locals.insert(k.to_string().into(), val)
                    }
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
