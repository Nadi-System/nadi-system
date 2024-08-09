# Proc Macros for Nadi System Plugins

This crate contains the necessary macros for the nadi system plugin
development in rust.

The plugins can be developed without using the macros, but this makes
it easier, and less error prone.


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
nadi_core = "0.2.0"
nadi_plugin = "0.1.0"
```

`src/lib.rs`:
```rust
use nadi_plugin::nadi_plugin;

#[nadi_plugin]
mod example {
    use nadi_core::{attributes::AsValue, Network, NodeInner};
    use nadi_plugin::nadi_func;

    /// Print the given attr of the node as string.
    ///
    /// This is a basic node funtion, the purpose of this function is
    /// to demonstrate how node functions can be written. But it might
    /// be useful in some cases.
    #[nadi_func]
    fn print_attr(node: &mut NodeInner, attr: String, key: bool) {
        println!(
            "{}{}",
            if key { &attr } else { "" },
            node.attr(&attr).into_string().unwrap_or_default()
        );
    }

    /// List all the attributes on the node
    #[nadi_func]
    fn list_attr(node: &mut NodeInner) {
        println!("{}: {}", node.name(), node.attributes().join(", "));
    }
	
    /// Print the given attr of all the nodes in a network
    #[nadi_func]
    fn print_net_attr(net: &mut Network, attr: String) {
        for node in net.nodes() {
            let node = node.borrow();
            println!(
                "{}: {}",
                node.name(),
                node.attr(&attr).into_string().unwrap_or_default()
            );
        }
    }
```
