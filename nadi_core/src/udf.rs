use crate::attrs::{AttrMap, Attribute};
use crate::expressions::{EvalError, EvalErrorType, Expression};
use crate::functions::FunctionCtx;
use crate::node::Node;
use crate::tasks::{FunctionType, TaskContext};
use abi_stable::std_types::{RNone, RSome, RVec};

#[derive(Clone, PartialEq, Debug)]
pub struct UserFunction {
    name: Option<String>,
    args: Vec<String>,
    kwargs: Vec<(String, Expression)>,
    expr: Expression,
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
            "function {}({}) {{\n\t{}\n}}",
            self.name.as_deref().unwrap_or_default(),
            args.join(","),
            self.expr
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
        expr: Expression,
    ) -> Self {
        Self {
            name,
            args,
            kwargs,
            expr,
        }
    }

    pub fn eval_val(
        &self,
        ft: &FunctionType,
        ctx: &TaskContext,
        fctx: FunctionCtx,
        node: Option<&Node>,
    ) -> Result<Attribute, EvalError> {
        let locals = self.resolve_locals(ctx, fctx.args, fctx.kwargs)?;
        match self.expr.resolve_eval(ft, ctx, Some(&locals), node) {
            Ok(Some(val)) => Ok(val),
            Ok(None) => Err(EvalErrorType::NoReturnValue(
                self.name.as_deref().unwrap_or("Annonymus").to_string(),
            )
            .no_pos()),
            Err(e) => Err(e),
        }
    }
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
                .zip(args.into_iter())
                .for_each(|(k, v)| {
                    locals.insert(k.to_string().into(), v);
                });
            for (k, expr) in self.kwargs.iter().skip(args_len - self.args.len()) {
                match kwargs.remove(k.as_str()) {
                    RSome(v) => locals.insert(k.to_string().into(), v),
                    RNone => locals.insert(
                        k.to_string().into(),
                        expr.eval_value(&FunctionType::Env, ctx, None, None)?,
                    ),
                };
            }
        } else {
            // When when there are not enough positional parameters in
            // the functioncall, we need to have all the remaining
            // positional parameters provided in the keyword arguments
            // of the function call
            self.args.iter().zip(args.into_iter()).for_each(|(k, v)| {
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
                        expr.eval_value(&FunctionType::Env, ctx, None, None)?,
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
