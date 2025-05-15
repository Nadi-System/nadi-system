use crate::expressions::{EvalError, Expression, InputVar};
use crate::functions::{FuncArg, FuncArgType, FunctionRet, NadiFunctions};
use crate::prelude::*;
use colored::Colorize;

pub struct TaskContext {
    pub network: Network,
    pub functions: NadiFunctions,
    pub env: AttrMap,
}

impl TaskContext {
    pub fn new(net: Option<Network>) -> Self {
        Self {
            network: net.unwrap_or(Network::default()),
            functions: NadiFunctions::new(),
            env: AttrMap::new(),
        }
    }

    pub fn execute(&mut self, task: Task) -> Result<Option<String>, String> {
        match task {
            Task::Eval(et) => self.eval_task(et),
            Task::Attr(at) => self.attr_task(at).map(|a| Some(a)),
            Task::Help(kw, var) => self.help(kw, var),
            Task::Exit => std::process::exit(0),
        }
    }

    pub fn eval_task(&mut self, task: EvalTask) -> Result<Option<String>, String> {
        match task.ty {
            FunctionType::Env => match task.input.resolve_eval(&FunctionType::Env, &self, None) {
                Ok(Some(a)) => {
                    if task.attribute.is_empty() {
                        if task.silent {
                            Ok(None)
                        } else {
                            Ok(Some(a.to_string()))
                        }
                    } else {
                        if let Some(old) = self.env.set_attr_nested(&task.attribute, a.clone())? {
                            if task.silent {
                                Ok(None)
                            } else {
                                Ok(Some(format!("{} -> {}", old.to_string(), a.to_string())))
                            }
                        } else {
                            Ok(None)
                        }
                    }
                }
                Ok(None) => Ok(None),
                Err(e) => Err(e.message()),
            },
            FunctionType::Node => {
                let nodes = self
                    .propagation(task.propagation.unwrap_or_default())
                    .map_err(|e| e.message())?;
                let mut attrs = Vec::with_capacity(nodes.len());
                for n in nodes {
                    let res = match task
                        .input
                        .resolve_eval_mut(&FunctionType::Network, self, Some(&n))
                        // add node name to this error
                        .map_err(|e| e.message())?
                    {
                        Some(r) => r,
                        None => continue,
                    };
                    // TODO add this to all other lock() so we get
                    // error instead of program freezing
                    let mut n = n
                        .try_lock()
                        .into_option()
                        .ok_or(EvalError::MutexError(file!(), line!()))
                        .map_err(|e| e.message())?;
                    if task.attribute.is_empty() {
                        if !task.silent {
                            attrs.push(format!("  {} = {}", n.name(), res.to_string()));
                        }
                    } else {
                        let old = n.set_attr_nested(&task.attribute, res.clone())?;
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
                    }
                }
                if task.silent {
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
                        if task.attribute.is_empty() {
                            if task.silent {
                                Ok(None)
                            } else {
                                Ok(Some(a.to_string()))
                            }
                        } else {
                            if let Some(old) =
                                self.network.set_attr_nested(&task.attribute, a.clone())?
                            {
                                if task.silent {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!("{} -> {}", old.to_string(), a.to_string())))
                                }
                            } else {
                                Ok(None)
                            }
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
                .attr_nested(&task.attribute)?
                .map(|a| a.to_string())
                .ok_or(EvalError::AttributeNotFound)
                .map_err(|e| e.to_string()),
            FunctionType::Node => {
                let nodes = self
                    .propagation(task.propagation.unwrap_or_default())
                    .map_err(|e| e.message())?;
                let attrs = nodes
                    .iter()
                    .map(|n| {
                        let n = n.lock();
                        Ok(format!(
                            "  {} = {}",
                            n.name(),
                            if let Some(a) = n
                                .attr_nested(&task.attribute)
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
                .attr_nested(&task.attribute)?
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
        match prop {
            Propagation::Sequential | Propagation::OutputFirst => {
                Ok(self.network.nodes().cloned().collect())
            }
            Propagation::Inverse | Propagation::InputsFirst => {
                Ok(self.network.nodes_rev().cloned().collect())
            }
            Propagation::Conditional(expr) => {
                let mut nodes = Vec::with_capacity(self.network.nodes().count());
                // simplify to save computation
                let expr = expr.simplify(&FunctionType::Node, &self)?;
                // propagation is evaluated for each node even if it's
                // in network function
                for n in self.network.nodes() {
                    let cond = expr.resolve(&FunctionType::Node, &self, Some(n))?;
                    let res = cond.eval_value(&FunctionType::Node, &self, Some(n))?;
                    match bool::try_from_attr(&res) {
                        Ok(true) => nodes.push(n.clone()),
                        Ok(false) => (),
                        Err(e) => {
                            return Err(EvalError::NodeAttributeError(
                                n.lock().name().to_string(),
                                e,
                            ))
                        }
                    }
                }
                Ok(nodes)
            }
            Propagation::List(lst) => lst
                .iter()
                .map(|n| {
                    self.network
                        .nodes_map
                        .get(n)
                        .cloned()
                        .ok_or_else(|| EvalError::NodeNotFound(n.to_string()))
                })
                .collect(),
            Propagation::Path(p) => self.network.nodes_path(&p),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum FunctionType {
    Env,
    Node,
    Network,
}

impl ToString for FunctionType {
    fn to_string(&self) -> String {
        match self {
            Self::Env => "env",
            Self::Node => "node",
            Self::Network => "network",
        }
        .to_string()
    }
}

impl FunctionType {
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
    pub attribute: Vec<String>,
    pub input: Expression,
    pub silent: bool,
}

impl ToString for EvalTask {
    fn to_string(&self) -> String {
        let outattr = if self.attribute.is_empty() {
            "".to_string()
        } else {
            format!(".{} =", self.attribute.join("."))
        };
        format!(
            "{}{}{} {}{}",
            self.ty.to_string(),
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            outattr,
            self.input.to_string(),
            self.silent.then(|| ";").unwrap_or_default()
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct AttrTask {
    pub ty: FunctionType,
    pub propagation: Option<Propagation>,
    pub attribute: Vec<String>,
}

impl ToString for AttrTask {
    fn to_string(&self) -> String {
        let outattr = if self.attribute.is_empty() {
            "".to_string()
        } else {
            format!(".{}", self.attribute.join("."))
        };
        format!(
            "{}{}{}",
            self.ty.to_string(),
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            outattr
        )
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum Task {
    Eval(EvalTask),
    Attr(AttrTask),
    Help(Option<TaskKeyword>, Option<String>),
    Exit,
}

impl ToString for Task {
    fn to_string(&self) -> String {
        match self {
            Self::Eval(et) => et.to_string(),
            Self::Attr(at) => at.to_string(),
            Self::Help(None, None) => "help".to_string(),
            Self::Help(Some(kw), None) => format!("help {}", kw.to_string()),
            Self::Help(None, Some(s)) => format!("help {s}"),
            Self::Help(Some(kw), Some(s)) => format!("help {} {s}", kw.to_string()),
            Self::Exit => "exit".to_string(),
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
    In,
    Match,
    Inputs,
    Output,
    Nodes,
}

impl std::str::FromStr for TaskKeyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "node" => TaskKeyword::Node,
            "network" => TaskKeyword::Network,
            "net" => TaskKeyword::Network,
            "env" => TaskKeyword::Env,
            "exit" => TaskKeyword::Exit,
            "end" => TaskKeyword::End,
            "help" => TaskKeyword::Help,
            "in" => TaskKeyword::In,
            "match" => TaskKeyword::Match,
            "inputs" => TaskKeyword::Inputs,
            "output" => TaskKeyword::Output,
            "nodes" => TaskKeyword::Nodes,
            k => return Err(format!("{k} is not a keyword")),
        })
    }
}

impl ToString for TaskKeyword {
    fn to_string(&self) -> String {
        match self {
            TaskKeyword::Node => "node",
            TaskKeyword::Network => "network",
            TaskKeyword::Env => "env",
            TaskKeyword::Exit => "exit",
            TaskKeyword::End => "end",
            TaskKeyword::Help => "help",
            TaskKeyword::In => "in",
            TaskKeyword::Match => "match",
            TaskKeyword::Inputs => "inputs",
            TaskKeyword::Output => "output",
            TaskKeyword::Nodes => "nodes",
        }
        .to_string()
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
            TaskKeyword::In => "Check if value is in an array/table",
            TaskKeyword::Match => "match regex pattern with strings",
            TaskKeyword::Inputs => "inputs of the current node",
            TaskKeyword::Output => "output of the current node",
            TaskKeyword::Nodes => "all the nodes in the network",
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
