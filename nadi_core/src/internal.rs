#![allow(clippy::module_inception)]

mod attrs;
mod attrs2;
mod command;
mod connections;
mod debug;
mod render;
mod table;
mod timeseries;

use crate::functions::NadiFunctions;
use crate::plugins::NadiPlugin;

pub(crate) fn register_internal(funcs: &mut NadiFunctions) {
    // These things need to be automated if possible, but I don't
    // think that is possible: search all types that implement
    // NadiPlugin trait within functions
    render::RenderMod {}.register(funcs);
    attrs::AttrsMod {}.register(funcs);
    attrs2::AttrsMod {}.register(funcs);
    debug::DebugMod {}.register(funcs);
    timeseries::TimeseriesMod {}.register(funcs);
    command::CommandMod {}.register(funcs);
    connections::ConnectionsMod {}.register(funcs);
    table::TableMod {}.register(funcs);
}
