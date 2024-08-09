pub mod attributes;
pub mod ffi;
pub mod network;
pub mod node;
pub mod plugins;
pub mod timeseries;

pub static CORE_VERSION: &str = env!("CARGO_PKG_VERSION");
pub static RUSTC_VERSION: &str = env!("RUSTC_VERSION");

pub use network::Network;
pub use node::{Node, NodeInner};
