#![allow(clippy::module_inception)]
use crate::attrs::{AttrMap, AttrSlice};
use crate::network::StrPath;
use crate::plugins::{load_library_safe, NadiPlugin};
use crate::prelude::*;
use crate::table::{contents_2_md, ColumnAlign};
use abi_stable::std_types::Tuple2;
use abi_stable::{
    sabi_trait,
    std_types::{
        map::REntry,
        RBox, RErr, RHashMap, ROk,
        ROption::{self, RNone, RSome},
        RResult, RString, RVec,
    },
    StableAbi,
};
use colored::Colorize;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Return values for Nadi Functions
#[repr(C)]
#[derive(StableAbi, Default)]
pub enum FunctionRet {
    // a catch rewind system might be nice to catch panic from plugins
    // Panic,
    #[default]
    None,
    Some(Attribute),
    Error(RString),
}

impl FunctionRet {
    pub fn res(self) -> Result<Option<Attribute>, String> {
        match self {
            Self::None => Ok(None),
            Self::Some(a) => Ok(Some(a)),
            Self::Error(e) => Err(e.to_string()),
        }
    }
}

impl From<()> for FunctionRet {
    fn from(_value: ()) -> Self {
        Self::None
    }
}

impl<T> From<T> for FunctionRet
where
    Attribute: From<T>,
{
    fn from(value: T) -> Self {
        Self::Some(Attribute::from(value))
    }
}

// Unstable feature where we could define how `?` can be used for generating FunctionRet
// impl<T> std::ops::FromResidual<Result<T, std::io::Error>> for FunctionRet
// where
//     Attribute: From<T>,
// {
//     fn from_residual(residual: Result<T, std::io::Error>) -> Self {
//         match residual {
//             Ok(v) => Self::Some(Attribute::from(v)),
//             Err(e) => Self::Error(RString::from(e.to_string())),
//         }
//     }
// }

impl From<std::io::Error> for FunctionRet {
    fn from(value: std::io::Error) -> Self {
        Self::Error(RString::from(value.to_string()))
    }
}

impl<T> From<Option<T>> for FunctionRet
where
    FunctionRet: From<T>,
{
    fn from(value: Option<T>) -> Self {
        match value {
            Some(v) => Self::from(v),
            None => Self::None,
        }
    }
}

impl<T> From<ROption<T>> for FunctionRet
where
    FunctionRet: From<T>,
{
    fn from(value: ROption<T>) -> Self {
        match value {
            RSome(v) => Self::from(v),
            RNone => Self::None,
        }
    }
}

impl<T, S> From<RResult<T, S>> for FunctionRet
where
    FunctionRet: From<T>,
    RString: From<S>,
{
    fn from(value: RResult<T, S>) -> Self {
        match value {
            ROk(v) => Self::from(v),
            RErr(e) => Self::Error(RString::from(e)),
        }
    }
}

// Since we have String error message, anything that can be string can
// be used. It should cover anything that impl Error as well
impl<T, S> From<Result<T, S>> for FunctionRet
where
    FunctionRet: From<T>,
    S: ToString,
{
    fn from(value: Result<T, S>) -> Self {
        match value {
            Ok(v) => Self::from(v),
            Err(e) => Self::Error(RString::from(e.to_string())),
        }
    }
}

#[repr(C)]
#[derive(StableAbi)]
pub struct FuncArg {
    pub name: RString,
    pub ty: RString,
    pub help: RString,
    pub category: FuncArgType,
}

impl ToString for FuncArg {
    fn to_string(&self) -> String {
        match &self.category {
            FuncArgType::Arg => format!("{}: '{}'", self.name, self.ty),
            FuncArgType::OptArg => format!("{}: '{}'", self.name, self.ty),
            FuncArgType::DefArg(val) => format!("{}: '{}' = {}", self.name, self.ty, val),
            FuncArgType::Args => format!("*{}", self.name),
            FuncArgType::KwArgs => format!("**{}", self.name),
        }
    }
}
impl FuncArg {
    fn to_colored_string(&self) -> String {
        match &self.category {
            FuncArgType::Arg => format!("{}: '{}'", self.name, self.ty.green()),
            FuncArgType::OptArg => format!("{}: '{}'", self.name, self.ty.green()),
            FuncArgType::DefArg(val) => {
                format!("{}: '{}' = {}", self.name, self.ty.green(), val.yellow())
            }
            FuncArgType::Args => format!("*{}", self.name.red()),
            FuncArgType::KwArgs => format!("**{}", self.name.red()),
        }
    }
}

#[repr(C)]
#[derive(StableAbi)]
pub enum FuncArgType {
    Arg,
    OptArg,
    DefArg(RString),
    Args,
    KwArgs,
}

/// Environmental functions that can be applied anywhere
#[sabi_trait]
pub trait EnvFunction: Debug + Clone {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    fn short_help(&self) -> RString {
        self.help()
            .trim()
            .split('\n')
            .next()
            .unwrap_or("No Help")
            .into()
    }
    fn args(&self) -> RVec<FuncArg>;
    fn signature(&self) -> RString {
        self.args()
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join(", ")
            .into()
    }
    fn code(&self) -> RString;
    fn call(&self, ctx: &FunctionCtx) -> FunctionRet;
}

#[sabi_trait]
pub trait NodeFunction: Debug + Clone {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    fn short_help(&self) -> RString {
        self.help()
            .trim()
            .split('\n')
            .next()
            .unwrap_or("No Help")
            .into()
    }
    fn args(&self) -> RVec<FuncArg>;
    fn signature(&self) -> RString {
        self.args()
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join(", ")
            .into()
    }
    fn code(&self) -> RString;
    fn call(&self, obj: &mut NodeInner, ctx: &FunctionCtx) -> FunctionRet;
}

// can't use generics because of sabi_trait
#[sabi_trait]
pub trait NetworkFunction: Debug + Clone {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    fn short_help(&self) -> RString {
        self.help()
            .trim()
            .split('\n')
            .next()
            .unwrap_or("No Help")
            .into()
    }
    fn args(&self) -> RVec<FuncArg>;
    fn signature(&self) -> RString {
        self.args()
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<String>>()
            .join(", ")
            .into()
    }
    fn code(&self) -> RString;
    fn call(&self, obj: &mut Network, ctx: &FunctionCtx) -> FunctionRet;
}

// A trait object for the `State` Trait Object
pub type EnvFunctionBox = EnvFunction_TO<'static, RBox<()>>;
pub type NodeFunctionBox = NodeFunction_TO<'static, RBox<()>>;
pub type NetworkFunctionBox = NetworkFunction_TO<'static, RBox<()>>;

#[repr(C)]
#[derive(StableAbi, Default)]
pub struct PluginFunctions {
    env: RVec<RString>,
    node: RVec<RString>,
    network: RVec<RString>,
}

impl PluginFunctions {
    pub fn with_network(mut self, func: RString) -> Self {
        self.network.push(func);
        self
    }

    pub fn with_node(mut self, func: RString) -> Self {
        self.node.push(func);
        self
    }

    pub fn with_env(mut self, func: RString) -> Self {
        self.env.push(func);
        self
    }

    pub fn push_network(&mut self, func: RString) {
        self.network.push(func);
    }

    pub fn push_node(&mut self, func: RString) {
        self.node.push(func);
    }

    pub fn push_env(&mut self, func: RString) {
        self.env.push(func);
    }

    pub fn network(&self) -> &RVec<RString> {
        &self.network
    }

    pub fn node(&self) -> &RVec<RString> {
        &self.node
    }

    pub fn env(&self) -> &RVec<RString> {
        &self.env
    }
}

// TODO: add environmental variables, like verbose, progress, debug,
// etc. that all functions can read (passed along with args, kwargs to
// all functions)
#[repr(C)]
#[derive(StableAbi, Default)]
pub struct NadiFunctions {
    env: RHashMap<RString, EnvFunctionBox>,
    env_alias: RHashMap<RString, RString>,
    node: RHashMap<RString, NodeFunctionBox>,
    node_alias: RHashMap<RString, RString>,
    network: RHashMap<RString, NetworkFunctionBox>,
    network_alias: RHashMap<RString, RString>,
    plugins: RHashMap<RString, PluginFunctions>,
}

impl NadiFunctions {
    pub fn new() -> Self {
        let mut funcs = Self::default();

        #[cfg(feature = "functions")]
        crate::internal::register_internal(&mut funcs);

        funcs.load_plugins().unwrap();
        funcs
    }

    pub fn register_network_function(&mut self, prefix: &str, func: NetworkFunctionBox) {
        let name = func.name();
        let fullname = RString::from(format!("{}.{}", prefix, name));
        self.network.insert(fullname.clone(), func);
        if let RSome(oldname) = self.network_alias.insert(name.clone(), fullname.clone()) {
            if fullname != oldname {
                eprintln!(
                    "WARN Function {} now uses {} instead of {}, use full name for disambiguity",
                    name, fullname, oldname
                );
            }
        }
        match self.plugins.entry(prefix.into()) {
            REntry::Occupied(mut o) => o.get_mut().push_network(name),
            REntry::Vacant(v) => {
                v.insert(PluginFunctions::default().with_network(name));
            }
        };
    }
    pub fn register_node_function(&mut self, prefix: &str, func: NodeFunctionBox) {
        let name = func.name();
        let fullname = RString::from(format!("{}.{}", prefix, name));
        self.node.insert(fullname.clone(), func);
        if let RSome(oldname) = self.node_alias.insert(name.clone(), fullname.clone()) {
            if fullname != oldname {
                eprintln!(
                    "WARN Function {} now uses {} instead of {}, use full name for disambiguity",
                    name, fullname, oldname
                );
            }
        }
        match self.plugins.entry(prefix.into()) {
            REntry::Occupied(mut o) => o.get_mut().push_node(name),
            REntry::Vacant(v) => {
                v.insert(PluginFunctions::default().with_node(name));
            }
        };
    }
    pub fn register_env_function(&mut self, prefix: &str, func: EnvFunctionBox) {
        let name = func.name();
        let fullname = RString::from(format!("{}.{}", prefix, name));
        self.env.insert(fullname.clone(), func);
        if let RSome(oldname) = self.env_alias.insert(name.clone(), fullname.clone()) {
            if fullname != oldname {
                eprintln!(
                    "WARN Function {} now uses {} instead of {}, use full name for disambiguity",
                    name, fullname, oldname
                );
            }
        }
        match self.plugins.entry(prefix.into()) {
            REntry::Occupied(mut o) => o.get_mut().push_env(name),
            REntry::Vacant(v) => {
                v.insert(PluginFunctions::default().with_env(name));
            }
        };
    }

    pub fn load_plugins(&mut self) -> anyhow::Result<()> {
        if let Ok(plugin_dirs) = std::env::var("NADI_PLUGIN_DIRS") {
            for pdir in plugin_dirs.split(':') {
                if let Ok(dir) = std::fs::read_dir(pdir) {
                    for path in dir {
                        if let Some(lib) = load_library_safe(&path?.path()) {
                            lib.register(self);
                        }
                    }
                }
            }
        } else {
            eprintln!("WARN: Environmental variable NADI_PLUGIN_DIRS is not set.");
        }
        Ok(())
    }

    pub fn env_functions(&self) -> &RHashMap<RString, EnvFunctionBox> {
        &self.env
    }

    pub fn env_alias(&self) -> &RHashMap<RString, RString> {
        &self.env_alias
    }

    pub fn node_functions(&self) -> &RHashMap<RString, NodeFunctionBox> {
        &self.node
    }

    pub fn node_alias(&self) -> &RHashMap<RString, RString> {
        &self.node_alias
    }

    pub fn network_functions(&self) -> &RHashMap<RString, NetworkFunctionBox> {
        &self.network
    }

    pub fn network_alias(&self) -> &RHashMap<RString, RString> {
        &self.network_alias
    }

    pub fn plugins(&self) -> &RHashMap<RString, PluginFunctions> {
        &self.plugins
    }

    pub fn plugins_doc<P: AsRef<Path>>(&self, outdir: P) -> anyhow::Result<()> {
        let mut doc = BufWriter::new(File::create(outdir.as_ref().join("index.md"))?);
        writeln!(doc, "# All Plugin Functions")?;
        writeln!(
            doc,
            "All the functions available on this instance of nadi, are listed here.\n"
        )?;
        let (node_funcs, net_funcs, env_funcs) = self.list_functions_md(true);

        writeln!(
            doc,
            "## Env Functions\n{}\n\n## Node Functions\n{}\n\n## Network Functions\n{}",
            env_funcs, node_funcs, net_funcs
        )?;

        fn func_sig(fargs: RVec<FuncArg>) -> (String, String) {
            let args: Vec<String> = fargs.iter().map(|s| s.to_string()).collect();
            let args_help: Vec<String> = fargs
                .iter()
                .map(|s| format!("- `{}` => {}", s.to_string(), s.help))
                .collect();
            (
                if args.len() < 3 {
                    format!("({})", args.join(", "))
                } else {
                    format!("(\n    {}\n)", args.join(",\n    "))
                },
                args_help.join("\n"),
            )
        }

        for Tuple2(plug, funcs) in self.plugins() {
            let mut doc = BufWriter::new(File::create(
                outdir.as_ref().join(plug.as_str()).with_extension("md"),
            )?);
            if !funcs.env().is_empty() {
                writeln!(doc, "# Env Functions")?;
                for func in funcs.env() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.env(&fname).expect("Func Should Exist");
                    writeln!(doc, "## {func} {{#env.{func}}}")?;
                    writeln!(doc, "```sig")?;
                    let (s, h) = func_sig(func_obj.args());
                    writeln!(doc, "env {}.{}{}", plug, func, s)?;
                    writeln!(doc, "```\n")?;
                    writeln!(doc, "### Arguments\n{}\n", h)?;
                    writeln!(doc, "{}", func_obj.help().replace("\n#", "\n###"))?;
                }
            }
            if !funcs.node().is_empty() {
                writeln!(doc, "# Node Functions")?;
                for func in funcs.node() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.node(&fname).expect("Func Should Exist");
                    writeln!(doc, "## {func} {{#node.{func}}}")?;
                    writeln!(doc, "```sig")?;
                    let (s, h) = func_sig(func_obj.args());
                    writeln!(doc, "node {}.{}{}", plug, func, s)?;
                    writeln!(doc, "```\n")?;
                    writeln!(doc, "### Arguments\n{}\n", h)?;
                    writeln!(doc, "{}", func_obj.help().replace("\n#", "\n###"))?;
                }
            }
            if !funcs.network().is_empty() {
                writeln!(doc, "# Network Functions")?;
                for func in funcs.network() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.network(&fname).expect("Func Should Exist");
                    writeln!(doc, "## {func} {{#network.{func}}}")?;
                    writeln!(doc, "```sig")?;
                    let (s, h) = func_sig(func_obj.args());
                    writeln!(doc, "network {}.{}{}", plug, func, s)?;
                    writeln!(doc, "```\n")?;
                    writeln!(doc, "### Arguments\n{}\n", h)?;
                    writeln!(doc, "{}", func_obj.help().replace("\n#", "\n###"))?;
                }
            }
        }
        Ok(())
    }

    pub fn list_functions(&self) {
        fn print_func(p: &RString, t: &str, f: &RString, args: RVec<FuncArg>) {
            print!("{} {}.{}", t, p.as_str().red(), f.as_str().blue(),);
            let args: Vec<String> = args.iter().map(|s| s.to_colored_string()).collect();
            if args.len() < 3 {
                println!("({})", args.join(", "));
            } else {
                println!("(\n    {}\n)", args.join(",\n    "));
            }
        }

        for Tuple2(plug, funcs) in self.plugins() {
            if !funcs.node().is_empty() {
                for func in funcs.node() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.node(&fname).expect("Func Should Exist");
                    print_func(plug, "node", func, func_obj.args());
                }
            }
            if !funcs.network().is_empty() {
                for func in funcs.network() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.network(&fname).expect("Func Should Exist");
                    print_func(plug, "network", func, func_obj.args());
                }
            }
        }
    }

    pub fn list_functions_md(&self, link: bool) -> (String, String, String) {
        let mut node_functions = vec![];
        let mut net_functions = vec![];
        let mut env_functions = vec![];
        let fname = if link {
            |p: &str, t: &str, n: &str, h: RString| {
                vec![
                    format!("[`{p}`]({p}.md)"),
                    format!("[`{n}`]({p}.md#{t}.{n})"),
                    h.to_string(),
                ]
            }
        } else {
            |p: &str, _t: &str, n: &str, h: RString| {
                vec![p.to_string(), n.to_string(), h.to_string()]
            }
        };
        for Tuple2(func, fobj) in &self.node {
            let (plug, name) = func.split_once('.').unwrap_or(("null", func.as_str()));
            node_functions.push(fname(plug, "network", name, fobj.short_help()));
        }
        for Tuple2(func, fobj) in &self.network {
            let (plug, name) = func.split_once('.').unwrap_or(("null", func.as_str()));
            net_functions.push(fname(plug, "node", name, fobj.short_help()));
        }
        for Tuple2(func, fobj) in &self.env {
            let (plug, name) = func.split_once('.').unwrap_or(("null", func.as_str()));
            env_functions.push(fname(plug, "env", name, fobj.short_help()));
        }
        (
            contents_2_md(
                &["Plugin", "Function", "Help"],
                &[&ColumnAlign::Left, &ColumnAlign::Left, &ColumnAlign::Left],
                node_functions,
            ),
            contents_2_md(
                &["Plugin", "Function", "Help"],
                &[&ColumnAlign::Left, &ColumnAlign::Left, &ColumnAlign::Left],
                net_functions,
            ),
            contents_2_md(
                &["Plugin", "Function", "Help"],
                &[&ColumnAlign::Left, &ColumnAlign::Left, &ColumnAlign::Left],
                env_functions,
            ),
        )
    }

    // pub fn call_node(
    //     &self,
    //     func: &str,
    //     nodes: RSlice<Node>,
    //     ctx: &FunctionCtx,
    // ) -> anyhow::Result<()> {
    //     match self.node(func) {
    //         Some(f) => f
    //             .call(nodes, ctx)
    //             .map_err(|e| anyhow::Error::msg(e.to_string()))
    //             .into(),
    //         None => anyhow::bail!("Node Function {} not found", func),
    //     }
    // }

    // pub fn call_network(
    //     &self,
    //     func: &str,
    //     network: &mut Network,
    //     ctx: &FunctionCtx,
    // ) -> anyhow::Result<()> {
    //     match self.network(func) {
    //         Some(f) => f
    //             .call(network, ctx)
    //             .res(),
    //         None => anyhow::bail!("Node Function {} not found", func),
    //     }
    // }

    pub fn env(&self, func: &str) -> Option<&EnvFunctionBox> {
        if func.contains('.') {
            self.env.get(func)
        } else {
            self.env_alias.get(func).and_then(|f| self.env.get(f))
        }
    }
    pub fn node(&self, func: &str) -> Option<&NodeFunctionBox> {
        if func.contains('.') {
            self.node.get(func)
        } else {
            self.node_alias.get(func).and_then(|f| self.node.get(f))
        }
    }
    pub fn network(&self, func: &str) -> Option<&NetworkFunctionBox> {
        if func.contains('.') {
            self.network.get(func)
        } else {
            self.network_alias
                .get(func)
                .and_then(|f| self.network.get(f))
        }
    }

    pub fn help(&self, func: &str) -> Option<String> {
        // node and network function might have same name
        self.help_network(func).or_else(|| self.help_node(func))
    }

    pub fn help_node(&self, func: &str) -> Option<String> {
        self.node(func).map(|f| f.help().into_string())
    }

    pub fn help_network(&self, func: &str) -> Option<String> {
        self.network(func).map(|f| f.help().into_string())
    }
    pub fn code(&self, func: &str) -> Option<String> {
        // node and network function might have same name
        self.code_network(func).or_else(|| self.code_node(func))
    }

    pub fn code_node(&self, func: &str) -> Option<String> {
        self.node(func).map(|f| f.code().into_string())
    }

    pub fn code_network(&self, func: &str) -> Option<String> {
        self.network(func).map(|f| f.code().into_string())
    }
}

#[repr(C)]
#[derive(StableAbi, Default, Debug, PartialEq)]
pub struct FunctionCtx {
    pub args: RVec<Attribute>,
    pub kwargs: AttrMap,
    pub propagation: Propagation,
}

impl FunctionCtx {
    pub fn from_arg_kwarg(args: Vec<Attribute>, kwargs: HashMap<String, Attribute>) -> Self {
        let args = RVec::from(args);
        let kwargs = kwargs
            .into_iter()
            .map(|(k, v)| (RString::from(k), v))
            .collect();
        Self {
            args,
            kwargs,
            propagation: Propagation::default(),
        }
    }

    pub fn propagation(&self) -> &Propagation {
        &self.propagation
    }

    pub fn args(&self) -> AttrSlice {
        self.args.as_rslice()
    }

    pub fn arg(&self, ind: usize) -> Option<&Attribute> {
        self.args.get(ind)
    }

    pub fn kwargs(&self) -> &AttrMap {
        &self.kwargs
    }

    pub fn kwarg(&self, name: &str) -> Option<&Attribute> {
        self.kwargs.get(name)
    }

    pub fn arg_kwarg<P: FromAttribute>(&self, ind: usize, name: &str) -> Option<Result<P, String>> {
        self.kwarg(name).or_else(|| self.arg(ind)).map(|arg| {
            match FromAttribute::try_from_attr(arg) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!(
                    "Argument {} ({} [{}]): {e}",
                    ind + 1,
                    name,
                    nadi_core::attrs::type_name::<P>()
                )),
            }
        })
    }

    pub fn arg_kwarg_relaxed<P: FromAttributeRelaxed>(
        &self,
        ind: usize,
        name: &str,
    ) -> Option<Result<P, String>> {
        self.kwarg(name).or_else(|| self.arg(ind)).map(|arg| {
            match FromAttributeRelaxed::try_from_attr_relaxed(arg) {
                Ok(v) => Ok(v),
                Err(e) => Err(format!(
                    "Argument {} ({} [{}]): {e}",
                    ind + 1,
                    name,
                    nadi_core::attrs::type_name::<P>()
                )),
            }
        })
    }
}

// TODO maybe add attr = "smth"; attr > 1.0 etc as conditions
// Maybe we can't because attribute name can be string or variable for now
#[repr(C)]
#[derive(StableAbi, Debug, Clone, PartialEq)]
pub enum Condition {
    Variable(RString),
    Literal(Attribute),
    Eq(RString, RBox<Condition>),
    Gt(RString, RBox<Condition>),
    Lt(RString, RBox<Condition>),
    Not(RBox<Condition>),
    And(RBox<Condition>, RBox<Condition>),
    Or(RBox<Condition>, RBox<Condition>),
}

fn partial_ord(
    node: &NodeInner,
    var: &RString,
    cond: &Condition,
    strict: bool,
) -> Result<Option<std::cmp::Ordering>, String> {
    let a = match node.attr_dot(var)? {
        Some(v) => v,
        None => return Ok(None),
    };
    let b = match cond {
        Condition::Variable(v) => match node.attr_dot(v)? {
            Some(v) => v,
            None => return Ok(None),
        },
        Condition::Literal(v) => v,
        _ => return Err(String::from("Comparision of conditions not supported")),
    };
    if strict {
        let (x, y) = (a.type_name(), b.type_name());
        if x != y {
            return Err(format!("Cannot compare {x} with {y}"));
        }
    }
    Ok(a.partial_cmp(b))
}

impl NodeInner {
    /// check if the condition is true
    pub fn check(&self, cond: &Condition) -> bool {
        match cond {
            Condition::Eq(var, val) => match partial_ord(self, var, val, false) {
                Ok(Some(x)) => x.is_eq(),
                _ => false,
            },
            Condition::Gt(var, val) => match partial_ord(self, var, val, false) {
                Ok(Some(x)) => x.is_gt(),
                _ => false,
            },
            Condition::Lt(var, val) => match partial_ord(self, var, val, false) {
                Ok(Some(x)) => x.is_lt(),
                _ => false,
            },
            Condition::Variable(v) => self.try_attr_relaxed(v.as_str()).unwrap_or(false),
            Condition::Literal(v) => bool::from_attr_relaxed(v).unwrap_or(false),
            Condition::Not(v) => !self.check(v),
            Condition::And(a, b) => self.check(a) & self.check(b),
            Condition::Or(a, b) => self.check(a) | self.check(b),
        }
    }
    /// check if condition is true only if attributes exist
    pub fn check_strict(&self, cond: &Condition) -> Result<bool, String> {
        match cond {
            Condition::Eq(var, val) => match partial_ord(self, var, val, false)? {
                Some(x) => Ok(x.is_eq()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Gt(var, val) => match partial_ord(self, var, val, false)? {
                Some(x) => Ok(x.is_gt()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Lt(var, val) => match partial_ord(self, var, val, false)? {
                Some(x) => Ok(x.is_lt()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Variable(v) => self.try_attr_relaxed(v.as_str()),
            Condition::Literal(v) => bool::try_from_attr_relaxed(v),
            Condition::Not(v) => self.check_strict(v).map(|b| !b),
            Condition::And(a, b) => {
                let a = self.check_strict(a)?;
                let b = self.check_strict(b)?;
                Ok(a & b)
            }
            Condition::Or(a, b) => {
                let a = self.check_strict(a)?;
                let b = self.check_strict(b)?;
                Ok(a | b)
            }
        }
    }
    /// check if condition is true only if attributes are bool
    pub fn check_super_strict(&self, cond: &Condition) -> Result<bool, String> {
        match cond {
            Condition::Eq(var, val) => match partial_ord(self, var, val, true)? {
                Some(x) => Ok(x.is_eq()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Gt(var, val) => match partial_ord(self, var, val, true)? {
                Some(x) => Ok(x.is_gt()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Lt(var, val) => match partial_ord(self, var, val, true)? {
                Some(x) => Ok(x.is_lt()),
                _ => Err(format!("Attrbute {var} doesn't exist")),
            },
            Condition::Variable(v) => self.try_attr(v.as_str()),
            Condition::Literal(v) => bool::try_from_attr(v),
            Condition::Not(v) => self.check_super_strict(v).map(|b| !b),
            Condition::And(a, b) => {
                let a = self.check_super_strict(a)?;
                let b = self.check_super_strict(b)?;
                Ok(a && b)
            }
            Condition::Or(a, b) => {
                let a = self.check_super_strict(a)?;
                let b = self.check_super_strict(b)?;
                Ok(a || b)
            }
        }
    }
}

impl Condition {
    fn maybe_paren(&self) -> String {
        match self {
            Condition::Variable(_) => self.to_string(),
            _ => format!("({})", self.to_string()),
        }
    }

    fn maybe_paren_colored(&self) -> String {
        match self {
            Condition::Variable(_) => self.to_colored_string(),
            _ => format!("{}{}{}", "(".red(), self.to_colored_string(), ")".red()),
        }
    }

    pub fn to_colored_string(&self) -> String {
        match self {
            Condition::Variable(v) => v.to_string(),
            Condition::Literal(v) => v.to_colored_string(),
            Condition::Eq(var, val) => format!("{} == {}", var.green(), val.to_colored_string()),
            Condition::Gt(var, val) => format!("{} > {}", var.green(), val.to_colored_string()),
            Condition::Lt(var, val) => format!("{} < {}", var.green(), val.to_colored_string()),
            Condition::Not(v) => format!("{}{}", "!".yellow(), v.maybe_paren_colored()),
            Condition::And(a, b) => {
                format!(
                    "{} {} {}",
                    a.maybe_paren_colored(),
                    "&".yellow(),
                    b.maybe_paren_colored()
                )
            }
            Condition::Or(a, b) => {
                format!(
                    "{} {} {}",
                    a.maybe_paren_colored(),
                    "|".yellow(),
                    b.maybe_paren_colored()
                )
            }
        }
    }
}

impl ToString for Condition {
    fn to_string(&self) -> String {
        match self {
            Condition::Variable(v) => v.to_string(),
            Condition::Literal(v) => v.to_string(),
            Condition::Eq(var, val) => format!("{} = {}", var, val.to_string()),
            Condition::Gt(var, val) => format!("{} > {}", var, val.to_string()),
            Condition::Lt(var, val) => format!("{} < {}", var, val.to_string()),
            Condition::Not(v) => format!("!{}", v.maybe_paren()),
            Condition::And(a, b) => format!("{} & {}", a.maybe_paren(), b.maybe_paren()),
            Condition::Or(a, b) => format!("{} | {}", a.maybe_paren(), b.maybe_paren()),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum Propagation {
    #[default]
    Sequential,
    Inverse,
    InputsFirst,
    OutputFirst,
    Conditional(Condition),
    ConditionalStrict(Condition),
    ConditionalSuperStrict(Condition),
    List(RVec<RString>),
    Path(StrPath),
}

impl ToString for Propagation {
    fn to_string(&self) -> String {
        match self {
            Self::Sequential => "<sequential>".to_string(),
            Self::Inverse => "<inverse>".to_string(),
            Self::InputsFirst => "<inputsfirst>".to_string(),
            Self::OutputFirst => "<outputfirst>".to_string(),
            Self::Conditional(c) => format!("({})", c.to_string()),
            Self::ConditionalStrict(c) => format!("(={})", c.to_string()),
            Self::ConditionalSuperStrict(c) => format!("(=={})", c.to_string()),
            Self::List(v) => format!(
                "[{}]",
                v.iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::Path(p) => format!("[{}]", p.to_string()),
        }
    }
}

impl Propagation {
    pub fn to_colored_string(&self) -> String {
        match self {
            Self::Sequential => format!("<{}>", "sequential".red()),
            Self::Inverse => format!("<{}>", "inverse".red()),
            Self::InputsFirst => format!("<{}>", "inputsfirst".red()),
            Self::OutputFirst => format!("<{}>", "outputfirst".red()),
            Self::Conditional(c) => format!("({})", c.to_colored_string()),
            Self::ConditionalStrict(c) => format!("(={})", c.to_colored_string()),
            Self::ConditionalSuperStrict(c) => format!("(=={})", c.to_colored_string()),
            Self::List(v) => format!(
                "[{}]",
                v.iter()
                    .map(|a| a.as_str().green().to_string())
                    .collect::<Vec<String>>()
                    .join(", ")
            ),
            Self::Path(p) => format!("[{}]", p.to_colored_string()),
        }
    }
}

#[repr(C)]
#[derive(StableAbi, Debug, Clone, PartialEq)]
pub enum FunctionArg {
    Arg(Attribute),
    KwArg(KeyVal),
}

#[repr(C)]
#[derive(StableAbi, Debug, Clone, PartialEq)]
pub struct KeyVal {
    pub key: RString,
    pub val: Attribute,
}
