#![allow(clippy::module_inception)]
use crate::attrs::{AttrMap, AttrSlice};
use crate::network::StrPath;
use crate::plugins::{load_library_safe, NadiPlugin};
use crate::prelude::*;
use crate::table::{contents_2_md, ColumnAlign};
use crate::tasks::{FunctionCall, Task, TaskInput, TaskType};
use abi_stable::std_types::Tuple2;
use abi_stable::{
    sabi_trait,
    std_types::{
        map::REntry,
        RBox, RErr, RHashMap, ROk,
        ROption::{self, RNone, RSome},
        RResult, RSlice, RString, RVec,
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

// #[repr(C)]
// #[derive(StableAbi, Default)]
// struct FunctionArg {
//     name: RString,
//     r#type: RString,
//     kwarg: bool,
//     optional: bool,
//     default: Attribute,
// }

#[sabi_trait]
pub trait NodeFunction: Debug + Clone {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    // fn args(&self) -> RSlice<FunctionArg>;
    fn signature(&self) -> RString;
    fn code(&self) -> RString;
    fn call(&self, obj: RSlice<Node>, ctx: &FunctionCtx) -> RResult<(), RString>;
}

#[sabi_trait]
pub trait NetworkFunction: Debug + Clone {
    fn name(&self) -> RString;
    fn help(&self) -> RString;
    // fn args(&self) -> RSlice<FunctionArg>;
    fn signature(&self) -> RString;
    fn code(&self) -> RString;
    fn call(&self, obj: &mut Network, ctx: &FunctionCtx) -> FunctionRet;
}

// A trait object for the `State` Trait Object
pub type NodeFunctionBox = NodeFunction_TO<'static, RBox<()>>;

pub type NetworkFunctionBox = NetworkFunction_TO<'static, RBox<()>>;

#[repr(C)]
#[derive(StableAbi, Default)]
pub struct PluginFunctions {
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

    pub fn push_network(&mut self, func: RString) {
        self.network.push(func);
    }

    pub fn push_node(&mut self, func: RString) {
        self.node.push(func);
    }

    pub fn network(&self) -> &RVec<RString> {
        &self.network
    }

    pub fn node(&self) -> &RVec<RString> {
        &self.node
    }
}

// TODO: add environmental variables, like verbose, progress, debug,
// etc. that all functions can read (passed along with args, kwargs to
// all functions)
#[repr(C)]
#[derive(StableAbi, Default)]
pub struct NadiFunctions {
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
        let (node_funcs, net_funcs) = self.list_functions_md(true);

        writeln!(
            doc,
            "## Node Functions\n{}\n\n## Network Functions\n{}",
            node_funcs, net_funcs
        )?;

        fn func_sig(sig: RString) -> String {
            let sig2 = sig.trim_start_matches('(').trim_end_matches(')');
            let args: Vec<&str> = sig2.split(",").map(|s| s.trim()).collect();
            if args.len() < 3 {
                sig.to_string()
            } else {
                format!("(\n    {}\n)", args.join(",\n    "))
            }
        }

        for Tuple2(plug, funcs) in self.plugins() {
            let mut doc = BufWriter::new(File::create(
                outdir.as_ref().join(plug.as_str()).with_extension("md"),
            )?);
            if !funcs.node().is_empty() {
                writeln!(doc, "# Node Functions")?;
                for func in funcs.node() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.node(&fname).expect("Func Should Exist");
                    writeln!(doc, "## {func} {{#node.{func}}}")?;
                    writeln!(doc, "```sig")?;
                    writeln!(
                        doc,
                        "node {}.{}{}",
                        plug,
                        func,
                        func_sig(func_obj.signature())
                    )?;
                    writeln!(doc, "```\n")?;
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
                    writeln!(
                        doc,
                        "network {}.{}{}",
                        plug,
                        func,
                        func_sig(func_obj.signature())
                    )?;
                    writeln!(doc, "```\n")?;
                    writeln!(doc, "{}", func_obj.help().replace("\n#", "\n###"))?;
                }
            }
        }
        Ok(())
    }

    pub fn list_functions(&self) {
        fn print_func(p: &RString, t: &str, f: &RString, sig: RString) {
            print!("{} {}.{}", t, p.as_str().red(), f.as_str().blue(),);
            let args: Vec<String> = sig
                .trim_start_matches('(')
                .trim_end_matches(')')
                .replace(" ", "")
                .split(",")
                .map(|a| match a.split_once(":") {
                    Some((key, tyval)) => match tyval.split_once("=") {
                        Some((ty, val)) => {
                            format!("{} : {} = {}", key.bright_red(), ty.green(), val.yellow())
                        }
                        None => format!("{} : {}", key.bright_red(), tyval.green()),
                    },
                    None => match a.split_once("=") {
                        Some((key, val)) => format!("{} = {}", key.bright_red(), val.yellow()),
                        None => format!("{}", a.bright_red()),
                    },
                })
                .collect();
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
                    print_func(plug, "node", func, func_obj.signature());
                }
            }
            if !funcs.network().is_empty() {
                for func in funcs.network() {
                    let fname = format!("{plug}.{func}");
                    let func_obj = self.network(&fname).expect("Func Should Exist");
                    print_func(plug, "network", func, func_obj.signature());
                }
            }
        }
    }

    pub fn list_functions_md(&self, link: bool) -> (String, String) {
        let mut node_functions = vec![];
        let mut net_functions = vec![];
        let fname = if link {
            |p: &str, t: &str, n: &str, h: &str| {
                vec![
                    format!("[`{p}`]({p}.md)"),
                    format!("[`{n}`]({p}.md#{t}.{n})"),
                    h.to_string(),
                ]
            }
        } else {
            |p: &str, _t: &str, n: &str, h: &str| vec![p.to_string(), n.to_string(), h.to_string()]
        };
        for Tuple2(func, fobj) in &self.node {
            let (plug, name) = func.split_once('.').unwrap_or(("null", func.as_str()));
            node_functions.push(fname(
                plug,
                "network",
                name,
                fobj.help().lines().next().unwrap_or_default(),
            ));
        }
        for Tuple2(func, fobj) in &self.network {
            let (plug, name) = func.split_once('.').unwrap_or(("null", func.as_str()));
            net_functions.push(fname(
                plug,
                "node",
                name,
                fobj.help().lines().next().unwrap_or_default(),
            ));
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
        )
    }

    pub fn call_node(
        &self,
        func: &str,
        nodes: RSlice<Node>,
        ctx: &FunctionCtx,
    ) -> anyhow::Result<()> {
        match self.node(func) {
            Some(f) => f
                .call(nodes, ctx)
                .map_err(|e| anyhow::Error::msg(e.to_string()))
                .into(),
            None => anyhow::bail!("Node Function {} not found", func),
        }
    }

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
}

impl FunctionCtx {
    pub fn from_arg_kwarg(args: Vec<Attribute>, kwargs: HashMap<String, Attribute>) -> Self {
        let args = RVec::from(args);
        let kwargs = kwargs
            .into_iter()
            .map(|(k, v)| (RString::from(k), v))
            .collect();
        Self { args, kwargs }
    }

    // pub fn node_task(node: &NodeInner, args: &[TaskInput], kwargs: &HashMap<String, TaskInput>, out: &Option<String>) -> anyhow::Result<Self> {
    // 	let args = args.iter().map(|a| {
    // 	    match a {
    // 		TaskInput::Literal(v) => Ok(v),
    // 		TaskInput::Variable(v) => node.try_attr(v),
    // 		_ => Err(anyhow::Error::msg("Invalid output")),
    // 	    }
    // 	}).collect::<anyhow::Result<Vec<Attribute>>>()?.into();
    // 	let kwargs = kwargs.iter().map(|(k, a)| {
    // 	    let k = RString::from(k);
    // 	    match a {
    // 		TaskInput::Literal(v) => Ok((k, v)),
    // 		TaskInput::Variable(v) => Ok((k, node.try_attr(v)?)),
    // 	    _ => Err(anyhow::Error::msg("Invalid output")),
    // 	    }
    // 	}).collect::<anyhow::Result<HashMap<String, Attribute>>>()?.into();
    // 	let outattr = out.map(|s| s.into()).into();
    //     Ok(Self { args, kwargs, outattr })
    // }

    // pub fn network_task(net: &Network, args: &[TaskInput], kwargs: &HashMap<String, TaskInput>, out: &Option<String>) -> anyhow::Result<Self> {
    // 	let args = args.iter().map(|a| {
    // 	    match a {
    // 		TaskInput::Literal(v) => Ok(v),
    // 		TaskInput::Variable(v) => net.try_attr(v),
    // 		_ => Err(anyhow::Error::msg("Invalid output")),
    // 	    }
    // 	}).collect::<anyhow::Result<Vec<Attribute>>>()?.into();
    // 	let kwargs = kwargs.iter().map(|(k, a)| {
    // 	    let k = RString::from(k);
    // 	    match a {
    // 		TaskInput::Literal(v) => Ok((k, v)),
    // 		TaskInput::Variable(v) => Ok((k, net.try_attr(v)?)),
    // 	    _ => Err(anyhow::Error::msg("Invalid output")),
    // 	    }
    // 	}).collect::<anyhow::Result<HashMap<String, Attribute>>>()?.into();
    // 	let outattr = out.map(|s| s.into()).into();
    //     Ok(Self { args, kwargs, outattr })
    // }

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

#[repr(C)]
#[derive(StableAbi, Debug, Default, Clone, PartialEq)]
pub enum Propagation {
    #[default]
    Sequential,
    Inverse,
    InputsFirst,
    OutputFirst,
    List(RVec<RString>),
    Path(StrPath),
}

impl ToString for Propagation {
    fn to_string(&self) -> String {
        match self {
            Self::Sequential => "(sequential)".to_string(),
            Self::Inverse => "(inverse)".to_string(),
            Self::InputsFirst => "(inputsfirst)".to_string(),
            Self::OutputFirst => "(outputfirst)".to_string(),
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
            Self::Sequential => format!("({})", "sequential".red()),
            Self::Inverse => format!("({})", "inverse".red()),
            Self::InputsFirst => format!("({})", "inputsfirst".red()),
            Self::OutputFirst => format!("({})", "outputfirst".red()),
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
