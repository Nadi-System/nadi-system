use crate::attrs::{AttrMap, Attribute};
use crate::expressions::{EvalError, EvalErrorType, Expression};
use crate::functions::FunctionCtx;
use crate::tasks::{FunctionType, Task, TaskContext};
use abi_stable::std_types::{RNone, RSome, RVec};

#[derive(Clone, PartialEq, Debug)]
pub struct UserFunction {
    ty: Option<FunctionType>,
    name: Option<String>,
    args: Vec<String>,
    kwargs: Vec<(String, Expression)>,
    tasks: Vec<Task>,
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
            self.tasks
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<String>>()
                .join("\n"),
        )
    }
}

impl UserFunction {
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
    pub fn ty(&self) -> &Option<FunctionType> {
        &self.ty
    }
    pub fn new(
        ty: Option<FunctionType>,
        name: Option<String>,
        args: Vec<String>,
        kwargs: Vec<(String, Expression)>,
        tasks: Vec<Task>,
    ) -> Self {
        Self {
            ty,
            name,
            args,
            kwargs,
            tasks,
        }
    }

    pub fn eval_val(&self, ctx: &TaskContext, fctx: FunctionCtx) -> Result<Attribute, EvalError> {
        let locals = self.resolve_locals(ctx, fctx.args, fctx.kwargs)?;
        // For this to work, tasks should start returning values
        // for task in self.tasks.clone() {
        //     _ = ctx.execute(task)?;
        // }
        Ok(locals.into())
    }
    pub fn resolve_locals(
        &self,
        ctx: &TaskContext,
        args: RVec<Attribute>,
        mut kwargs: AttrMap,
    ) -> Result<AttrMap, EvalError> {
        let mut locals = AttrMap::new();
        let args_len = args.len();
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
                        expr.eval_value(
                            self.ty.as_ref().unwrap_or(&FunctionType::Env),
                            ctx,
                            None,
                            None,
                        )?,
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
            for arg in self.args.iter().skip(self.args.len() - args_len) {
                match kwargs.remove(arg.as_str()) {
                    RSome(v) => {
                        locals.insert(arg.to_string().into(), v);
                    }
                    RNone => {
                        return Err(EvalErrorType::FunctionError(
                            self.name.clone().unwrap_or("Unknown".into()),
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
                        expr.eval_value(
                            self.ty.as_ref().unwrap_or(&FunctionType::Env),
                            ctx,
                            None,
                            None,
                        )?,
                    ),
                };
            }
        }
        // TODO also check for args len is valid
        // BUG: tried running with all kwargs in functioncall with one pos arg in def, but it triggered this
        // Write tests
        if !kwargs.is_empty() {
            return Err(EvalErrorType::FunctionError(
                self.name.clone().unwrap_or("Unknown".into()),
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
