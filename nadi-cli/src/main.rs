use clap::{Parser, ValueEnum};
use nadi_core::parser::tokenizer::TaskToken;
use nadi_core::tasks::TaskContext;
use nadi_core::{functions::NadiFunctions, network::Network};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Default, Debug, Clone, ValueEnum)]
enum FunctionType {
    #[default]
    Node,
    Network,
    Env,
}

impl FunctionType {
    fn print_functions(&self, functions: &NadiFunctions) {
        match self {
            FunctionType::Node => {
                for f in functions.node_functions().keys() {
                    println!("{f}");
                }
            }
            FunctionType::Network => {
                for f in functions.network_functions().keys() {
                    println!("{f}");
                }
            }
            FunctionType::Env => {
                for f in functions.env_functions().keys() {
                    println!("{f}");
                }
            }
        }
    }
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// list all functions and exit for completions
    #[arg(short = 'C', long, value_name = "FUNC_TYPE")]
    completion: Option<FunctionType>,
    /// print code for a function
    #[arg(short = 'c', long, value_name = "FUNCTION")]
    fncode: Option<String>,
    /// print help for a function
    #[arg(short, long, value_name = "FUNCTION")]
    fnhelp: Option<String>,
    /// Generate markdown doc for all plugins and functions
    #[arg(short, long, value_name = "DOC_DIR")]
    generate_doc: Option<PathBuf>,
    /// list all functions and exit
    #[arg(short, long)]
    list_functions: bool,
    /// network file to load before executing tasks
    #[arg(short, long, value_name = "NETWORK_FILE")]
    network: Option<PathBuf>,
    /// print tasks before running
    #[arg(short, long)]
    print_tasks: bool,
    /// Show the tasks file, do not do anything
    #[arg(short, long, action, requires = "tasks")]
    show: bool,
    /// Use stdin for the tasks; reads the whole stdin before execution
    #[arg(short = 'S', long, action)]
    stdin: bool,
    /// Run given string as task before running the file
    #[arg(short, long, value_name = "TASK_STR")]
    task: Option<String>,
    /// Tasks file to run; if `--stdin` is also provided this runs before stdin
    #[arg(value_name = "TASK_FILE")]
    tasks: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    let functions = NadiFunctions::new();

    if args.show {
        show_tasks(&args.tasks.unwrap());
    } else if let Some(dir) = args.generate_doc {
        functions.plugins_doc(&dir)?;
    } else if let Some(func) = args.fnhelp {
        println!("{}", functions.help(&func).unwrap_or_default());
    } else if let Some(func) = args.fncode {
        println!("{}", functions.code(&func).unwrap_or_default());
    } else if args.list_functions {
        functions.list_functions();
    } else if let Some(comp) = args.completion {
        match comp {
            FunctionType::Env => (),
            _ => comp.print_functions(&functions),
        }
        FunctionType::Env.print_functions(&functions);
    } else {
        let net = if let Some(ref net) = args.network {
            Some(Network::from_file(net)?)
        } else {
            None
        };
        let mut tasks_ctx = nadi_core::tasks::TaskContext::new(net);

        if let Some(ref txt) = args.task {
            execute_tasks(txt, args.print_tasks, &mut tasks_ctx)?;
        }
        if let Some(ref tasks) = args.tasks {
            let txt = std::fs::read_to_string(tasks)?;
            execute_tasks(&txt, args.print_tasks, &mut tasks_ctx)?;
        }
        if args.stdin {
            let mut txt = String::new();
            std::io::stdin().read_to_string(&mut txt)?;
            execute_tasks(&txt, args.print_tasks, &mut tasks_ctx)?;
        }
    }
    Ok(())
}

fn show_tasks(filename: &Path) {
    let txt = std::fs::read_to_string(filename).unwrap();
    let tokens = nadi_core::parser::tokenizer::get_tokens(&txt);
    let mut line = 1;
    print!("{line:3}: ");
    for tok in &tokens {
        match tok.ty {
            TaskToken::NewLine => {
                line += 1;
                print!("\n{line:3}: ");
            }
            _ => tok.colored_print(),
        }
    }
    println!("\n----Parsing Tasks----");
    match nadi_core::parser::tasks::parse(tokens) {
        Ok(tasks) => {
            for task in tasks {
                // println!("{task:?}");
                for tk in nadi_core::parser::tokenizer::get_tokens(&task.to_string()) {
                    tk.colored_print();
                }
                println!();
            }
        }
        Err(e) => println!("{}", e.user_msg(Some(&filename.to_string_lossy()))),
    };
}

fn execute_tasks(txt: &str, print_tasks: bool, tasks_ctx: &mut TaskContext) -> anyhow::Result<()> {
    let tokens = nadi_core::parser::tokenizer::get_tokens(&txt);
    let tasks = match nadi_core::parser::tasks::parse(tokens) {
        Ok(t) => t,
        Err(e) => return Err(anyhow::Error::msg(e.user_msg(None))),
    };

    for fc in tasks {
        if print_tasks {
            println!("{}", fc.to_string());
        }
        match tasks_ctx.execute(fc) {
            Ok(Some(p)) => println!("{p}"),
            Err(p) => return Err(anyhow::Error::msg(p)),
            _ => (),
        }
    }
    Ok(())
}
