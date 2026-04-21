use crate::output::{clipped_from_stdout, output_handler, output_verbose};
use anyhow::Context;
use nadi_core::{
    attrs::AttrMap,
    parser::{tasks, tokenizer::get_tokens},
    tasks::{Task, TaskContextWrap},
    template::Template,
};
use pulldown_cmark::Event;
use std::io::Read;
use std::path::Path;
use std::str::FromStr;
use std::sync::{LazyLock, Mutex};

pub type CodeHandler = fn(&str, &str, &Path) -> Result<Vec<Event<'static>>, anyhow::Error>;

// TODO: remove table and stp, only keep task and then have better
// arguments like from org mode so everything can be done through the
// task, maybe this can be done after the taskreturn has options like
// image/file, etc so that we can take that information.
pub fn nadi_code_args(mark: &str) -> Option<(CodeHandler, String)> {
    // only run for ones with "run" in it
    mark.split_once(" run").and_then(|(p, a)| match p.trim() {
        "table" => Some((run_table as CodeHandler, a.to_string())),
        "task" => Some((run_task as CodeHandler, a.to_string())),
        "stp" | "string-template" => Some((run_template as CodeHandler, a.to_string())),
        _ => None,
    })
}

static mut NADI_CTX: LazyLock<Mutex<TaskContextWrap>> =
    LazyLock::new(|| Mutex::new(TaskContextWrap::new(None)));

fn clear_context() {
    #[allow(static_mut_refs)]
    let ctx: &mut TaskContextWrap =
        unsafe { &mut NADI_CTX.lock().expect("Couldn't lock the task context") };
    ctx.context.clear();
}

fn execute_task(task: Task, loc: &mut AttrMap) -> Result<Option<String>, String> {
    // This saves us from loading the plugins over and over again for
    // each code block, significantly improving the runtime speed
    #[allow(static_mut_refs)]
    let ctx: &mut TaskContextWrap =
        unsafe { &mut NADI_CTX.lock().expect("Couldn't lock the task context") };
    ctx.execute(task, loc).map_err(|e| e.to_string())
}

pub fn run_task(task: &str, args: &str, pwd: &Path) -> anyhow::Result<Vec<Event<'static>>> {
    let mut tasks = String::with_capacity(task.len());
    for line in task.split('\n') {
        tasks.push_str(line.trim_start_matches('!'));
        tasks.push('\n');
    }
    tasks.push('\n');

    let tokens = get_tokens(&tasks);
    let tasks = match tasks::parse(tokens) {
        Ok(t) => t,
        Err(e) => {
            return Ok(output_verbose(
                vec![Event::Text("ParseError:".into())],
                e.user_msg(None),
                "error",
                pwd,
            ));
        }
    };

    if !args.contains("continue") {
        clear_context();
    }

    let mut response = String::new();
    std::env::set_current_dir(pwd)?;
    let mut locals = AttrMap::new();
    for task in tasks {
        let mut buf = gag::BufferRedirect::stdout().unwrap();
        let res = execute_task(task, &mut locals);
        let r = buf.read_to_string(&mut response).unwrap();
        if r > 0 {
            response.push('\n');
        }
        match res {
            Ok(Some(out)) => {
                response.push_str(&out);
                response.push('\n');
            }
            Ok(None) => (),
            Err(e) => {
                return Ok(output_verbose(
                    vec![Event::Text("*Error*:".into())],
                    format!("{response}{e}"),
                    "error",
                    pwd,
                ));
            }
        }
    }
    let output_fmt = args.trim().split(' ').next().unwrap_or("verbose");
    let handler = output_handler(output_fmt);
    Ok(handler(
        vec![Event::Text("Results:".into())],
        clipped_from_stdout(&response),
        args.trim().split_once(' ').map(|a| a.1).unwrap_or_default(),
        pwd,
    ))
}

pub fn run_template(templ: &str, args: &str, pwd: &Path) -> anyhow::Result<Vec<Event<'static>>> {
    let templ = match Template::from_str(templ) {
        Ok(t) => t,
        Err(e) => {
            return Ok(output_verbose(
                vec![Event::Text("*Error*:".into())],
                e.to_string(),
                "error",
                pwd,
            ));
        }
    };
    let mut op = AttrMap::default();
    for kv in args.split(';') {
        if kv.is_empty() {
            continue;
        }
        let (k, v) = kv
            .split_once('=')
            .context("variables not in key=value pairs")?;
        op.insert(k.trim().into(), v.trim().to_string().into());
    }
    match templ.render(&op) {
        Ok(txt) => Ok(output_verbose(
            vec![Event::Text(format!("Results (with: {args}):").into())],
            txt,
            "",
            pwd,
        )),
        Err(e) => Ok(output_verbose(
            vec![Event::Text("*Error*:".into())],
            e.to_string(),
            "error",
            pwd,
        )),
    }
}

pub fn run_table(table: &str, args: &str, pwd: &Path) -> anyhow::Result<Vec<Event<'static>>> {
    let mut table_contents = String::new();
    let mut tasks = String::new();
    for line in table.split('\n') {
        if let Some(l) = line.strip_prefix('!') {
            tasks.push_str(l);
            tasks.push('\n');
        } else {
            table_contents.push_str(line);
            table_contents.push('\n');
        }
    }

    let mut output_fmt = "markdown";
    let mut targs = String::new();
    if let Some((fmt, mut a)) = args.trim().split_once(' ') {
        output_fmt = fmt;
        a = a.trim();
        if !a.is_empty() {
            targs.push(',');
            targs.push_str(a);
        }
    }

    tasks.push_str("\nnetwork table_to_");
    tasks.push_str(output_fmt);
    tasks.push_str("(template=\"");
    tasks.push_str(&table_contents);
    tasks.push('"');
    tasks.push_str(&targs);
    tasks.push_str(")\n");

    let tokens = get_tokens(&tasks);
    let tasks = tasks::parse(tokens)?;

    clear_context();

    let mut response = String::new();
    let mut locals = AttrMap::new();
    for task in tasks {
        // since we can't have anything else print on mdbook
        let mut buf = gag::BufferRedirect::stdout().unwrap();
        let res = execute_task(task, &mut locals);
        response.clear();
        buf.read_to_string(&mut response).unwrap();
        response.push('\n');
        match res {
            Ok(Some(out)) => {
                response.push_str(&out);
            }
            Ok(None) => (),
            Err(e) => {
                return Ok(output_verbose(
                    vec![Event::Text("*Error*:".into())],
                    e,
                    "error",
                    pwd,
                ));
            }
        }
    }
    let handler = output_handler(output_fmt);
    Ok(handler(
        vec![Event::Text("Results:".into())],
        response,
        "",
        pwd,
    ))
}
