use crate::expressions::{EvalError, Expression};
use crate::functions::{FuncArg, FuncArgType, NadiFunctions};
use crate::network::PropCondition;
use crate::prelude::*;

#[derive(Default, Clone)]
pub struct TaskContext {
    pub network: Network,
    pub functions: NadiFunctions,
    pub env: AttrMap,
}

impl TaskContext {
    pub fn new(net: Option<Network>) -> Self {
        Self {
            network: net.unwrap_or_default(),
            functions: NadiFunctions::new(),
            env: AttrMap::new(),
        }
    }

    pub fn execute(&mut self, task: Task) -> Result<Option<String>, String> {
        match task {
            Task::Eval(et) => self.eval_task(et),
            Task::Attr(at) => self.attr_task(at).map(Some),
            Task::Conditional(ct) => {
                let mut outputs = vec![];
                let cond = ct.cond.resolve(&FunctionType::Env, self, None)?;
                let res = cond.eval_value(&FunctionType::Env, self, None)?;
                match bool::try_from_attr(&res).map_err(EvalError::AttributeError)? {
                    true => {
                        for task in ct.iftrue {
                            outputs.push(self.execute(task)?)
                        }
                    }
                    false => {
                        for task in ct.iffalse {
                            outputs.push(self.execute(task)?)
                        }
                    }
                }
                let outputs: Vec<String> = outputs.into_iter().flatten().collect();
                if outputs.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(outputs.join("\n")))
                }
            }
            Task::WhileLoop(lt) => {
                let max_iter = 1_000_000; // todo make it a constant (maybe configurable)
                let mut outputs = vec![];
                for _ in 0..max_iter {
                    let cond = lt.cond.resolve(&FunctionType::Env, self, None)?;
                    let res = cond.eval_value(&FunctionType::Env, self, None)?;
                    match bool::try_from_attr(&res).map_err(EvalError::AttributeError)? {
                        true => {
                            for task in &lt.tasks {
                                // TODO really need to work on a way
                                // to get the output as it runs in
                                // async manner
                                outputs.push(self.execute(task.clone())?)
                            }
                        }
                        false => break,
                    }
                }
                let outputs: Vec<String> = outputs.into_iter().flatten().collect();
                if outputs.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(outputs.join("\n")))
                }
            }
            Task::Help(kw, var) => self.help(kw, var),
            Task::Exit => std::process::exit(0),
        }
    }

    pub fn eval_task(&mut self, task: EvalTask) -> Result<Option<String>, String> {
        match task.ty {
            FunctionType::Env => match task.input.resolve_eval(&FunctionType::Env, self, None)? {
                Some(a) => {
                    if let Some(attr) = &task.attr {
                        if let Some(old) =
                            self.env.set_attr_nested(&task.attr_pre, attr, a.clone())?
                        {
                            if task.silent {
                                Ok(None)
                            } else {
                                Ok(Some(format!("{} -> {}", old.to_string(), a.to_string())))
                            }
                        } else {
                            Ok(None)
                        }
                    } else if task.silent {
                        Ok(None)
                    } else {
                        Ok(Some(a.to_string()))
                    }
                }
                None => Ok(None),
            },
            FunctionType::Node => {
                let nodes = self
                    .propagation(task.propagation.unwrap_or_default())
                    .map_err(|e| e.message())?;
                let mut attrs = Vec::with_capacity(nodes.len());
                for n in nodes {
                    let res = match task
                        .input
                        // add node name to this error
                        .resolve_eval_mut(&FunctionType::Node, self, Some(&n))?
                    {
                        Some(r) => r,
                        None => continue,
                    };
                    let mut n = n
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))?;
                    if let Some(attr) = &task.attr {
                        let old = n.set_attr_nested(&task.attr_pre, attr, res.clone())?;
                        if !task.silent {
                            if let Some(o) = old {
                                attrs.push(format!(
                                    "  {} = {} -> {}",
                                    n.name(),
                                    o.to_string(),
                                    res.to_string()
                                ));
                            }
                        }
                    } else if !task.silent {
                        attrs.push(format!("  {} = {}", n.name(), res.to_string()));
                    }
                }
                if task.silent || attrs.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(format!("{{\n{}\n}}", attrs.join(",\n"))))
                }
            }
            FunctionType::Network => {
                match task
                    .input
                    .resolve_eval_mut(&FunctionType::Network, self, None)
                {
                    Ok(Some(a)) => {
                        if let Some(attr) = &task.attr {
                            if let Some(old) =
                                self.network
                                    .set_attr_nested(&task.attr_pre, attr, a.clone())?
                            {
                                if task.silent {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("{} -> {}", old.to_string(), a.to_string())))
                                }
                            } else {
                                Ok(None)
                            }
                        } else if task.silent {
                            Ok(None)
                        } else {
                            Ok(Some(a.to_string()))
                        }
                    }
                    Ok(None) => Ok(None),
                    Err(e) => Err(e.message()),
                }
            }
        }
    }

    pub fn attr_task(&self, task: AttrTask) -> Result<String, String> {
        match task.ty {
            FunctionType::Env => self
                .env
                .attr_nested(&task.attr_pre, &task.attr)?
                .map(|a| a.to_string())
                .ok_or(EvalError::AttributeNotFound)
                .map_err(|e| e.to_string()),
            FunctionType::Node => {
                let nodes = self.propagation(task.propagation.unwrap_or_default())?;
                let attrs = nodes
                    .iter()
                    .map(|n| {
                        let n = n.lock();
                        Ok(format!(
                            "  {} = {}",
                            n.name(),
                            if let Some(a) = n
                                .attr_nested(&task.attr_pre, &task.attr)
                                .map_err(|e| format!("Node {}: {e}", n.name()))?
                            {
                                a.to_string()
                            } else {
                                "<None>".to_string()
                            }
                        ))
                    })
                    .collect::<Result<Vec<String>, String>>()?;
                Ok(format!("{{\n{}\n}}", attrs.join(",\n")))
            }
            FunctionType::Network => self
                .network
                .attr_nested(&task.attr_pre, &task.attr)?
                .map(|a| a.to_string())
                .ok_or(EvalError::AttributeNotFound)
                .map_err(|e| e.to_string()),
        }
    }
    pub fn help(
        &self,
        kw: Option<TaskKeyword>,
        var: Option<String>,
    ) -> Result<Option<String>, String> {
        match (kw, var) {
            (None, Some(var)) => {
                let mut helpstr = String::new();
                if let Some(f) = self.functions.node(&var) {
                    helpstr = format_help("node", &var, &f.signature(), &f.args(), &f.help());
                }
                if let Some(f) = self.functions.network(&var) {
                    helpstr.push_str(&format_help(
                        "network",
                        &var,
                        &f.signature(),
                        &f.args(),
                        &f.help(),
                    ));
                }
                if !helpstr.is_empty() {
                    Ok(Some(helpstr))
                } else {
                    Err(format!("Function {} not found", var))
                }
            }
            (Some(TaskKeyword::Node), Some(var)) => {
                if let Some(f) = self.functions.node(&var) {
                    Ok(Some(format_help(
                        "node",
                        &var,
                        &f.signature(),
                        &f.args(),
                        &f.help(),
                    )))
                } else {
                    Err(format!("Node Function {} not found", var))
                }
            }
            (Some(TaskKeyword::Network), Some(var)) => {
                if let Some(f) = self.functions.network(&var) {
                    Ok(Some(format_help(
                        "network",
                        &var,
                        &f.signature(),
                        &f.args(),
                        &f.help(),
                    )))
                } else {
                    Err(format!("Network Function {} not found", var))
                }
            }
            (Some(kw), None) => Ok(Some(kw.help())),
            (Some(kw), Some(x)) => Err(format!(
                "Keyword {} does not have help for {}",
                kw.to_string(),
                x
            )),
            (None, None) => Ok(Some("Usage: help <keyword> [function]".into())),
        }
    }

    pub fn propagation(&self, prop: Propagation) -> Result<Vec<Node>, EvalError> {
        let nodes = self.network.nodes_select(&prop.order, &prop.nodes)?;
        match prop.condition {
            PropCondition::All => Ok(nodes),
            PropCondition::Expr(expr) => {
                let mut sel_nodes = Vec::with_capacity(self.network.nodes().count());
                // simplify to save computation (not tested/benchmarked)
                let expr = expr.simplify(&FunctionType::Node, self)?;
                // expression is evaluated for each node
                for n in nodes {
                    let cond = expr.resolve(&FunctionType::Node, self, Some(&n))?;
                    let res = cond.eval_value(&FunctionType::Node, self, Some(&n))?;
                    match bool::try_from_attr(&res) {
                        Ok(true) => sel_nodes.push(n),
                        Ok(false) => (),
                        Err(e) => {
                            return Err(EvalError::NodeAttributeError(
                                n.lock().name().to_string(),
                                e,
                            ))
                        }
                    }
                }
                Ok(sel_nodes)
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionType {
    Env,
    Node,
    Network,
}

impl std::fmt::Display for FunctionType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

impl FunctionType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::Node => "node",
            Self::Network => "network",
            Self::Env => "env",
        }
    }

    pub fn from_keyword(kw: &TaskKeyword) -> Option<Self> {
        match kw {
            TaskKeyword::Node => Some(FunctionType::Node),
            TaskKeyword::Network => Some(FunctionType::Network),
            TaskKeyword::Env => Some(FunctionType::Env),
            _ => None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct EvalTask {
    pub ty: FunctionType,
    pub propagation: Option<Propagation>,
    pub attr_pre: Vec<String>,
    pub attr: Option<String>,
    pub input: Expression,
    pub silent: bool,
}

impl std::fmt::Display for EvalTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let outattr = if let Some(attr) = &self.attr {
            format!(
                ".{} =",
                self.attr_pre
                    .iter()
                    .map(|s| s.as_str())
                    .chain([attr.as_str()])
                    .collect::<Vec<&str>>()
                    .join(".")
            )
        } else {
            "".to_string()
        };
        write!(
            f,
            "{}{}{} {}{}",
            self.ty,
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            outattr,
            self.input.to_string(),
            self.silent.then_some(";").unwrap_or_default()
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AttrTask {
    pub ty: FunctionType,
    pub propagation: Option<Propagation>,
    pub attr_pre: Vec<String>,
    pub attr: String,
}

impl std::fmt::Display for AttrTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let outattr = format!(
            ".{} =",
            self.attr_pre
                .iter()
                .map(|s| s.as_str())
                .chain([self.attr.as_str()])
                .collect::<Vec<&str>>()
                .join(".")
        );
        write!(
            f,
            "{}{}{}",
            self.ty,
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            outattr
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct CondTask {
    pub cond: Expression,
    pub iftrue: Vec<Task>,
    pub iffalse: Vec<Task>,
}

impl std::fmt::Display for CondTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let tasks = self
            .iftrue
            .iter()
            .map(|p| p.to_string())
            .collect::<Vec<String>>()
            .join("\n");
        if self.iffalse.is_empty() {
            write!(f, "if ({}) {{\n\t{}\n}}", self.cond.to_string(), tasks,)
        } else {
            write!(
                f,
                "if ({}) {{\n\t{}\n}} else {{\n\t{}\n}}",
                self.cond.to_string(),
                tasks,
                self.iffalse
                    .iter()
                    .map(|p| p.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            )
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct WhileTask {
    pub cond: Expression,
    pub tasks: Vec<Task>,
}

impl std::fmt::Display for WhileTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "while ({}) {{\n\t{}\n}}",
            self.cond.to_string(),
            self.tasks
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<String>>()
                .join("\n"),
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Task {
    Eval(EvalTask),
    Attr(AttrTask),
    Conditional(CondTask),
    WhileLoop(WhileTask),
    Help(Option<TaskKeyword>, Option<String>),
    Exit,
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Eval(et) => std::fmt::Display::fmt(et, f),
            Self::Attr(at) => std::fmt::Display::fmt(at, f),
            Self::Conditional(t) => std::fmt::Display::fmt(t, f),
            Self::WhileLoop(t) => std::fmt::Display::fmt(t, f),
            Self::Help(None, None) => write!(f, "help"),
            Self::Help(Some(kw), None) => write!(f, "help {kw}"),
            Self::Help(None, Some(s)) => write!(f, "help {s}"),
            Self::Help(Some(kw), Some(s)) => write!(f, "help {kw} {s}"),
            Self::Exit => write!(f, "exit"),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskKeyword {
    Node,
    Network,
    Env,
    Exit,
    End,
    Help,
    Inputs,
    Output,
    Nodes,
    If,
    Else,
    While,
    In,
    Match,
    // reserved
    Function,
    Map,
    Attrs,
    Loop,
    For,
}

impl std::str::FromStr for TaskKeyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "node" => TaskKeyword::Node,
            "network" | "net" => TaskKeyword::Network,
            "env" => TaskKeyword::Env,
            "exit" => TaskKeyword::Exit,
            "end" => TaskKeyword::End,
            "help" => TaskKeyword::Help,
            "inputs" => TaskKeyword::Inputs,
            "output" => TaskKeyword::Output,
            "nodes" => TaskKeyword::Nodes,
            "if" => TaskKeyword::If,
            "else" => TaskKeyword::Else,
            "while" => TaskKeyword::While,
            "in" => TaskKeyword::In,
            "match" => TaskKeyword::Match,
            "function" | "func" => TaskKeyword::Function,
            "map" => TaskKeyword::Map,
            "attrs" => TaskKeyword::Attrs,
            "loop" => TaskKeyword::Loop,
            "for" => TaskKeyword::For,
            k => return Err(format!("{k} is not a keyword")),
        })
    }
}

impl std::fmt::Display for TaskKeyword {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TaskKeyword::Node => "node",
                TaskKeyword::Network => "network",
                TaskKeyword::Env => "env",
                TaskKeyword::Exit => "exit",
                TaskKeyword::End => "end",
                TaskKeyword::Help => "help",
                TaskKeyword::Inputs => "inputs",
                TaskKeyword::Output => "output",
                TaskKeyword::Nodes => "nodes",
                TaskKeyword::If => "if",
                TaskKeyword::Else => "else",
                TaskKeyword::While => "while",
                TaskKeyword::In => "in",
                TaskKeyword::Match => "match",
                TaskKeyword::Function => "function",
                TaskKeyword::Map => "map",
                TaskKeyword::Attrs => "attrs",
                TaskKeyword::Loop => "loop",
                TaskKeyword::For => "for",
            }
        )
    }
}

impl TaskKeyword {
    pub fn help(&self) -> String {
        match self {
            TaskKeyword::Node => "node function",
            TaskKeyword::Network => "network function",
            TaskKeyword::Env => "environmental variables",
            TaskKeyword::Exit => "exit",
            TaskKeyword::End => "End the tasks file here (discard everything else)",
            TaskKeyword::Help => "help",
            TaskKeyword::Inputs => "inputs of the current node",
            TaskKeyword::Output => "output of the current node",
            TaskKeyword::Nodes => "all the nodes in the network",
            TaskKeyword::If => "if part of if-else block",
            TaskKeyword::Else => "else part of if-else block",
            TaskKeyword::While => "while loop",
            TaskKeyword::In => "Check if value is in an array/table",
            TaskKeyword::Match => "match regex pattern with strings",
            TaskKeyword::Function => "function definition",
            TaskKeyword::Map => "map array to a function",
            TaskKeyword::Attrs => "attrs of a node or network",
            TaskKeyword::Loop => "a generic loop",
            TaskKeyword::For => "for loop",
        }
        .to_string()
    }
}

fn format_help(prefix: &str, name: &str, signature: &str, args: &[FuncArg], help: &str) -> String {
    let mut help = help.trim().split('\n');
    let short_help = help.next().unwrap_or("No Help");
    let desc = help.collect::<Vec<&str>>().join("\n");
    let mut argshelp = "# Arguments\n".to_string();
    for arg in args {
        let desc = match &arg.category {
            FuncArgType::Arg => format!("- `{}: {}` {}", arg.name, arg.ty, arg.help),
            FuncArgType::OptArg => format!("- `{}: {}` [optional] {}", arg.name, arg.ty, arg.help),
            FuncArgType::DefArg(v) => {
                format!("- `{}: {}` [def = {}] {}", arg.name, arg.ty, v, arg.help)
            }
            FuncArgType::Args => format!("- `*{}` {}", arg.name, arg.help),
            FuncArgType::KwArgs => format!("- `**{}` {}", arg.name, arg.help),
        };
        argshelp.push_str(&desc);
        argshelp.push('\n');
    }
    format!(
        "{} {} ({})\n{}\n{}\n{}",
        prefix, name, signature, short_help, argshelp, desc
    )
}
