pub mod attrs;
pub mod functions;
pub mod network;
pub mod node;
pub mod plugins;
pub mod table;
pub mod tasks;
pub mod timeseries;

#[cfg(feature = "functions")]
mod internal;

#[cfg(feature = "parser")]
pub mod parser;

/// Prelude for the most basic NADI types: node, network and attributes
pub mod prelude {
    pub use crate::attrs::{AttrMap, AttrSlice, Attribute, FromAttribute, FromAttributeRelaxed};
    pub use crate::network::Network;
    pub use crate::node::{Node, NodeInner};
}

// workaround for nadi_plugin_macros to work with ::nadi_core:: style
// path made to be used from other libraries/plugins
// https://github.com/rust-lang/rust/pull/55275
extern crate self as nadi_core;
// re-export these so the plugin systems will use the same version
pub use abi_stable;
pub use anyhow;
pub use nadi_plugin;
pub use string_template_plus as string_template;

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
