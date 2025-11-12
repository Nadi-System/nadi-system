use crate::expressions::{EvalError, EvalErrorType, Expression, TaskPosition};
use crate::functions::{FuncArg, FuncArgType, NadiFunctions};
use crate::network::PropCondition;
use crate::prelude::*;
use crate::udf::UserFunction;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};

// /// Result of a Task when executed
// pub enum TaskResult {
//     None,
//     Value(Attribute),
//     Update(Attribute, Attribute),
//     Return(Attribute),
//     Help(String),
//     Error(EvalError),
//     OrderedTable(AttrMap, Vec<String>),
// }

/// Message that can be send from the task
#[derive(Debug, Clone)]
pub enum TaskMessage {
    Progress(String, usize, usize),
    Warning(String),
    Info(String),
}

impl TaskMessage {
    pub fn print(&self) {
        match self {
            Self::Progress(l, i, t) => {
                eprintln!("{l}: {}", i * 100 / t);
            }
            Self::Warning(msg) => eprintln!("WARN: {msg}"),
            Self::Info(msg) => eprintln!("INFO: {msg}"),
        }
    }
}

/// Wrapper for TaskContext without any Channel
pub struct TaskContextWrap {
    pub receiver: Receiver<TaskMessage>,
    pub context: TaskContext,
}

impl TaskContextWrap {
    pub fn new(net: Option<Network>) -> Self {
        let (sender, receiver) = channel();
        let context = TaskContext::new(net, sender);
        Self { receiver, context }
    }
}

impl TaskContextWrap {
    pub fn execute(&mut self, task: Task) -> Result<Option<String>, EvalError> {
        let msg: Vec<String> = self
            .receiver
            .try_recv()
            .into_iter()
            .filter_map(|m| match m {
                TaskMessage::Info(i) | TaskMessage::Warning(i) => Some(i),
                _ => None,
            })
            .collect();
        match self.context.execute(task) {
            Ok(Some(v)) => Ok(Some(format!("{v}\n{}", msg.join("\n")))),
            Ok(None) => Ok(Some(msg.join("\n"))),
            Err(e) => Err(e),
        }
    }
}

/// Main Context for Task System
///
/// Everything is evaluated in the task context while using the task
/// system. It contains a network, functions loaded from the plugins
/// and environment variables.
#[derive(Clone)]
pub struct TaskContext {
    /// Network in the context
    pub network: Network,
    /// Functions loaded from the plugins
    pub functions: NadiFunctions,
    /// User defined functions (only env functions with single expression now)
    pub udf: HashMap<String, UserFunction>,
    /// environment variables
    pub env: AttrMap,
    /// tasks to run after every assign execution
    pub hook: Vec<Task>,
    /// channel for sending messages
    pub channel: Sender<TaskMessage>,
    // TODO Channel to tell taskcontext to abort/cancel the current run. When it takes a long time, like while loop/ node functions can end in the middle.
}

impl TaskContext {
    pub fn new(net: Option<Network>, channel: Sender<TaskMessage>) -> Self {
        Self {
            network: net.unwrap_or_default(),
            functions: NadiFunctions::new(),
            udf: HashMap::new(),
            env: AttrMap::new(),
            hook: Vec::new(),
            channel,
        }
    }

    pub fn clear(&mut self) {
        self.network = Network::default();
        self.env = AttrMap::new();
    }

    pub fn udf(&self, name: &str) -> Option<&UserFunction> {
        self.udf.get(name)
    }

    pub fn run_hooks(&mut self) {
        for t in self.hook.clone() {
            _ = self.execute_single(t);
        }
    }

    /// execute a task in the task context, possible with hook
    pub fn execute(&mut self, task: Task) -> Result<Option<String>, EvalError> {
        match task {
            Task::Eval(_) => {
                let val = self.execute_single(task)?;
                self.run_hooks();
                Ok(val)
            }
            t => self.execute_single(t),
        }
    }

    /// execute a task in the task context
    pub fn execute_single(&mut self, task: Task) -> Result<Option<String>, EvalError> {
        match task {
            Task::Function(fdef) => {
                if let Some(name) = fdef.name() {
                    // TODO: check the function doesn't have
                    // node/inputs/ and output variable types (those
                    // that can't be calculated in env context)
                    self.udf.insert(name.into(), fdef);
                    Ok(None)
                } else {
                    Ok(Some("Anonymous Function".into()))
                }
            }
            Task::Return(_) => Err(EvalErrorType::InvalidReturn.no_pos()),
            Task::Expr(expr) => expr
                .resolve_eval(&FunctionType::Env, self, None, None)
                .map(|a| a.map(|a| a.to_string())),
            Task::Hook(tasks) => {
                self.hook = tasks;
                Ok(None)
            }
            Task::Eval(et) => self.eval_task(et),
            Task::Attr(at) => self.attr_task(at).map(Some),
            Task::Conditional(ct) => {
                let cond = ct.cond.resolve(&FunctionType::Env, self, None, None)?;
                let res = cond.eval_value(&FunctionType::Env, self, None, None)?;
                match bool::try_from_attr(&res)
                    .map_err(|e| EvalErrorType::AttributeError(e).pos(ct.position()))?
                {
                    true => {
                        let mut progress = 0;
                        let total = ct.iftrue.len();
                        for task in ct.iftrue {
                            let _ = self.channel.send(TaskMessage::Progress(
                                task.to_string(),
                                progress,
                                total,
                            ));
                            if let Some(a) = self.execute(task.clone())? {
                                let _ = self.channel.send(TaskMessage::Info(a));
                            }
                            progress += 1;
                        }
                    }
                    false => {
                        let mut progress = 0;
                        let total = ct.iffalse.len();
                        for task in ct.iffalse {
                            let _ = self.channel.send(TaskMessage::Progress(
                                task.to_string(),
                                progress,
                                total,
                            ));
                            if let Some(a) = self.execute(task.clone())? {
                                let _ = self.channel.send(TaskMessage::Info(a));
                            }
                            progress += 1;
                        }
                    }
                }
                Ok(None)
            }
            Task::WhileLoop(lt) => {
                let max_iter = 1_000_000; // todo make it a constant (maybe configurable)
                let mut progress = 0;
                for i in 0..max_iter {
                    let _ = self.channel.send(TaskMessage::Progress(
                        format!("Loop: {}", i + 1),
                        progress,
                        max_iter,
                    ));
                    let cond = lt.cond.resolve(&FunctionType::Env, self, None, None)?;
                    let res = cond.eval_value(&FunctionType::Env, self, None, None)?;
                    match bool::try_from_attr(&res)
                        .map_err(|e| EvalErrorType::AttributeError(e).pos(lt.position()))?
                    {
                        true => {
                            for task in &lt.tasks {
                                if let Some(a) = self.execute(task.clone())? {
                                    let _ = self.channel.send(TaskMessage::Info(a));
                                }
                                progress += 1;
                            }
                        }
                        false => break,
                    }
                }
                Ok(None)
            }
            Task::Help(kw, var) => self.help(kw, var),
            Task::Exit => std::process::exit(0),
        }
    }

    /// evaluate a task and possibly get return value in terms of string.
    pub fn eval_task(&mut self, task: EvalTask) -> Result<Option<String>, EvalError> {
        match task.ty {
            FunctionType::Env => match task
                .input
                .resolve_eval(&FunctionType::Env, self, None, None)
                .map_err(|e| e.pos(task.position()))?
            {
                Some(a) => {
                    if let Some(attr) = &task.attr {
                        if let Some(old) = self
                            .env
                            .set_attr_nested(&task.attr_pre, attr, a.clone())
                            .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?
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
                let nodes = self.propagation(task.propagation.unwrap_or_default())?;
                let total = nodes.len();
                let mut progress = 0;
                let mut attrs = Vec::with_capacity(total);
                for n in nodes {
                    let res = match task
                        .input
                        // add node name to this error
                        .resolve_eval_mut(&FunctionType::Node, self, None, Some(&n))
                        .map_err(|e| e.pos(task.start))?
                    {
                        Some(r) => r,
                        None => {
                            progress += 1;
                            continue;
                        }
                    };
                    let mut n = n
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(task.start))?;
                    progress += 1;
                    let _ = self.channel.send(TaskMessage::Progress(
                        n.name().to_string(),
                        progress,
                        total,
                    ));
                    if let Some(attr) = &task.attr {
                        let old = n
                            .set_attr_nested(&task.attr_pre, attr, res.clone())
                            .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?;
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
                    .resolve_eval_mut(&FunctionType::Network, self, None, None)
                    .map_err(|e| e.pos(task.position()))?
                {
                    Some(a) => {
                        if let Some(attr) = &task.attr {
                            if let Some(old) = self
                                .network
                                .set_attr_nested(&task.attr_pre, attr, a.clone())
                                .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?
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
                }
            }
        }
    }

    /// evaluate an attribute task
    pub fn attr_task(&self, task: AttrTask) -> Result<String, EvalError> {
        match task.ty {
            FunctionType::Env => self
                .env
                .attr_nested(&task.attr_pre, &task.attr)
                .map_err(|e| EvalErrorType::AttributeError(e).pos(task.position()))?
                .map(|a| a.to_string())
                .ok_or(EvalErrorType::AttributeNotFound.pos(task.position())),
            FunctionType::Node => {
                let nodes = self.propagation(task.propagation.clone().unwrap_or_default())?;
                let attrs = nodes
                    .iter()
                    .map(|n| {
                        let n = n.lock();
                        Ok(format!(
                            "  {} = {}",
                            n.name(),
                            if let Some(a) =
                                n.attr_nested(&task.attr_pre, &task.attr).map_err(|e| {
                                    EvalErrorType::AttributeError(format!("Node {}: {e}", n.name()))
                                        .pos(task.position())
                                })?
                            {
                                a.to_string()
                            } else {
                                "<None>".to_string()
                            }
                        ))
                    })
                    .collect::<Result<Vec<String>, EvalError>>()?;
                Ok(format!("{{\n{}\n}}", attrs.join(",\n")))
            }
            FunctionType::Network => self
                .network
                .attr_nested(&task.attr_pre, &task.attr)
                .map_err(|e| EvalErrorType::AttributeError(e).pos(task.position()))?
                .map(|a| a.to_string())
                .ok_or(EvalErrorType::AttributeNotFound.pos(task.position())),
        }
    }

    /// get help
    pub fn help(
        &self,
        kw: Option<TaskKeyword>,
        var: Option<String>,
    ) -> Result<Option<String>, EvalError> {
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
                    Err(EvalErrorType::FunctionNotFound(None, var).no_pos())
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
                    Err(EvalErrorType::FunctionNotFound(Some(FunctionType::Node), var).no_pos())
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
                    Err(EvalErrorType::FunctionNotFound(Some(FunctionType::Network), var).no_pos())
                }
            }
            (Some(kw), None) => Ok(Some(kw.help())),
            (Some(kw), Some(x)) => {
                Err(EvalErrorType::FunctionNotFound(FunctionType::from_keyword(&kw), x).no_pos())
            }
            (None, None) => Ok(Some("Usage: help <keyword> [function]".into())),
        }
    }

    /// Get node propagation using the context (network and variables)
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
                    let cond = expr.resolve(&FunctionType::Node, self, None, Some(&n))?;
                    let res = cond.eval_value(&FunctionType::Node, self, None, Some(&n))?;
                    match bool::try_from_attr(&res) {
                        Ok(true) => sel_nodes.push(n),
                        Ok(false) => (),
                        Err(e) => {
                            return Err(EvalErrorType::NodeAttributeError(
                                n.lock().name().to_string(),
                                e,
                            )
                            .pos(prop.start));
                        }
                    }
                }
                Ok(sel_nodes)
            }
        }
    }
}

/// Types of functions
#[derive(Debug, Clone, PartialEq)]
pub enum FunctionType {
    /// environement function
    Env,
    /// Node function
    Node,
    /// network function
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

/// Task representing evaluation of expression or functions
#[derive(Clone, PartialEq, Debug)]
pub struct EvalTask {
    /// type of function
    pub ty: FunctionType,
    /// node propagation for node function
    pub propagation: Option<Propagation>,
    /// prefix for set attribute
    pub attr_pre: Vec<String>,
    /// attribute to set the result of the expression
    pub attr: Option<String>,
    /// input expression
    pub input: Expression,
    /// do not show the results to stdout/terminal
    pub silent: bool,
    /// start position of the task
    pub start: (usize, usize),
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

/// Task representing getting of attribute value
#[derive(Clone, PartialEq, Debug)]
pub struct AttrTask {
    /// type of function
    pub ty: FunctionType,
    /// node propagation for node function
    pub propagation: Option<Propagation>,
    /// prefix for set attribute
    pub attr_pre: Vec<String>,
    /// attribute to get
    pub attr: String,
    /// start position of the task
    pub start: (usize, usize),
}

impl std::fmt::Display for AttrTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let outattr = format!(
            ".{}",
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

/// Task representing conditional task
#[derive(Clone, PartialEq, Debug)]
pub struct CondTask {
    /// condition to evaluate and test
    pub cond: Expression,
    /// tasks to run if condition is true
    pub iftrue: Vec<Task>,
    /// tasks to run if condition is false
    pub iffalse: Vec<Task>,
    /// start position of the task
    pub start: (usize, usize),
}

impl CondTask {
    /// The given task has the capacity to change the task context
    pub fn can_mutate(&self) -> bool {
        self.iftrue.iter().any(|t| t.can_mutate()) || self.iffalse.iter().any(|t| t.can_mutate())
    }
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

/// Task representing a while loop
#[derive(Clone, PartialEq, Debug)]
pub struct WhileTask {
    /// condition to evaluate and test before each evaluation
    pub cond: Expression,
    /// tasks to execute each time
    pub tasks: Vec<Task>,
    /// start position of the task
    pub start: (usize, usize),
}

impl WhileTask {
    /// The given task has the capacity to change the task context
    pub fn can_mutate(&self) -> bool {
        self.tasks.iter().any(|t| t.can_mutate())
    }
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

/// Execution body of the Task System
#[derive(Clone, PartialEq, Debug)]
pub enum Task {
    /// Evaluate the expression (possible set values)
    Eval(EvalTask),
    /// get an attribute
    Attr(AttrTask),
    /// conditionally execute tasks
    Conditional(CondTask),
    /// execute tasks in a loop
    WhileLoop(WhileTask),
    /// Tasks to run after each eval execution
    Hook(Vec<Task>),
    /// get function help information
    Help(Option<TaskKeyword>, Option<String>),
    /// Function Definition (needs to be named),
    Function(UserFunction),
    /// Return the value if inside a function
    Return(Expression),
    /// Evaluate the expression
    Expr(Expression),
    /// exit the task system/process
    Exit,
}
// TODO: add import task that loads a file; to be used to import function definitions from the file. We can make it only import the functions and not run anything. Alternatively use load keyword that will run everything in the current task context

// While working on this, also define a syntax to alias a function. Can either do single function or whole plugin

impl Task {
    /// The given task has the capacity to change the task context
    pub fn can_mutate(&self) -> bool {
        match self {
            Task::Eval(_) => true,
            Task::Attr(_) => false,
            Task::Conditional(c) => c.can_mutate(),
            Task::WhileLoop(w) => w.can_mutate(),
            Task::Hook(ht) => ht.iter().any(|t| t.can_mutate()),
            Task::Help(_, _) => false,
            Task::Function(_) => false,
            Task::Return(_) => false,
            Task::Expr(_) => false,
            Task::Exit => false,
        }
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Self::Eval(et) => std::fmt::Display::fmt(et, f),
            Self::Attr(at) => std::fmt::Display::fmt(at, f),
            Self::Conditional(t) => std::fmt::Display::fmt(t, f),
            Self::WhileLoop(t) => std::fmt::Display::fmt(t, f),
            Self::Hook(tasks) => write!(
                f,
                "hook {{\n{}\n}}",
                tasks
                    .iter()
                    .map(|t| t.to_string())
                    .collect::<Vec<String>>()
                    .join("\n")
            ),
            Self::Help(None, None) => write!(f, "help"),
            Self::Help(Some(kw), None) => write!(f, "help {kw}"),
            Self::Help(None, Some(s)) => write!(f, "help {s}"),
            Self::Help(Some(kw), Some(s)) => write!(f, "help {kw} {s}"),
            Task::Function(fdef) => write!(f, "{fdef}"),
            Task::Return(expr) => write!(f, "return({expr})"),
            Task::Expr(expr) => write!(f, "{expr}"),
            Self::Exit => write!(f, "exit"),
        }
    }
}

/// Keywords in the task system
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
    Root,
    If,
    Else,
    While,
    Try,
    Catch,
    In,
    Match,
    Hook,
    Local,
    Function,
    Return,
    Error,
    // reserved
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
            "root" => TaskKeyword::Root,
            "if" => TaskKeyword::If,
            "else" => TaskKeyword::Else,
            "while" => TaskKeyword::While,
            "try" => TaskKeyword::Try,
            "catch" => TaskKeyword::Catch,
            "in" => TaskKeyword::In,
            "match" => TaskKeyword::Match,
            "hook" => TaskKeyword::Hook,
            "loc" | "local" => TaskKeyword::Local,
            "function" | "func" => TaskKeyword::Function,
            "return" => TaskKeyword::Return,
            "error" => TaskKeyword::Error,
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
                TaskKeyword::Root => "root",
                TaskKeyword::If => "if",
                TaskKeyword::Else => "else",
                TaskKeyword::While => "while",
                TaskKeyword::Try => "try",
                TaskKeyword::Catch => "catch",
                TaskKeyword::In => "in",
                TaskKeyword::Match => "match",
                TaskKeyword::Hook => "hook",
                TaskKeyword::Local => "local",
                TaskKeyword::Function => "function",
                TaskKeyword::Return => "return",
                TaskKeyword::Error => "error",
                TaskKeyword::Map => "map",
                TaskKeyword::Attrs => "attrs",
                TaskKeyword::Loop => "loop",
                TaskKeyword::For => "for",
            }
        )
    }
}

impl TaskKeyword {
    #[cfg(not(tarpaulin_include))]
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
            TaskKeyword::Root => "root node of the network",
            TaskKeyword::If => "if part of if-else block",
            TaskKeyword::Else => "else part of if-else block",
            TaskKeyword::While => "while loop",
            TaskKeyword::Try => "try statement to contain tasks",
            TaskKeyword::Catch => "catch statement when error occurs on try block",
            TaskKeyword::In => "Check if value is in an array/table",
            TaskKeyword::Match => "match regex pattern with strings",
            TaskKeyword::Hook => "hook tasks to run at each execution",
            TaskKeyword::Local => "Local; similar to environment but within current locale",
            TaskKeyword::Function => "function definition",
            TaskKeyword::Return => "return statement inside function",
            TaskKeyword::Error => "raises an error while evaluating",
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

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;
    use std::str::FromStr;

    #[rstest]
    fn test_keyword(
        #[values(
            "node", "network", "env", "exit", "end", "help", "inputs", "output", "nodes", "root",
            "local", "if", "else", "while", "in", "match", "function", "map", "attrs", "loop",
            "for"
        )]
        tk: &str,
    ) {
        assert_eq!(TaskKeyword::from_str(tk).unwrap().to_string(), tk);
    }

    #[rstest]
    #[case("loc", "local")]
    #[case("net", "network")]
    #[case("func", "function")]
    fn test_keyword_equivalent(#[case] tk: &str, #[case] eqvl: &str) {
        assert_eq!(TaskKeyword::from_str(tk).unwrap().to_string(), eqvl);
    }
}
