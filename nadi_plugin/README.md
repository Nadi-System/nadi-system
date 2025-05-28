# Proc Macros for Nadi System Plugins

## Note:
This crate is re-exported by the `nadi_core` library, **do not use this directly**.

## Introduction
This crate contains the necessary macros for the nadi system plugin
development in rust.

The plugins can be developed without using the macros, but this makes
it concise, less error prone, and easier to upgrade to new versions.


Example Plugin:

`Cargo.toml`:
```toml
[package]
name = "example_plugin"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
anyhow = "1.0.86"
nadi_core = "0.7.0"
```

`src/lib.rs`:
```rust
use nadi_core::nadi_plugin::nadi_plugin;

#[nadi_plugin]
mod example {
    use nadi_core::{Network, NodeInner};
    use nadi_core::nadi_plugin::{network_func, node_func};

    /// Print the given attr of the node as string.
    ///
    /// This is a basic node funtion, the purpose of this function is
    /// to demonstrate how node functions can be written. But it might
    /// be useful in some cases. You can have function argument
	/// documentation and default values for the arguments that are not
	/// supported by rust. That's the beauty of macros in rust.
    #[node_func(key=true)]
    fn print_attr(
		node: &NodeInner,
		/// Attribute name
		attr: String,
		/// Print the attribute name as well
		key: bool
	) {
        println!(
            "{}{}",
            if key { &attr } else { "" },
            node.attr(&attr).map(|a| a.to_string()).unwrap_or_default()
        );
    }

    /// List all the attributes on the node
    #[node_func]
    fn list_attr(node: &NodeInner) {
        let attrs: Vec<&str> = node.attrs().iter().map(|kv| kv.0.as_str()).collect();
        println!("{}: {}", node.name(), attrs.join(", "));
    }

    /// Print the given attr of all the nodes in a network
    #[network_func]
    fn print_net_attr(net: &Network, attr: String) {
        for node in net.nodes() {
            let node = node.lock();
            println!(
                "{}: {}",
                node.name(),
                node.attr(&attr).map(|a| a.to_string()).unwrap_or_default()
            );
        }
    }
}
```
