use pyo3::prelude::*;

mod attrs;
mod functions;
mod network;
mod node;

use functions::*;
use nadi_core::functions::NadiFunctions;

/// A Python module implemented in Rust.
#[pymodule]
fn nadi(py: Python, m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<attrs::PyNDate>()?;
    m.add_class::<attrs::PyNTime>()?;
    m.add_class::<attrs::PyNDateTime>()?;
    m.add_class::<node::PyNode>()?;
    m.add_class::<network::PyNetwork>()?;
    m.add_class::<functions::PyNadiFunctions>()?;
    register_function_modules(py, m)
}

/// This loads the functions and puts them in a hierarchical order as submodules
///
/// You can import anything from inside it, or use dot notation to
/// refer to something, that makes it a lot easier to access and use
/// the functions.
///
/// The hierarchy looks like this:
/// nadi [contains Node, Network, etc]
///   +-- functions
///   | +-- node [contains node functions]
///   | +-- network [contains network functions]
///   | +-- env [contains env functions]
///   +-- plugins
///     +-- <plugin> [each plugin will be added here]
///       +-- node [contains node functions]
///       +-- network [contains network functions]
///       +-- env [contains env functions]
pub fn register_function_modules(py: Python, parent: &Bound<'_, PyModule>) -> PyResult<()> {
    let funcs = PyModule::new(parent.py(), "functions")?;
    let node = PyModule::new(funcs.py(), "node")?;
    let network = PyModule::new(funcs.py(), "network")?;
    let env = PyModule::new(funcs.py(), "env")?;
    funcs.add_submodule(&node)?;
    funcs.add_submodule(&network)?;
    funcs.add_submodule(&env)?;
    parent.add_submodule(&funcs)?;

    let nadi_funcs = NadiFunctions::new();
    for f in nadi_funcs.node_alias() {
        let name = f.0.as_str();
        let f = nadi_funcs
            .node(f.1)
            .expect("Function in alias should exist");
        node.setattr(name, PyNodeFunction::new(f.clone()))?;
    }
    for f in nadi_funcs.network_alias() {
        let name = f.0.as_str();
        let f = nadi_funcs
            .network(f.1)
            .expect("Function in alias should exist");
        network.setattr(name, PyNetworkFunction::new(f.clone()))?;
    }
    for f in nadi_funcs.env_alias() {
        let name = f.0.as_str();
        let f = nadi_funcs
            .env(f.1)
            .expect("Function in alias should exist");
        env.setattr(name, PyEnvFunction::new(f.clone()))?;
    }

    let mods = py.import("sys")?.getattr("modules")?;
    mods.set_item("nadi.functions", funcs)?;
    mods.set_item("nadi.functions.node", node)?;
    mods.set_item("nadi.functions.network", network)?;
    mods.set_item("nadi.functions.env", env)?;

    let plugins = PyModule::new(parent.py(), "plugins")?;
    for plug in nadi_funcs.plugins() {
        let name = plug.0.as_str();
        let pmod = PyModule::new(plugins.py(), name)?;
        let env = PyModule::new(pmod.py(), "env")?;
        let node = PyModule::new(pmod.py(), "node")?;
        let network = PyModule::new(pmod.py(), "network")?;
        for f in plug.1.node() {
            let fname = f.as_str();
            let f = nadi_funcs
                .node(&format!("{}.{}", name, fname))
                .expect("Plugin Function should exist");
            node.setattr(fname, PyNodeFunction::new(f.clone()))?;
        }
        for f in plug.1.network() {
            let fname = f.as_str();
            let f = nadi_funcs
                .network(&format!("{}.{}", name, fname))
                .expect("Plugin Function should exist");
            network.setattr(fname, PyNetworkFunction::new(f.clone()))?;
        }
        for f in plug.1.env() {
            let fname = f.as_str();
            let f = nadi_funcs
                .env(&format!("{}.{}", name, fname))
                .expect("Plugin Function should exist");
            env.setattr(fname, PyEnvFunction::new(f.clone()))?;
        }
        pmod.add_submodule(&node)?;
        pmod.add_submodule(&network)?;
        pmod.add_submodule(&env)?;
        plugins.add_submodule(&pmod)?;

        mods.set_item(format!("nadi.plugins.{}", name), pmod)?;
        mods.set_item(format!("nadi.plugins.{}.node", name), node)?;
        mods.set_item(format!("nadi.plugins.{}.network", name), network)?;
        mods.set_item(format!("nadi.plugins.{}.env", name), env)?;
    }
    parent.add_submodule(&plugins)?;
    mods.set_item("nadi.plugins", plugins)?;

    Ok(())
}
