use crate::functions::{
    FuncArg, FuncArgType, FunctionCtx, FunctionRet, NadiFunctions, Propagation,
};
use crate::prelude::*;
use abi_stable::std_types::{RString, Tuple2};
use colored::Colorize;
use std::collections::HashMap;

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
        match &task.ty {
            TaskType::Exit => std::process::exit(0),
            TaskType::Env => {
                if let Some(var) = task.attribute {
                    match task.input {
                        TaskInput::Literal(val) => {
                            self.env.insert(var.into(), val);
                            Ok(None)
                        }
                        TaskInput::None => {
                            if let Some(v) = self.env.get(var.as_str()) {
                                Ok(Some(v.to_colored_string()))
                            } else {
                                Err(format!("Env variable {var} doesn't exist"))
                            }
                        }
                        _ => Err(String::from("Couldn't set env variable")),
                    }
                } else {
                    let mut list = String::new();
                    for Tuple2(k, v) in &self.env {
                        list.push_str(&format!("{k}={}", v.to_colored_string()));
                    }
                    Ok(Some(list))
                }
            }
            TaskType::Node(p) => {
                let nodes: Vec<Node> = self.network.nodes_propagation(p)?;
                match task.input {
                    TaskInput::None => {
                        if let Some(attr) = task.attribute {
                            // this or filter_map ?
                            let attrs = nodes
                                .iter()
                                .map(|n| {
                                    let n = n.lock();
                                    format!(
                                        "  {} = {}",
                                        n.name(),
                                        if let Some(a) = n.attr(&attr) {
                                            a.to_colored_string()
                                        } else {
                                            "<None>".truecolor(100, 100, 100).to_string()
                                        }
                                    )
                                })
                                .collect::<Vec<String>>();
                            Ok(Some(format!("{attr} = {{\n{}\n}}", attrs.join(",\n"))))
                        } else {
                            Ok(None) // it's just keyword with nothing to do
                        }
                    }
                    TaskInput::Literal(v) => {
                        if let Some(attr) = task.attribute {
                            nodes.iter().for_each(|n| {
                                n.lock().set_attr(&attr, v.clone());
                            });
                            Ok(None)
                        } else {
                            Err("Invalid operation, no attribute to assign".to_string())
                        }
                    }
                    TaskInput::Variable(v) => {
                        if let Some(attr) = task.attribute {
                            nodes.iter().try_for_each(|n| {
                                let mut n = n.lock();
                                let a = n.attr(&v).cloned();
                                match a {
                                    Some(v) => {
                                        n.set_attr(&attr, v);
                                        Ok(())
                                    }
                                    None => {
                                        Err(format!("Node {}: Attribute {} not found", n.name(), v))
                                    }
                                }
                            })?;
                            Ok(None)
                        } else {
                            Err("Invalid operation, no attribute to assign".to_string())
                        }
                    }
                    TaskInput::Function(fc) => match self.functions.node(&fc.name) {
                        Some(f) => {
                            let attrs = nodes
                                .iter()
                                .map(|n| {
                                    let mut node = n.lock();
                                    let ctx = fc
                                        .node_ctx(&node)
                                        .map_err(|e| format!("{}: {e}", node.name()))?;
                                    match f.call(&mut node, &ctx) {
                                        FunctionRet::None => Ok(None),
                                        FunctionRet::Some(a) => {
                                            if let Some(attr) = &task.attribute {
                                                node.set_attr(&attr, a);
                                                Ok(None)
                                            } else {
                                                Ok(Some(format!(
                                                    "  {} = {}",
                                                    node.name(),
                                                    a.to_colored_string()
                                                )))
                                            }
                                        }
                                        FunctionRet::Error(e) => {
                                            Err(format!("{}: {e}", node.name()))
                                        }
                                    }
                                })
                                .collect::<Result<Vec<Option<String>>, String>>()?;
                            let attrs =
                                attrs.into_iter().filter_map(|v| v).collect::<Vec<String>>();
                            if attrs.is_empty() {
                                Ok(None)
                            } else {
                                Ok(Some(format!("{{\n{}\n}}", attrs.join(",\n"))))
                            }
                        }
                        None => Err(format!("Node Function {} not found", fc.name)),
                    },
                }
            }
            TaskType::Network => match task.input {
                TaskInput::None => {
                    if let Some(attr) = task.attribute {
                        if let Some(a) = self.network.attr(&attr) {
                            Ok(Some(a.to_colored_string()))
                        } else {
                            Err(format!("Attribute not found {}", attr))
                        }
                    } else {
                        Ok(None) // same thing, nothing to do
                    }
                }
                TaskInput::Literal(a) => {
                    if let Some(attr) = task.attribute {
                        self.network.set_attr(&attr, a.clone());
                    }
                    Ok(None)
                }
                TaskInput::Variable(var) => {
                    if let Some(attr) = task.attribute {
                        if let Some(v) = self.network.attr(&var) {
                            self.network.set_attr(&attr, v.clone());
                            Ok(None)
                        } else {
                            Err(format!("Attribute not found {}", attr))
                        }
                    } else {
                        Err(format!("Nothing to do, found variable {}", var))
                    }
                }
                TaskInput::Function(fc) => match self.functions.network(&fc.name) {
                    Some(f) => {
                        let ctx = fc.network_ctx(&self.network)?;
                        match f.call(&mut self.network, &ctx) {
                            FunctionRet::None => Ok(None),
                            FunctionRet::Some(a) => {
                                if let Some(attr) = task.attribute {
                                    self.network.set_attr(&attr, a);
                                    Ok(None)
                                } else {
                                    Ok(Some(a.to_colored_string()))
                                }
                            }
                            FunctionRet::Error(e) => Err(e.to_string()),
                        }
                    }
                    None => Err(format!("Network Function {} not found", fc.name)),
                },
            },
            TaskType::Help(None, Some(var)) => {
                let mut helpstr = String::new();
                if let Some(f) = self.functions.node(&var) {
                    helpstr = format_help("node", var, &f.signature(), &f.args(), &f.help());
                }
                if let Some(f) = self.functions.network(&var) {
                    helpstr.push_str(&format_help(
                        "network",
                        var,
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
            TaskType::Help(Some(TaskKeyword::Node), Some(var)) => {
                if let Some(f) = self.functions.node(&var) {
                    Ok(Some(format_help(
                        "node",
                        var,
                        &f.signature(),
                        &f.args(),
                        &f.help(),
                    )))
                } else {
                    Err(format!("Node Function {} not found", var))
                }
            }
            TaskType::Help(Some(TaskKeyword::Network), Some(var)) => {
                if let Some(f) = self.functions.network(&var) {
                    Ok(Some(format_help(
                        "network",
                        var,
                        &f.signature(),
                        &f.args(),
                        &f.help(),
                    )))
                } else {
                    Err(format!("Network Function {} not found", var))
                }
            }
            TaskType::Help(Some(TaskKeyword::Env), None) => {
                Ok(Some(format!("Set Environmental Variable")))
            }
            _ => todo!(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Task {
    pub ty: TaskType,
    pub attribute: Option<String>,
    pub input: TaskInput,
}

impl Task {
    pub fn to_colored_string(&self) -> String {
        if let Some(ref a) = self.attribute {
            if self.input == TaskInput::None {
                format!("{}.{}", self.ty.to_colored_string(), a.green())
            } else {
                format!(
                    "{}.{} = {}",
                    self.ty.to_colored_string(),
                    a.green(),
                    self.input.to_colored_string()
                )
            }
        } else {
            format!(
                "{} {}",
                self.ty.to_colored_string(),
                self.input.to_colored_string()
            )
        }
    }

    pub fn exit() -> Self {
        Task {
            ty: TaskType::Exit,
            attribute: None,
            input: TaskInput::None,
        }
    }

    pub fn env(var: String, val: Attribute) -> Self {
        Task {
            ty: TaskType::Env,
            attribute: Some(var),
            input: TaskInput::Literal(val),
        }
    }

    pub fn help(kw: Option<TaskKeyword>, var: Option<String>) -> Self {
        Task {
            ty: TaskType::Help(kw, var),
            attribute: None,
            input: TaskInput::None,
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskType {
    Node(Propagation),
    Network,
    Env,
    Help(Option<TaskKeyword>, Option<String>),
    Exit,
}

impl ToString for TaskType {
    fn to_string(&self) -> String {
        match self {
            Self::Node(p) => format!("node{}", p.to_string()),
            Self::Network => "network".to_string(),
            Self::Help(None, None) => "help".to_string(),
            Self::Help(Some(x), None) => format!("help {}", x.to_string()),
            Self::Help(None, Some(y)) => format!("help {}", y),
            Self::Help(Some(x), Some(y)) => format!("help {} {}", x.to_string(), y),
            Self::Env => "env".to_string(),
            Self::Exit => "exit".to_string(),
        }
    }
}

impl TaskType {
    fn to_colored_string(&self) -> String {
        match self {
            Self::Node(p) => format!("{}{}", "node".red(), p.to_colored_string()),
            Self::Network => "network".red().to_string(),
            Self::Help(None, None) => "help".red().to_string(),
            Self::Help(Some(x), None) => format!("{} {}", "help".red(), x.to_string().red()),
            Self::Help(None, Some(y)) => format!("{} {}", "help".red(), y),
            Self::Help(Some(x), Some(y)) => {
                format!("{} {} {}", "help".red(), x.to_string().red(), y)
            }
            Self::Env => "env".red().to_string(),
            Self::Exit => "exit".red().to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskInput {
    None,
    Function(FunctionCall),
    Literal(Attribute),
    Variable(String),
}

impl TaskInput {
    pub fn to_colored_string(&self) -> String {
        match self {
            Self::None => "".into(),
            Self::Function(fc) => fc.to_colored_string(),
            Self::Literal(a) => a.to_colored_string(),
            Self::Variable(s) => s.green().to_string(),
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct FunctionCall {
    pub name: String,
    pub args: Vec<TaskInput>,
    pub kwargs: HashMap<String, TaskInput>,
}

impl FunctionCall {
    pub fn to_colored_string(&self) -> String {
        let mut args_str: Vec<String> = self
            .args
            .iter()
            .map(|a| a.to_colored_string().to_string())
            .collect();
        let kwargs_str: Vec<String> = self
            .kwargs
            .iter()
            .map(|(k, v)| format!("{}={}", k.to_string().blue(), v.to_colored_string()))
            .collect();
        args_str.extend(kwargs_str);
        format!(
            "{}({})",
            self.name.truecolor(80, 80, 200),
            args_str.join(", ")
        )
    }

    pub fn node_ctx(&self, node: &NodeInner) -> Result<FunctionCtx, String> {
        let args = self
            .args
            .iter()
            .map(|a| match a {
                TaskInput::Literal(v) => Ok(v.clone()),
                TaskInput::Variable(v) => node
                    .attr(v)
                    .cloned()
                    .ok_or(format!("Attribute {v} not found")),
                _ => Err(String::from("Invalid output")),
            })
            .collect::<Result<Vec<Attribute>, String>>()?
            .into();
        let kwargs = self
            .kwargs
            .iter()
            .map(|(k, a)| {
                let k = RString::from(k.as_str());
                match a {
                    TaskInput::Literal(v) => Ok((k, v.clone())),
                    TaskInput::Variable(v) => Ok((
                        k,
                        node.attr(v)
                            .cloned()
                            .ok_or(format!("Attribute {v} not found"))?,
                    )),
                    _ => Err(String::from("Invalid output")),
                }
            })
            .collect::<Result<HashMap<RString, Attribute>, String>>()?
            .into();
        Ok(FunctionCtx { args, kwargs })
    }

    // TODO this and above is duplicate, maybe use some trait for things with Attribute
    pub fn network_ctx(&self, net: &Network) -> Result<FunctionCtx, String> {
        let args = self
            .args
            .iter()
            .map(|a| match a {
                TaskInput::Literal(v) => Ok(v.clone()),
                TaskInput::Variable(v) => net
                    .attr(v)
                    .cloned()
                    .ok_or(format!("Attribute {v} not found")),
                _ => Err(String::from("Invalid output")),
            })
            .collect::<Result<Vec<Attribute>, String>>()?
            .into();
        let kwargs = self
            .kwargs
            .iter()
            .map(|(k, a)| {
                let k = RString::from(k.as_str());
                match a {
                    TaskInput::Literal(v) => Ok((k, v.clone())),
                    TaskInput::Variable(v) => Ok((
                        k,
                        net.attr(v)
                            .cloned()
                            .ok_or(format!("Attribute {v} not found"))?,
                    )),
                    _ => Err(String::from("Invalid output")),
                }
            })
            .collect::<Result<HashMap<RString, Attribute>, String>>()?
            .into();
        Ok(FunctionCtx { args, kwargs })
    }
}

#[derive(Clone, PartialEq, Debug)]
pub enum TaskKeyword {
    Node,
    Network,
    Env,
    Exit,
    Help,
}

impl ToString for TaskKeyword {
    fn to_string(&self) -> String {
        match self {
            TaskKeyword::Node => "node",
            TaskKeyword::Network => "network",
            TaskKeyword::Env => "env",
            TaskKeyword::Exit => "exit",
            TaskKeyword::Help => "help",
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
            TaskKeyword::Help => "help",
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
        "{} {} ({})\n{}",
        prefix.red(),
        name.truecolor(80, 80, 200),
        signature.blue(),
        format_md(&format!("{}\n{}\n{}", short_help, argshelp, desc))
    )
}

fn format_md(txt: &str) -> String {
    let mut skin = termimad::MadSkin::default_dark();
    for h in &mut skin.headers {
        h.align = termimad::Alignment::Left;
    }
    skin.text(txt, None).to_string()
}
