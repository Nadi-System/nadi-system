use crate::eval::{Eval, EvalCtx, EvalError, EvalErrorType};
use crate::expressions::{
    ExprContext, ExprResult, ExprType, RawExpr, ResolvedExpr, SeriesExpression,
};

use crate::functions::{FuncArg, FuncArgType, NadiFunctions};
use crate::network::{PropCondition, PropOrder};
use crate::prelude::*;
use crate::structs::{NadiAttrType, NadiStruct};
use crate::timeseries::{HasSeries, HasTimeSeries, SeriesMap, TsMap};
use crate::udf::UserFunction;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

// /// Result of a Task when executed => move to expression result => move to function return
// pub enum TaskResult {
//     None,
//     Value(Attribute),
//     Update(Attribute, Attribute),
//     Return(Attribute),
//     OrderedTable(AttrMap, Vec<String>),
//     Help(String),  // this can be formatted text
//     Error(EvalError),
//     Image(PathBuf),
//     File(PathBuf),
//     FormattedText(String),
// }

/// Some constants that are similar to system variables
///
/// These constants decide how certain things are shown in the task
/// context, users can set the values to these constants to change how
/// task system acts. The macro rules make it easy to add more
/// variables for future.
pub struct TaskCtxConsts;

macro_rules! task_ctx_consts {
    ($($func:ident, $name:literal, $t:ty => $value:expr);+ $(;)*) => {
        impl TaskCtxConsts {
	    pub fn init() -> AttrMap {
		let mut env = AttrMap::new();
		$(
		    env.set_attr($name, $value.into());
		)+
		    env
	    }

            $(
		pub fn $func(ctx: &TaskContext) -> $t {
                ctx.env.try_attr($name).unwrap_or($value)
		}
	    )+
        }
    };
}

task_ctx_consts!(
    max_nodes_length, "MAX_NODES_LENGTH", usize => 50;
    max_attrs_length, "MAX_ATTRS_LENGTH", usize => 100;
    max_attrs_depth, "MAX_ATTRS_DEPTH", usize => 10;
    max_series_length, "MAX_SERIES_LENGTH", usize => 10;
    max_iterations, "MAX_ITERATIONS", usize => 10_000_000;
    series_show_na_as, "SERIES_SHOW_NA_AS", String => "-".to_string();
    parallize_nodes, "PARALLIZE_NODES", bool => false;
    parallel_cores, "PARALLEL_CORES", usize => 8;
    prettify_map, "PRETTIFY_MAP", bool => true;
    prettify_all_map, "PRETTIFY_ALL_MAP", bool => false;
    track_change, "TRACK_CHANGE", bool => true;
);

/// Message that can be sent from the task
#[derive(Debug, Clone)]
pub enum TaskMessage {
    Progress(String, usize, usize),
    Warning(String),
    Info(String),
    /// Value changed (mut function or expression called)
    Changed,
}

impl TaskMessage {
    pub fn print(&self) {
        match self {
            Self::Progress(l, i, t) => {
                eprintln!("{l}: {}", i * 100 / t);
            }
            Self::Warning(msg) => eprintln!("WARN: {msg}"),
            Self::Info(msg) => eprintln!("INFO: {msg}"),
            Self::Changed => (),
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

/// Environment for Task Context to save attributes, series and ts
#[derive(Clone)]
pub struct TaskContextEnv {
    /// Environment variables
    pub(crate) attrs: AttrMap,
    /// Environment Series
    pub(crate) series: SeriesMap,
    /// Environment TimeSeries
    pub(crate) timeseries: TsMap,
}

impl TaskContextEnv {
    pub fn new() -> Self {
        Self {
            attrs: TaskCtxConsts::init(),
            series: SeriesMap::new(),
            timeseries: TsMap::new(),
        }
    }
}

impl HasAttributes for TaskContextEnv {
    fn attr_map(&self) -> &AttrMap {
        &self.attrs
    }

    fn attr_map_mut(&mut self) -> &mut AttrMap {
        &mut self.attrs
    }
}

impl HasSeries for TaskContextEnv {
    fn series_map(&self) -> &SeriesMap {
        &self.series
    }

    fn series_map_mut(&mut self) -> &mut SeriesMap {
        &mut self.series
    }
}

impl HasTimeSeries for TaskContextEnv {
    fn ts_map(&self) -> &TsMap {
        &self.timeseries
    }

    fn ts_map_mut(&mut self) -> &mut TsMap {
        &mut self.timeseries
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
    /// Functions loaded from the plugins
    pub structs: HashMap<String, NadiStruct>,
    /// User defined functions (only env functions with single expression now)
    pub udf: HashMap<String, UserFunction>,
    /// Environment variables, series and timeseries
    pub env: TaskContextEnv,
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
            structs: HashMap::new(),
            udf: HashMap::new(),
            env: TaskContextEnv::new(),
            hook: Vec::new(),
            channel,
        }
    }

    pub fn clear(&mut self) {
        self.network = Network::default();
        self.env = TaskContextEnv::new();
        self.structs = HashMap::new();
        self.udf = HashMap::new();
        self.hook = Vec::new();
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
        self.execute_single(task)
    }

    /// execute a task in the task context
    pub fn execute_single(&mut self, task: Task) -> Result<Option<String>, EvalError> {
        match task {
            Task::Network => Ok(Some(format!("{:?}", self.network))),
            Task::Env => Ok(Some(format!("{:?}", self.env.attrs))),
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
            Task::StructDef(sdef) => {
                self.structs.insert(sdef.name.to_string(), sdef);
                Ok(None)
            }
            Task::Expr(expr) => {
                let ectx = EvalCtx::default();
                expr.eval_mut(self, &ectx).map(|a| self.show_res(&a, 0))
            }
            Task::Hook(tasks) => {
                self.hook = tasks;
                Ok(None)
            }
            Task::GetSeries(gst) => self.get_series_task(gst).map(Some),
            Task::SetSeries(sst) => {
                self.set_series_task(sst)?;
                Ok(None)
            }
            Task::Help(kw, var) => self.help(kw, var),
            Task::Clear => {
                self.clear();
                Ok(None)
            }
            Task::Exit => std::process::exit(0),
        }
    }

    pub fn get_series_task(&self, gst: GetSeriesTask) -> Result<String, EvalError> {
        match (gst.timeseries, gst.ty) {
            (_, FunctionType::Env) => self
                .env
                .try_series(&gst.name)
                .map(|sr| self.show_sr(sr))
                .map_err(|e| EvalErrorType::SeriesNotFound(e).pos(gst.start)),
            (ts, FunctionType::Node) => {
                let nodes = self.propagation(gst.propagation.clone().unwrap_or_default())?;
                let max_nodes_len = TaskCtxConsts::max_nodes_length(self);
                let trunc = nodes.len() > max_nodes_len;
                let attrs = nodes
                    .iter()
                    .take(max_nodes_len)
                    .map(|n| {
                        let n = n.lock();
                        format!(
                            "  {} = {}",
                            n.name(),
                            if ts {
                                n.try_ts(&gst.name).map(|ts| self.show_ts(ts))
                            } else {
                                n.try_series(&gst.name).map(|ts| self.show_sr(ts))
                            }
                            .unwrap_or(crate::expressions::NONE_VALUE.to_string())
                        )
                    })
                    .collect::<Vec<String>>();
                Ok(format!(
                    "{{\n{}\n{}}}",
                    attrs.join(",\n"),
                    if trunc { "...truncated\n" } else { "" }
                ))
            }
            (true, FunctionType::Network) => self
                .network
                .try_ts(&gst.name)
                .map(|ts| self.show_ts(ts))
                .map_err(|e| EvalErrorType::TimeSeriesNotFound(e).pos(gst.start)),
            (false, FunctionType::Network) => self
                .network
                .try_series(&gst.name)
                .map(|sr| self.show_sr(sr))
                .map_err(|e| EvalErrorType::SeriesNotFound(e).pos(gst.start)),
        }
    }

    pub fn set_series_task(&mut self, sst: SetSeriesTask) -> Result<(), EvalError> {
        match (sst.timeseries, sst.ty) {
            (_, FunctionType::Env) => {
                let series =
                    sst.expression
                        .resolve_eval_value(&FunctionType::Env, self, None, None)?;

                self.env.set_series(&sst.name, series);
                Ok(())
            }
            (false, FunctionType::Node) => {
                let nodes = self.propagation(sst.propagation.clone().unwrap_or_default())?;
                nodes.iter().try_for_each(|n| -> Result<(), EvalError> {
                    let series = sst.expression.resolve_eval_value(
                        &FunctionType::Node,
                        self,
                        None,
                        Some(n),
                    )?;
                    n.lock().set_series(&sst.name, series);
                    Ok(())
                })?;
                Ok(())
            }
            (true, FunctionType::Node) => {
                Err(EvalErrorType::NotImplementedError("Can not set timeseries").pos(sst.start))
            }
            (true, FunctionType::Network) => {
                Err(EvalErrorType::NotImplementedError("Can not set timeseries").pos(sst.start))
            }
            (false, FunctionType::Network) => {
                let series =
                    sst.expression
                        .resolve_eval_value(&FunctionType::Network, self, None, None)?;

                self.network.set_series(&sst.name, series);
                Ok(())
            }
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
                // expression is evaluated for each node
                let ectx = EvalCtx::default();
                for n in nodes {
                    let ectx = ectx.at_node(n.clone());
                    let res = expr
                        .clone()
                        .resolve(self, ectx.clone())?
                        .eval_value(self, &ectx)?;
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
#[derive(Debug, Clone, PartialEq, Default)]
pub enum FunctionType {
    /// environement function
    #[default]
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

    pub fn get_expr_context(&self, node: Option<&Node>) -> Result<ExprContext, EvalErrorType> {
        match self {
            Self::Node => node
                .ok_or(EvalErrorType::NotANodeContext)
                .cloned()
                .map(ExprContext::Node),
            Self::Network => Ok(ExprContext::Network),
            Self::Env => Ok(ExprContext::Env),
        }
    }
}

/// Task representing getting of series/timeseries value
#[derive(Clone, PartialEq, Debug)]
pub struct GetSeriesTask {
    /// type of function
    pub ty: FunctionType,
    /// node propagation for node function
    pub propagation: Option<Propagation>,
    /// Timeseries instead of Series
    pub timeseries: bool,
    /// name of the series/timeseries
    pub name: String,
    // TODO: Add indexing capabilities (multiple index should be accepted)
    // pub index: Option<Range>,
    /// start position of the task
    pub start: (usize, usize),
}

impl std::fmt::Display for GetSeriesTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}{}",
            self.ty,
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            if self.timeseries { "$$" } else { "$" },
            self.name
        )
    }
}

/// Task representing getting of series/timeseries value
#[derive(Clone, PartialEq, Debug)]
pub struct SetSeriesTask {
    /// type of function
    pub ty: FunctionType,
    /// node propagation for node function
    pub propagation: Option<Propagation>,
    /// Timeseries instead of Series
    pub timeseries: bool,
    /// name of the series/timeseries
    pub name: String,
    /// Series Evaluation Expression
    pub expression: SeriesExpression,
    // TODO: Add indexing capabilities (multiple index should be accepted)
    // pub index: Option<Range>,
    /// start position of the task
    pub start: (usize, usize),
}

impl std::fmt::Display for SetSeriesTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}{} = {}",
            self.ty,
            self.propagation
                .as_ref()
                .map(|p| p.to_string())
                .unwrap_or_default(),
            if self.timeseries { "$$" } else { "$" },
            self.name,
            self.expression,
        )
    }
}

/// Execution body of the Task System
#[derive(Clone, PartialEq, Debug)]
pub enum Task {
    /// Tasks to run after each eval execution
    Hook(Vec<Task>),
    /// get function help information
    Help(Option<TaskKeyword>, Option<String>),
    /// Function Definition (needs to be named),
    Function(UserFunction),
    /// Struct definition
    StructDef(NadiStruct),
    /// Evaluate the expression
    Expr(RawExpr),
    /// Get a series/timeseries from the node/network/env
    GetSeries(GetSeriesTask),
    /// Set a series/timeseries to the node/network/env
    SetSeries(SetSeriesTask),
    /// Network details
    Network,
    /// Env details
    Env,
    /// Clear the task context
    Clear,
    /// exit the task system/process,
    Exit,
}
// TODO: add import task that loads a file; to be used to import function definitions from the file. We can make it only import the functions and not run anything. Alternatively use load keyword that will run everything in the current task context

// While working on this, also define a syntax to alias a function. Can either do single function or whole plugin

impl Task {
    /// The given task has the capacity to change the network/node/env
    pub fn can_mutate(&self) -> bool {
        match self {
            Task::Hook(ht) => ht.iter().any(|t| t.can_mutate()),
            Task::Help(_, _) => false,
            Task::Function(_) => false,
            Task::StructDef(_) => false,
            Task::Expr(_) => false,
            Task::GetSeries(_) => false,
            Task::SetSeries(_) => true,
            Task::Network => false,
            Task::Env => false,
            Task::Clear => false,
            Task::Exit => false,
        }
    }

    /// Message for the current task's functionality
    pub fn message(&self) -> &'static str {
        match self {
            Task::Hook(_) => "Evaluate these tasks after each eval task",
            Task::Help(_, _) => "Show help",
            Task::Function(_) => "Define a new user function",
            Task::StructDef(_) => "Defines a new user struct",
            Task::Expr(_) => "Evaluate expression",
            Task::GetSeries(_) => "Query Series/TimeSeries values",
            Task::SetSeries(_) => "Set Series/TimeSeries values",
            Task::Network => "Show current Network Details",
            Task::Env => "Show current environment",
            Task::Clear => "Clear the task context",
            Task::Exit => "Exit the program",
        }
    }
}

impl std::fmt::Display for Task {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
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
            Task::StructDef(sdef) => write!(f, "{sdef}"),
            Task::Expr(expr) => write!(f, "{expr}"),
            Task::GetSeries(gst) => write!(f, "{gst}"),
            Task::SetSeries(sst) => write!(f, "{sst}"),
            Self::Network => write!(f, "network"),
            Self::Env => write!(f, "env"),
            Self::Clear => write!(f, "clear"),
            Self::Exit => write!(f, "exit"),
        }
    }
}

/// Keywords in the task system
#[derive(Clone, PartialEq, Debug)]
pub enum TaskKeyword {
    Clear,
    Import,
    Exec,
    From,
    Node,
    Network,
    Env,
    Exit,
    End,
    Help,
    Input,
    Inputs,
    InputsMap,
    Output,
    Outputs,
    OutputsMap,
    Nodes,
    NodesMap,
    Root,
    Roots,
    RootsMap,
    Leaves,
    LeavesMap,
    If,
    Else,
    While,
    Try,
    Catch,
    In,
    Match,
    Hook,
    Local,
    Struct,
    Function,
    Return,
    Break,
    Continue,
    Error,
    For,
    Loop,
    Progress,
    // reserved
    Networks,
    NetworksMap,
    Map,
    Attrs,
}

impl std::str::FromStr for TaskKeyword {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "clear" => TaskKeyword::Clear,
            "import" => TaskKeyword::Import,
            "exec" => TaskKeyword::Exec,
            "from" => TaskKeyword::From,
            "node" => TaskKeyword::Node,
            "network" | "net" => TaskKeyword::Network,
            "env" => TaskKeyword::Env,
            "exit" => TaskKeyword::Exit,
            "end" => TaskKeyword::End,
            "help" => TaskKeyword::Help,
            "input" => TaskKeyword::Input,
            "inputs" => TaskKeyword::Inputs,
            "inputsmap" | "im" => TaskKeyword::InputsMap,
            "output" => TaskKeyword::Output,
            "outputs" => TaskKeyword::Outputs,
            "outputsmap" | "om" => TaskKeyword::OutputsMap,
            "nodes" => TaskKeyword::Nodes,
            "nodesmap" | "nm" => TaskKeyword::NodesMap,
            "networks" | "nets" => TaskKeyword::Networks,
            "networksmap" | "netm" => TaskKeyword::NetworksMap,
            "root" => TaskKeyword::Root,
            "roots" => TaskKeyword::Roots,
            "rootsmap" | "rm" => TaskKeyword::RootsMap,
            "leaves" => TaskKeyword::Leaves,
            "leavesmap" | "lm" => TaskKeyword::LeavesMap,
            "if" => TaskKeyword::If,
            "else" => TaskKeyword::Else,
            "while" => TaskKeyword::While,
            "try" => TaskKeyword::Try,
            "catch" => TaskKeyword::Catch,
            "in" => TaskKeyword::In,
            "match" => TaskKeyword::Match,
            "hook" => TaskKeyword::Hook,
            "loc" | "local" => TaskKeyword::Local,
            "struct" => TaskKeyword::Struct,
            "function" | "func" => TaskKeyword::Function,
            "return" => TaskKeyword::Return,
            "break" => TaskKeyword::Break,
            "continue" => TaskKeyword::Continue,
            "error" => TaskKeyword::Error,
            "map" => TaskKeyword::Map,
            "attrs" => TaskKeyword::Attrs,
            "loop" => TaskKeyword::Loop,
            "progress" | "prog" => TaskKeyword::Progress,
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
                TaskKeyword::Clear => "clear",
                TaskKeyword::Import => "import",
                TaskKeyword::Exec => "exec",
                TaskKeyword::From => "from",
                TaskKeyword::Node => "node",
                TaskKeyword::Network => "network",
                TaskKeyword::Env => "env",
                TaskKeyword::Exit => "exit",
                TaskKeyword::End => "end",
                TaskKeyword::Help => "help",
                TaskKeyword::Input => "input",
                TaskKeyword::Inputs => "inputs",
                TaskKeyword::InputsMap => "inputsmap",
                TaskKeyword::Output => "output",
                TaskKeyword::Outputs => "outputs",
                TaskKeyword::OutputsMap => "outputsmap",
                TaskKeyword::Nodes => "nodes",
                TaskKeyword::NodesMap => "nodesmap",
                TaskKeyword::Networks => "networks",
                TaskKeyword::NetworksMap => "networksmap",
                TaskKeyword::Root => "root",
                TaskKeyword::Roots => "roots",
                TaskKeyword::RootsMap => "rootsmap",
                TaskKeyword::Leaves => "leaves",
                TaskKeyword::LeavesMap => "leavesmap",
                TaskKeyword::If => "if",
                TaskKeyword::Else => "else",
                TaskKeyword::While => "while",
                TaskKeyword::Try => "try",
                TaskKeyword::Catch => "catch",
                TaskKeyword::In => "in",
                TaskKeyword::Match => "match",
                TaskKeyword::Hook => "hook",
                TaskKeyword::Local => "local",
                TaskKeyword::Struct => "struct",
                TaskKeyword::Function => "function",
                TaskKeyword::Return => "return",
                TaskKeyword::Break => "break",
                TaskKeyword::Continue => "continue",
                TaskKeyword::Error => "error",
                TaskKeyword::Map => "map",
                TaskKeyword::Attrs => "attrs",
                TaskKeyword::Loop => "loop",
                TaskKeyword::Progress => "progress",
                TaskKeyword::For => "for",
            }
        )
    }
}

impl TaskKeyword {
    #[cfg(not(tarpaulin_include))]
    pub fn help(&self) -> String {
        match self {
            TaskKeyword::Clear => "clear the context",
            TaskKeyword::Import => "import a plugin or tasks file",
            TaskKeyword::Exec => "exec a tasks file",
            TaskKeyword::From => "import/exec from a path",
            TaskKeyword::Node => "node function",
            TaskKeyword::Network => "network function",
            TaskKeyword::Env => "environmental variables",
            TaskKeyword::Exit => "exit",
            TaskKeyword::End => "End the tasks file here (discard everything else)",
            TaskKeyword::Help => "help",
            TaskKeyword::Input => "single input of the current node; err if zero or multiple",
            TaskKeyword::Inputs => "inputs of the current node",
            TaskKeyword::InputsMap => "map of inputs of the current node",
            TaskKeyword::Output => "single output of the current node, err if zero or multiple",
            TaskKeyword::Outputs => "outputs of the current node",
            TaskKeyword::OutputsMap => "map of outputs of the current node",
            TaskKeyword::Nodes => "all the nodes in the network",
            TaskKeyword::NodesMap => "map of all the nodes in the network",
            TaskKeyword::Networks => "all the networks in the context",
            TaskKeyword::NetworksMap => "map of all the networks in the context",
            TaskKeyword::Root => "root node of the network (if single outlet)",
            TaskKeyword::Roots => "root nodes of the network",
            TaskKeyword::RootsMap => "map of root nodes of the network",
            TaskKeyword::Leaves => "leaf nodes of the network",
            TaskKeyword::LeavesMap => "map of leaf nodes of the network",
            TaskKeyword::If => "if part of if-else block",
            TaskKeyword::Else => "else part of if-else block",
            TaskKeyword::While => "while loop",
            TaskKeyword::Try => "try statement to contain tasks",
            TaskKeyword::Catch => "catch statement when error occurs on try block",
            TaskKeyword::In => "Check if value is in an array/table",
            TaskKeyword::Match => "match regex pattern with strings",
            TaskKeyword::Hook => "hook tasks to run at each execution",
            TaskKeyword::Local => "Local; similar to environment but within current locale",
            TaskKeyword::Struct => "struct definition",
            TaskKeyword::Function => "function definition",
            TaskKeyword::Return => "return statement inside function",
            TaskKeyword::Break => "return statement inside a loop",
            TaskKeyword::Continue => "continue to next loop",
            TaskKeyword::Error => "raises an error while evaluating",
            TaskKeyword::Map => "map array to a function",
            TaskKeyword::Attrs => "attrs of a node or network",
            TaskKeyword::Loop => "a generic loop",
            TaskKeyword::Progress => "report progress",
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
            "for", "roots", "leaves"
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
