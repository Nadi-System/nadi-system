#![allow(unused)]
#![allow(dead_code)]

pub mod attrs;
pub mod functions;
pub mod network;
pub mod node;
pub mod parser;
pub mod plugins;
pub mod table;
pub mod timeseries;

pub use attrs::{AttrMap, AttrSlice, Attribute, FromAttribute};
pub use network::{Network, StrPath};
pub use node::{new_node, Node, NodeInner};

// workaround for nadi_plugin_macros to work with ::nadi_core:: style
// path made to be used from other libraries/plugins
// https://github.com/rust-lang/rust/pull/55275
extern crate self as nadi_core;
pub use nadi_plugin;

#[macro_export]
macro_rules! return_on_err {
    ($val: expr) => {
        match $val {
            Ok(t) => t,
            Err(e) => return ::nadi_core::functions::FunctionRet::Error(e.to_string().into()),
        }
    };
}

#[macro_export]
macro_rules! return_on_none {
    ($val: expr, $msg: expr) => {
        match $val {
            Some(t) => t,
            None => return ::nadi_core::functions::FunctionRet::Error($msg.into()),
        }
    };
}
