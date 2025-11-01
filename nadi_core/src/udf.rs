use crate::attrs::{AttrMap, Attribute};
use crate::expressions::Expression;
use crate::tasks::{FunctionType, Task, TaskContext};

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

    pub fn eval_val(&self, ctx: &TaskContext, args: Vec<Attribute>, kwargs: AttrMap) -> Attribute {
        todo!()
    }
}
