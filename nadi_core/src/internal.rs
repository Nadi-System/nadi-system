#![allow(clippy::module_inception)]

mod attrs;
mod attrs2;
mod command;
mod connections;
mod core;
mod debug;
mod files;
mod logic;
mod math;
mod regex;
mod render;
mod series;
mod table;
mod timeseries;
mod visuals;

use crate::functions::NadiFunctions;
use crate::plugins::NadiPlugin;

/// Register the internal plugins
pub(crate) fn register_internal(funcs: &mut NadiFunctions) {
    // These things need to be automated if possible, but I don't
    // think that is possible: search all types that implement
    // NadiPlugin trait within functions
    attrs::AttrsMod {}.register(funcs);
    attrs2::AttrsMod {}.register(funcs);
    command::CommandMod {}.register(funcs);
    connections::ConnMod {}.register(funcs);
    core::CoreMod {}.register(funcs);
    debug::DebugMod {}.register(funcs);
    files::FilesMod {}.register(funcs);
    logic::LogicMod {}.register(funcs);
    math::MathMod {}.register(funcs);
    regex::RegexMod {}.register(funcs);
    render::RenderMod {}.register(funcs);
    series::SeriesMod {}.register(funcs);
    table::TableMod {}.register(funcs);
    timeseries::TsMod {}.register(funcs);
    visuals::VisualsMod {}.register(funcs);
}

#[cfg(test)]
mod tests {
    use super::register_internal;
    use crate::functions::NadiFunctions;
    use crate::prelude::*;
    use crate::tasks::TaskContext;
    use crate::timeseries::{SeriesMap, TsMap};
    use abi_stable::std_types::Tuple2;
    use pulldown_cmark::{CodeBlockKind, CowStr, Event, Options, Parser, Tag, TagEnd};
    use std::collections::HashMap;

    fn test_plugin_function(txt: &str, ctx: &mut TaskContext) -> Result<(), String> {
        let tokens = nadi_core::parser::tokenizer::get_tokens(txt);
        let tasks = match nadi_core::parser::tasks::parse(tokens) {
            Ok(t) => t,
            Err(e) => return Err(e.user_msg_color(None)),
        };
        for tsk in tasks {
            if let Err(p) = ctx.execute(tsk.clone()) {
                return Err(format!("Error in:\n{tsk}\n {p}"));
            }
        }
        Ok(())
    }

    fn extract_tasks(txt: &str) -> String {
        let parser = Parser::new_ext(txt, Options::empty());
        let mut active = false;
        let mut result: Vec<String> = vec![];

        for el in parser {
            match el {
                Event::Start(Tag::CodeBlock(CodeBlockKind::Fenced(CowStr::Borrowed("task")))) => {
                    active = true
                }
                Event::End(TagEnd::CodeBlock) => active = false,
                Event::Text(code) if active => result.push(code.to_string()),
                _ => (),
            }
        }

        result.join("\n")
    }

    #[test]
    fn test_all_functions() {
        let mut functions = NadiFunctions::default();
        register_internal(&mut functions);
        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut ctx = TaskContext {
            network: Network::default(),
            functions: functions.clone(),
            env: AttrMap::new(),
            series: SeriesMap::new(),
            timeseries: TsMap::new(),
            hook: Vec::new(),
            udf: HashMap::new(),
            channel: sender,
        };
        let mut tests = 0;
        let mut errors = Vec::new();
        for Tuple2(name, func) in functions.env_functions() {
            eprintln!("Testing env {name}");
            let tasks = extract_tasks(func.help().as_str());
            if let Err(e) = test_plugin_function(&tasks, &mut ctx) {
                errors.push(("env", name, e));
            }
            ctx.clear();
        }
        for Tuple2(name, func) in functions.network_functions() {
            eprintln!("Testing network {name}");
            let tasks = extract_tasks(func.help().as_str());
            if let Err(e) = test_plugin_function(&tasks, &mut ctx) {
                errors.push(("net", name, e));
            }
            tests += 1;
            ctx.clear();
        }
        for Tuple2(name, func) in functions.node_functions() {
            eprintln!("Testing node {name}");
            let tasks = extract_tasks(func.help().as_str());
            if let Err(e) = test_plugin_function(&tasks, &mut ctx) {
                errors.push(("node", name, e));
            }
            tests += 1;
            ctx.clear();
        }
        if !errors.is_empty() {
            let total = errors.len();
            for (ty, name, er) in errors {
                eprintln!("* {ty} {name}: \n{er}");
            }
            panic!("{total} Error(s) (out of {tests}) in Internal Function Help");
        }
    }
}
