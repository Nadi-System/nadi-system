#![allow(clippy::module_inception)]

mod attrs;
mod attrs2;
mod command;
mod connections;
mod core;
mod debug;
mod regex;
mod render;
mod table;
mod timeseries;
mod visuals;

use crate::functions::NadiFunctions;
use crate::plugins::NadiPlugin;

pub(crate) fn register_internal(funcs: &mut NadiFunctions) {
    // These things need to be automated if possible, but I don't
    // think that is possible: search all types that implement
    // NadiPlugin trait within functions
    attrs::AttrsMod {}.register(funcs);
    attrs2::AttrsMod {}.register(funcs);
    command::CommandMod {}.register(funcs);
    connections::ConnectionsMod {}.register(funcs);
    core::CoreMod {}.register(funcs);
    debug::DebugMod {}.register(funcs);
    regex::RegexMod {}.register(funcs);
    render::RenderMod {}.register(funcs);
    table::TableMod {}.register(funcs);
    timeseries::TimeseriesMod {}.register(funcs);
    visuals::VisualsMod {}.register(funcs);
}
