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
                if let Some(ref var) = task.attribute {
                    match task.input {
                        TaskInput::Literal(val) => {
                            self.env.insert(var.to_string().into(), val);
                            Ok(None)
                        }
                        TaskInput::Variable(v) => {
                            if let Some(v) = self.env.attr_dot(&v)? {
                                let cs = v.to_colored_string();
                                self.env.set_attr_dot(&var, v.clone()).map(|b| {
                                    b.map(|b| format!("{} -> {}", b.to_colored_string(), cs))
                                })
                            } else {
                                Err(format!("Attribute not found {}", var))
                            }
                        }
                        TaskInput::Function(fc) => match self.functions.env(&fc.name) {
                            Some(f) => handle_func_response(
                                // propagation doesn't make sense for env function though
                                f.call(&fc.context(&self.env, &self, &Propagation::default())?),
                                &mut self.env,
                                &task.attribute,
                            ),
                            None => Err(format!("Environment Function {} not found", fc.name)),
                        },
                        TaskInput::None => {
                            if let Some(v) = self.env.get(var.as_str()) {
                                Ok(Some(v.to_colored_string()))
                            } else {
                                Err(format!("Env variable {var} doesn't exist"))
                            }
                        }
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
                                    Ok(format!(
                                        "  {} = {}",
                                        n.name(),
                                        if let Some(a) = n
                                            .attr_dot(&attr)
                                            .map_err(|e| format!("Node {}: {e}", n.name()))?
                                        {
                                            a.to_colored_string()
                                        } else {
                                            "<None>".truecolor(100, 100, 100).to_string()
                                        }
                                    ))
                                })
                                .collect::<Result<Vec<String>, String>>()?;
                            Ok(Some(format!("{attr} = {{\n{}\n}}", attrs.join(",\n"))))
                        } else {
                            Ok(None) // it's just keyword with nothing to do
                        }
                    }
                    TaskInput::Literal(v) => {
                        if let Some(attr) = task.attribute {
                            let cs = v.to_colored_string();
                            let updates = nodes
                                .iter()
                                .map(|n| {
                                    let mut n = n.lock();
                                    n.set_attr_dot(&attr, v.clone())
                                        .map(|b| {
                                            b.map(|b| {
                                                format!(
                                                    "  {} = {} -> {}",
                                                    n.name(),
                                                    b.to_colored_string(),
                                                    cs
                                                )
                                            })
                                        })
                                        .map_err(|e| format!("Node {}: {e}", n.name()))
                                })
                                .collect::<Result<Vec<Option<String>>, String>>()?;
                            let updates: Vec<String> =
                                updates.into_iter().filter_map(|u| u).collect();
                            if updates.is_empty() {
                                Ok(None)
                            } else {
                                Ok(Some(format!("{{\n{}\n}}", updates.join(",\n"))))
                            }
                        } else {
                            Err("Invalid operation, no attribute to assign".to_string())
                        }
                    }
                    TaskInput::Variable(v) => {
                        if let Some(attr) = task.attribute {
                            let updates = nodes
                                .iter()
                                .map(|n| {
                                    let mut n = n.lock();
                                    let a = n
                                        .attr_dot(&v)
                                        .map_err(|e| format!("Node {}: {e}", n.name()))?
                                        .cloned();
                                    match a {
                                        Some(v) => {
                                            let cs = v.to_colored_string();
                                            n.set_attr_dot(&attr, v).map(|b| {
                                                b.map(|b| {
                                                    format!(
                                                        "  {} = {} -> {}",
                                                        n.name(),
                                                        b.to_colored_string(),
                                                        cs
                                                    )
                                                })
                                            })
                                        }
                                        None => Err(format!(
                                            "Node {}: Attribute {} not found",
                                            n.name(),
                                            v
                                        )),
                                    }
                                })
                                .collect::<Result<Vec<Option<String>>, String>>()?;
                            let updates: Vec<String> =
                                updates.into_iter().filter_map(|u| u).collect();
                            if updates.is_empty() {
                                Ok(None)
                            } else {
                                Ok(Some(format!("{{\n{}\n}}", updates.join(",\n"))))
                            }
                        } else {
                            Err("Invalid operation, no attribute to assign".to_string())
                        }
                    }
                    TaskInput::Function(fc) => {
                        let contex = |n: &NodeInner| -> Result<FunctionCtx, String> {
                            fc.context(n, &self, p)
                                .map_err(|e| format!("Node {}: {e}", n.name()))
                        };
                        let attrs = match self.functions.node(&fc.name) {
                            Some(f) => nodes
                                .iter()
                                .map(|n| {
                                    let mut node = n.lock();
                                    let ctx = contex(&node)?;
                                    let n: &mut NodeInner = &mut node;
                                    let res = f.call(n, &ctx);
                                    handle_func_response(res, n, &task.attribute)
                                        .map(|a| a.map(|a| format!("  {} = {a}", n.name())))
                                        .map_err(|e| format!("Node {}: {e}", n.name()))
                                })
                                .collect::<Result<Vec<Option<String>>, String>>()?,
                            None => match self.functions.env(&fc.name) {
                                Some(f) => nodes
                                    .iter()
                                    .map(|n| {
                                        let mut node = n.lock();
                                        let ctx = contex(&node)?;
                                        let n: &mut NodeInner = &mut node;
                                        let res = f.call(&ctx);
                                        handle_func_response(res, n, &task.attribute)
                                            .map(|a| a.map(|a| format!("  {} = {a}", n.name())))
                                            .map_err(|e| format!("Node {}: {e}", n.name()))
                                    })
                                    .collect::<Result<Vec<Option<String>>, String>>()?,
                                None => {
                                    return Err(format!("Node Function {} not found", fc.name));
                                }
                            },
                        };
                        let attrs = attrs.into_iter().filter_map(|v| v).collect::<Vec<String>>();
                        if attrs.is_empty() {
                            Ok(None)
                        } else {
                            Ok(Some(format!("{{\n{}\n}}", attrs.join(",\n"))))
                        }
                    }
                }
            }
            TaskType::Network(p) => match task.input {
                TaskInput::None => {
                    if let Some(attr) = task.attribute {
                        if let Some(a) = self.network.attr_dot(&attr)? {
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
                        let cs = a.to_colored_string();
                        self.network
                            .set_attr_dot(&attr, a.clone())
                            .map(|b| b.map(|b| format!("{} -> {}", b.to_colored_string(), cs)))
                    } else {
                        Ok(None)
                    }
                }
                TaskInput::Variable(var) => {
                    if let Some(attr) = task.attribute {
                        if let Some(v) = self.network.attr_dot(&var)? {
                            let cs = v.to_colored_string();
                            self.network
                                .set_attr_dot(&attr, v.clone())
                                .map(|b| b.map(|b| format!("{} -> {}", b.to_colored_string(), cs)))
                        } else {
                            Err(format!("Attribute not found {}", attr))
                        }
                    } else {
                        Err(format!("Nothing to do, found variable {}", var))
                    }
                }
                TaskInput::Function(fc) => {
                    let ctx = fc.context(&self.network, &self, p)?;
                    match self.functions.network(&fc.name) {
                        Some(f) => handle_func_response(
                            f.call(&mut self.network, &ctx),
                            &mut self.network,
                            &task.attribute,
                        ),
                        None => {
                            // if network function not found try environment function
                            match self.functions.env(&fc.name) {
                                Some(f) => handle_func_response(
                                    f.call(&ctx),
                                    &mut self.network,
                                    &task.attribute,
                                ),
                                None => Err(format!("Network Function {} not found", fc.name)),
                            }
                        }
                    }
                }
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
    Network(Propagation),
    Env,
    Help(Option<TaskKeyword>, Option<String>),
    Exit,
}

impl ToString for TaskType {
    fn to_string(&self) -> String {
        match self {
            Self::Node(p) => format!("node{}", p.to_string()),
            Self::Network(p) => format!("network{}", p.to_string()),
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
            Self::Network(p) => format!("{}{}", "network".red(), p.to_colored_string()),
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

    pub fn context<P: HasAttributes>(
        &self,
        node: &P,
        tctx: &TaskContext,
        propagation: &Propagation,
    ) -> Result<FunctionCtx, String> {
        let args = self
            .args
            .iter()
            .map(|a| match a {
                TaskInput::Literal(v) => Ok(v.clone()),
                TaskInput::Variable(v) => node
                    .attr_dot(v)?
                    .cloned()
                    .ok_or(format!("Attribute {v} not found")),
                TaskInput::Function(fc) => match tctx.functions.env(&fc.name) {
                    Some(f) => match f.call(&fc.context(node, tctx, propagation)?) {
                        FunctionRet::None => Err(format!("Function {} returned no value", fc.name)),
                        FunctionRet::Some(a) => Ok(a),
                        FunctionRet::Error(e) => Err(e.to_string()),
                    },
                    None => Err(format!("Environment Function {} not found", fc.name)),
                },
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
                        node.attr_dot(v)?
                            .cloned()
                            .ok_or(format!("Attribute {v} not found"))?,
                    )),
                    TaskInput::Function(fc) => match tctx.functions.env(&fc.name) {
                        Some(f) => match f.call(&fc.context(node, tctx, propagation)?) {
                            FunctionRet::None => {
                                Err(format!("Function {} returned no value", fc.name))
                            }
                            FunctionRet::Some(a) => Ok((k, a)),
                            FunctionRet::Error(e) => Err(e.to_string()),
                        },
                        None => Err(format!("Environment Function {} not found", fc.name)),
                    },
                    _ => Err(String::from("Invalid output")),
                }
            })
            .collect::<Result<HashMap<RString, Attribute>, String>>()?
            .into();
        Ok(FunctionCtx {
            args,
            kwargs,
            propagation: propagation.clone(),
        })
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

fn handle_func_response<P: HasAttributes>(
    res: FunctionRet,
    pt: &mut P,
    attr: &Option<String>,
) -> Result<Option<String>, String> {
    match res {
        FunctionRet::None => Ok(None),
        FunctionRet::Some(a) => {
            let cs = a.to_colored_string();
            if let Some(attr) = attr {
                pt.set_attr_dot(&attr, a)
                    .map(|b| b.map(|b| format!("{} -> {}", b.to_colored_string(), cs)))
            } else {
                Ok(Some(cs))
            }
        }
        FunctionRet::Error(e) => Err(e.to_string()),
    }
}
