use crate::expressions::{EvalError, EvalErrorType, Expression, SeriesExpression, TaskPosition};
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

// /// Result of a Task when executed
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
);

/// Message that can be sent from the task
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
            #[cfg(feature = "parser")]
            Task::Import(imp) => {
                if let Some(path) = imp.path() {
                    let txt = std::fs::read_to_string(path).unwrap();
                    let tokens = crate::parser::tokenizer::get_tokens(&txt);
                    let tasks = crate::parser::tasks::parse(tokens)
                        .map_err(|e| EvalErrorType::ParseError(e.to_string()).no_pos())?;
                    if imp.tasks {
                        for fc in tasks {
                            self.execute(fc)?;
                        }
                    } else {
                        for mut fc in tasks {
                            let mut exec = false;
                            if let Task::Function(fc) = &mut fc {
                                if let Some(name) = &mut fc.name {
                                    *name = format!("{}.{}", imp.name, name);
                                    exec = true;
                                }
                            }
                            if exec {
                                self.execute(fc)?;
                            }
                        }
                    }
                } else {
                    // In this case look at the available plugins and
                    // load the functions from there to this context.
                    todo!()
                }
                Ok(None)
            }
            Task::Expr(expr) => expr
                .resolve_eval(&FunctionType::Env, self, None, None)
                .map(|a| a.map(|a| self.show_attr(&a, 0))),
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
                        let total = ct.iftrue.len();
                        for (p, task) in ct.iftrue.into_iter().enumerate() {
                            let _ = self.channel.send(TaskMessage::Progress(
                                task.to_string(),
                                p,
                                total,
                            ));
                            if let Some(a) = self.execute(task.clone())? {
                                let _ = self.channel.send(TaskMessage::Info(a));
                            }
                        }
                    }
                    false => {
                        let total = ct.iffalse.len();
                        for (p, task) in ct.iffalse.into_iter().enumerate() {
                            let _ = self.channel.send(TaskMessage::Progress(
                                task.to_string(),
                                p,
                                total,
                            ));
                            if let Some(a) = self.execute(task.clone())? {
                                let _ = self.channel.send(TaskMessage::Info(a));
                            }
                        }
                    }
                }
                Ok(None)
            }
            Task::WhileLoop(lt) => {
                let max_iter = TaskCtxConsts::max_iterations(self);
                let mut progress = 0;
                let mut exit = false;
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
                        false => {
                            exit = true;
                            break;
                        }
                    }
                }
                if exit {
                    Ok(None)
                } else {
                    Err(EvalErrorType::MaxIteratorError(max_iter).pos(lt.position()))
                }
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
                            .unwrap_or("<None>".to_string())
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
                        // assert the type if explicitely provided
                        if let Some(ty) = &attr.1 {
                            if !a.is_type(&ty) {
                                if task.attr.is_some() {
                                    return Err(EvalErrorType::InvalidAttributeType(
                                        ty.clone(),
                                        a.dtype(),
                                    )
                                    .no_pos());
                                }
                            }
                        }
                        if let Some(old) = self
                            .env
                            .set_attr_nested(&task.attr_pre, &attr.0, a.clone())
                            .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?
                        {
                            if task.silent {
                                Ok(None)
                            } else {
                                Ok(Some(format!(
                                    "{} -> {}",
                                    self.show_attr(&old, 0),
                                    self.show_attr(&a, 0)
                                )))
                            }
                        } else {
                            Ok(None)
                        }
                    } else if task.silent {
                        Ok(None)
                    } else {
                        Ok(Some(self.show_attr(&a, 0)))
                    }
                }
                None => Ok(None),
            },
            FunctionType::Node => {
                // this is the only task that needs parallization,
                let parallize = TaskCtxConsts::parallize_nodes(self)
                    & match task.propagation {
                        // if a propagation order is given it needs to be run at that order
                        Some(ref p) => matches!(p.order, PropOrder::Auto),
                        None => true,
                    };
                if parallize {
                    // Implementation not possible because we call
                    // functions from loaded .so files, that are not
                    // thread safe
                    // Err(EvalErrorType::LogicalError(
                    //     "Parallel Execution not supported at the moment",
                    // )
                    // .at(&task))
                    self.run_nodes_task_parallel(task)
                } else {
                    self.run_nodes_task(task)
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
                            // assert the type if explicitely provided
                            if let Some(ty) = &attr.1 {
                                if !a.is_type(&ty) {
                                    if task.attr.is_some() {
                                        return Err(EvalErrorType::InvalidAttributeType(
                                            ty.clone(),
                                            a.dtype(),
                                        )
                                        .no_pos());
                                    }
                                }
                            }
                            if let Some(old) = self
                                .network
                                .set_attr_nested(&task.attr_pre, &attr.0, a.clone())
                                .map_err(|e| EvalErrorType::AttributeError(e).no_pos())?
                            {
                                if task.silent {
                                    Ok(None)
                                } else {
                                    Ok(Some(format!(
                                        "{} -> {}",
                                        self.show_attr(&old, 0),
                                        self.show_attr(&a, 0)
                                    )))
                                }
                            } else {
                                Ok(None)
                            }
                        } else if task.silent {
                            Ok(None)
                        } else {
                            Ok(Some(self.show_attr(&a, 0)))
                        }
                    }
                    None => Ok(None),
                }
            }
        }
    }

    fn run_nodes_task(&mut self, task: EvalTask) -> Result<Option<String>, EvalError> {
        let nodes = self.propagation(task.propagation.unwrap_or_default())?;
        let total = nodes.len();
        let max_nodes_len = TaskCtxConsts::max_nodes_length(self);
        let trunc = total > max_nodes_len;
        let mut progress = 0;
        let mut attrs = Vec::with_capacity(total);
        for n in nodes {
            let name = n
                .try_lock()
                .into_option()
                .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(task.start))?
                .name()
                .to_string();
            let res = match task
                .input
                // add node name to this error
                .resolve_eval_mut(&FunctionType::Node, self, None, Some(&n))
                .map_err(|e| e.pos(task.start).node(name.clone()))?
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
            let _ = self
                .channel
                .send(TaskMessage::Progress(name.clone(), progress, total));
            // because we did progress +=1 above we need <=
            let update = !task.silent & (progress <= max_nodes_len);
            if let Some(attr) = &task.attr {
                // assert the type if explicitely provided
                if let Some(ty) = &attr.1 {
                    if !res.is_type(&ty) {
                        if task.attr.is_some() {
                            return Err(EvalErrorType::InvalidAttributeType(
                                ty.clone(),
                                res.dtype(),
                            )
                            .no_pos()
                            .node(name));
                        }
                        continue;
                    }
                }
                let old = n
                    .set_attr_nested(&task.attr_pre, &attr.0, res.clone())
                    .map_err(|e| EvalErrorType::AttributeError(e).no_pos().node(name.clone()))?;
                if update {
                    if let Some(o) = old {
                        attrs.push(format!(
                            "  {} = {} -> {}",
                            name,
                            self.show_attr(&o, 0),
                            self.show_attr(&res, 0)
                        ));
                    }
                }
            } else if update {
                attrs.push(format!("  {} = {}", name, self.show_attr(&res, 0)));
            }
        }
        if task.silent || attrs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(format!(
                "{{\n{}\n{}}}",
                attrs.join(",\n"),
                if trunc { "...truncated\n" } else { "" }
            )))
        }
    }

    // // Can not compile because of the sabi_trait object not being Send, idk if it can be fixed
    fn run_nodes_task_parallel(&mut self, task: EvalTask) -> Result<Option<String>, EvalError> {
        let nodes = self.propagation(task.propagation.unwrap_or_default())?;
        let total = nodes.len();
        let expressions: Arc<Mutex<Vec<(String, Node, Expression)>>> = Arc::new(Mutex::new(
            nodes
                .into_iter()
                .map(|n| {
                    let name = n
                        .try_lock()
                        .into_option()
                        .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(task.start))?
                        .name()
                        .to_string();
                    task.input
                        // add node name to this error
                        .resolve(&FunctionType::Node, self, None, Some(&n))
                        .map_err(|e| e.pos(task.start).node(name.clone()))
                        .map(|e| (name, n, e))
                })
                .collect::<Result<Vec<(String, Node, Expression)>, EvalError>>()?,
        ));

        #[allow(clippy::type_complexity)]
        let (tx, rx): (
            Sender<(String, Result<Option<Attribute>, EvalError>)>,
            Receiver<(String, Result<Option<Attribute>, EvalError>)>,
        ) = channel();

        let mut attrs = Vec::with_capacity(total);
        let max_nodes_len = TaskCtxConsts::max_nodes_length(self);
        let trunc = total > max_nodes_len;
        thread::scope(|s| -> Result<(), EvalError> {
            let cores = TaskCtxConsts::parallel_cores(self);
            // just to make it work for now
            let mut children = Vec::with_capacity(cores);
            let tctx = Arc::new(&*self);
            for _ in 0..cores {
                let ctx = tx.clone();
                let expr_lst = expressions.clone();
                let tc = tctx.clone();
                let child = s.spawn(move || -> Result<(), anyhow::Error> {
                    loop {
                        let expr = expr_lst
                            .lock()
                            .map_err(|e| anyhow::Error::msg(e.to_string()))?
                            .pop();
                        if let Some((name, n, expr)) = expr {
                            let res = expr.eval(&FunctionType::Node, &tc, None, Some(&n));
                            ctx.send((name, res))?
                        } else {
                            break;
                        }
                    }
                    Ok::<(), anyhow::Error>(())
                });

                children.push(child);
            }
            // since we cloned it, only the cloned ones are dropped when
            // the thread ends
            drop(tx);

            let mut progress = 0;
            for (name, res) in rx {
                let res = match res {
                    Ok(Some(r)) => r,
                    Ok(None) => {
                        progress += 1;
                        if task.attr.is_some() {
                            // remove them from the queue (might have extra computations)
                            expressions.lock().unwrap().clear();
                            return Err(EvalErrorType::NoReturnValue(
                                "input expression".to_string(),
                            )
                            .no_pos()
                            .node(name));
                        }
                        continue;
                    }
                    Err(e) => {
                        // remove them from the queue (might have extra computations)
                        expressions.lock().unwrap().clear();
                        return Err(e);
                    }
                };
                let node = self
                    .network
                    .node_by_name(&name)
                    .expect("Should have this node in the network")
                    .clone();
                let mut n = node
                    .try_lock()
                    .into_option()
                    .ok_or(EvalErrorType::MutexError(file!(), line!()).pos(task.start))?;
                progress += 1;
                let _ = self
                    .channel
                    .send(TaskMessage::Progress(name.clone(), progress, total));
                // because we did progress +=1 above we need <=
                let update = !task.silent & (progress <= max_nodes_len);
                if let Some(attr) = &task.attr {
                    // assert the type if explicitely provided
                    if let Some(ty) = &attr.1 {
                        if !res.is_type(&ty) {
                            if task.attr.is_some() {
                                // remove them from the queue (might have extra computations)
                                expressions.lock().unwrap().clear();
                                return Err(EvalErrorType::InvalidAttributeType(
                                    ty.clone(),
                                    res.dtype(),
                                )
                                .no_pos()
                                .node(name));
                            }
                            continue;
                        }
                    }
                    let old = n
                        .set_attr_nested(&task.attr_pre, &attr.0, res.clone())
                        .map_err(|e| {
                            EvalErrorType::AttributeError(e).no_pos().node(name.clone())
                        })?;
                    if update {
                        if let Some(o) = old {
                            attrs.push(format!(
                                "  {} = {} -> {}",
                                name,
                                self.show_attr(&o, 0),
                                self.show_attr(&res, 0)
                            ));
                        }
                    }
                } else if update {
                    attrs.push(format!("  {} = {}", name, self.show_attr(&res, 0)));
                }
            }
            // by this time all threads should be complete (otherwise the loop does not end)
            for child in children {
                let _ = child.join();
            }
            Ok(())
        })?;
        if task.silent || attrs.is_empty() {
            Ok(None)
        } else {
            Ok(Some(format!(
                "{{\n{}\n{}}}",
                attrs.join(",\n"),
                if trunc { "...truncated\n" } else { "" }
            )))
        }
    }

    /// evaluate an attribute task
    pub fn attr_task(&self, task: AttrTask) -> Result<String, EvalError> {
        match task.ty {
            FunctionType::Env => self
                .env
                .attr_nested(&task.attr_pre, &task.attr)
                .map_err(|e| EvalErrorType::AttributeError(e).pos(task.position()))?
                .map(|a| self.show_attr(a, 0))
                .ok_or(EvalErrorType::AttributeNotFound.pos(task.position())),
            FunctionType::Node => {
                let nodes = self.propagation(task.propagation.clone().unwrap_or_default())?;
                let max_nodes_len = TaskCtxConsts::max_nodes_length(self);
                let trunc = nodes.len() > max_nodes_len;
                let attrs = nodes
                    .iter()
                    .take(max_nodes_len)
                    .map(|n| {
                        let n = n.lock();
                        let name = n.name().to_string();
                        Ok(format!(
                            "  {} = {}",
                            name,
                            if let Some(a) =
                                n.attr_nested(&task.attr_pre, &task.attr).map_err(|e| {
                                    EvalErrorType::AttributeError(e)
                                        .pos(task.position())
                                        .node(name.clone())
                                })?
                            {
                                self.show_attr(a, 0)
                            } else {
                                "<None>".to_string()
                            }
                        ))
                    })
                    .collect::<Result<Vec<String>, EvalError>>()?;
                Ok(format!(
                    "{{\n{}\n{}}}",
                    attrs.join(",\n"),
                    if trunc { "...truncated\n" } else { "" }
                ))
            }
            FunctionType::Network => self
                .network
                .attr_nested(&task.attr_pre, &task.attr)
                .map_err(|e| EvalErrorType::AttributeError(e).pos(task.position()))?
                .map(|a| self.show_attr(a, 0))
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
    pub attr: Option<(String, Option<NadiAttrType>)>,
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
                ".{}{} =",
                self.attr_pre
                    .iter()
                    .map(|s| s.as_str())
                    .chain([attr.0.as_str()])
                    .collect::<Vec<&str>>()
                    .join("."),
                if let Some(ty) = &attr.1 {
                    format!(": {ty}")
                } else {
                    "".to_string()
                }
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
            self.input,
            if self.silent { ";" } else { Default::default() }
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
            write!(f, "if ({}) {{\n\t{}\n}}", self.cond, tasks,)
        } else {
            write!(
                f,
                "if ({}) {{\n\t{}\n}} else {{\n\t{}\n}}",
                self.cond,
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
            self.cond,
            self.tasks
                .iter()
                .map(|p| p.to_string())
                .collect::<Vec<String>>()
                .join("\n"),
        )
    }
}

// /// Functions to import
// ///
// /// None will import the plugin but functions need dot syntax
// /// All will import all functions to be used directly
// /// Some will only import the listed functions to be used directly
// #[derive(Clone, PartialEq, Debug)]
// pub enum ImportFunctions {
//     None,
//     All,
//     Some(Vec<String>),
// }

#[cfg(feature = "parser")]
/// Task that is an import statement
#[derive(Clone, PartialEq, Debug)]
pub struct ImportTask {
    /// name of the plugin/nadi file
    pub name: String,
    /// path to the plugin/nadi code
    pub path: Option<PathBuf>,
    // /// Functions to import
    // functions: ImportFunctions,
    /// Execute tasks while importing functions
    pub tasks: bool,
}

#[cfg(feature = "parser")]
impl std::fmt::Display for ImportTask {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let key = if self.tasks { "exec" } else { "import" };
        if let Some(p) = &self.path {
            write!(f, "{key} {} from {p:?}", self.name)
        } else {
            write!(f, "{key} {}", self.name)
        }
    }
}

#[cfg(feature = "parser")]
impl ImportTask {
    pub fn path(&self) -> Option<PathBuf> {
        if let Some(p) = &self.path {
            return Some(p.clone());
        }
        let name = &self.name;
        let path = PathBuf::from(format!("{name}.tasks"));
        if path.exists() {
            return Some(path);
        }
        None
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
    /// Evaluate the expression
    Expr(Expression),
    #[cfg(feature = "parser")]
    /// Import functions from a tasks file
    Import(ImportTask),
    /// Get a series/timeseries from the node/network/env
    GetSeries(GetSeriesTask),
    /// Set a series/timeseries to the node/network/env
    SetSeries(SetSeriesTask),
    /// Clear the task context
    Clear,
    /// exit the task system/process,
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
            Task::Expr(_) => false,
            #[cfg(feature = "parser")]
            Task::Import(_) => true,
            Task::GetSeries(_) => false,
            Task::SetSeries(_) => true,
            Task::Clear => false,
            Task::Exit => false,
        }
    }

    /// Message for the current task's functionality
    pub fn message(&self) -> &'static str {
        match self {
            Task::Eval(_) => "Evaluate the expression",
            Task::Attr(_) => "Query the variable",
            Task::Conditional(_) => "Conditional evaluation of tasks",
            Task::WhileLoop(_) => "Repeat the tasks while the condition is true",
            Task::Hook(_) => "Evaluate these tasks after each eval task",
            Task::Help(_, _) => "Show help",
            Task::Function(_) => "Define a new user function",
            Task::Expr(_) => "Evaluate expression",
            #[cfg(feature = "parser")]
            Task::Import(_) => "Import functions from the file",
            Task::GetSeries(_) => "Query Series/TimeSeries values",
            Task::SetSeries(_) => "Set Series/TimeSeries values",
            Task::Clear => "Clear the task context",
            Task::Exit => "Exit the program",
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
            Task::Expr(expr) => write!(f, "{expr}"),
            #[cfg(feature = "parser")]
            Task::Import(imp) => write!(f, "{imp}"),
            Task::GetSeries(gst) => write!(f, "{gst}"),
            Task::SetSeries(sst) => write!(f, "{sst}"),
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
    Inputs,
    Output,
    Nodes,
    Root,
    Outlets,
    Leaves,
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
    Error,
    For,
    // reserved
    Map,
    Attrs,
    Loop,
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
            "inputs" => TaskKeyword::Inputs,
            "output" => TaskKeyword::Output,
            "nodes" => TaskKeyword::Nodes,
            "root" => TaskKeyword::Root,
            "outlets" => TaskKeyword::Outlets,
            "leaves" => TaskKeyword::Leaves,
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
                TaskKeyword::Inputs => "inputs",
                TaskKeyword::Output => "output",
                TaskKeyword::Nodes => "nodes",
                TaskKeyword::Root => "root",
                TaskKeyword::Outlets => "outlets",
                TaskKeyword::Leaves => "leaves",
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
            TaskKeyword::Inputs => "inputs of the current node",
            TaskKeyword::Output => "output of the current node",
            TaskKeyword::Nodes => "all the nodes in the network",
            TaskKeyword::Root => "root node of the network (if single outlet)",
            TaskKeyword::Outlets => "outlet nodes of the network",
            TaskKeyword::Leaves => "leaf nodes of the network",
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
            "for", "outlets", "leaves"
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
