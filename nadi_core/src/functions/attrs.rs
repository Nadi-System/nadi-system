// This mod is kept as an example to how the functions are written
// without the macros. Any additional functions are recommended to be
// written using the macros provided by nadi_plugin crate
use crate::functions::{FunctionRet, NadiFunctions, NodeFunction};
use crate::plugins::NadiPlugin;
use crate::{return_on_err, AttrSlice, FromAttribute, NodeInner};
use abi_stable::sabi_trait::TD_CanDowncast;

use abi_stable::std_types::Tuple2;

use abi_stable::std_types::RString;
use nadi_plugin::node_func;
use string_template_plus::Template;

use super::{FunctionCtx, NodeFunction_TO};

pub struct AttrsMod;

impl NadiPlugin for AttrsMod {
    fn name(&self) -> RString {
        "attrs".into()
    }
    fn register(&self, nf: &mut NadiFunctions) {
        nf.register_node_function(NodeFunction_TO::from_value(LoadAttrs, TD_CanDowncast));
        nf.register_node_function(NodeFunction_TO::from_value(PrintAllAttrs, TD_CanDowncast));
        // node_func makes struct from function_name to  FunctionNameNode:
        nf.register_node_function(NodeFunction_TO::from_value(PrintAttrsNode, TD_CanDowncast));
    }
}

#[derive(Debug)]
pub struct LoadAttrs;

impl NodeFunction for LoadAttrs {
    fn name(&self) -> RString {
        "load_attrs".into()
    }

    fn help(&self) -> RString {
        "Loads attrs from file for all nodes based on the given template
"
        .into()
    }

    fn call(&self, node: &mut NodeInner, ctx: &FunctionCtx) -> FunctionRet {
        let templ: Template = match ctx.arg_kwarg(0, "filename") {
            Some(Ok(a)) => a,
            Some(Err(e)) => return FunctionRet::Error(e.into()),
            None => return FunctionRet::Error("Text template not given".into()),
        };
        let filepath = match node.render(&templ) {
            Ok(f) => f,
            Err(e) => return FunctionRet::Error(e.to_string().into()),
        };
        println!("Loadin Attributes from: {filepath}");
        match node.load_attr(&filepath) {
            Ok(_) => FunctionRet::None,
            Err(e) => FunctionRet::Error(e.to_string().into()),
        }
    }

    fn code(&self) -> RString {
        "".into()
    }
}

#[derive(Debug)]
pub struct PrintAllAttrs;

impl NodeFunction for PrintAllAttrs {
    fn name(&self) -> RString {
        "print_all_attrs".into()
    }

    fn help(&self) -> RString {
        "Print all attrs node
"
        .into()
    }

    fn call(&self, node: &mut NodeInner, _ctx: &FunctionCtx) -> FunctionRet {
        for Tuple2(k, v) in node.attrs() {
            println!("{k} = {}", v.to_string());
        }
        FunctionRet::None
    }
    fn code(&self) -> RString {
        "".into()
    }
}

/// Print the given node attributes
#[node_func]
fn print_attrs(node: &mut NodeInner, #[args] attrs: AttrSlice) -> FunctionRet {
    let attrs = return_on_err!(attrs
        .iter()
        .map(String::try_from_attr)
        .collect::<Result<Vec<String>, String>>());

    for a in attrs {
        if let Some(v) = node.attr(&a) {
            println!("{a} = {}", v.to_string());
        }
    }
    FunctionRet::None
}
