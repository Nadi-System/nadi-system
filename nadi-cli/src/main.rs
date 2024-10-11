use std::{io::Read, path::PathBuf};

use clap::{Parser, ValueEnum};
use nadi_core::{functions::NadiFunctions, network::Network};

#[derive(Default, Debug, Clone, ValueEnum)]
enum FunctionType {
    #[default]
    Node,
    Network,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct CliArgs {
    /// list all functions and exit
    #[arg(short, long)]
    list_functions: bool,
    /// list all functions and exit for completions
    #[arg(short = 'C', long)]
    completion: Option<FunctionType>,
    /// print help for a function
    #[arg(short, long)]
    fnhelp: Option<String>,
    /// print code for a function
    #[arg(short = 'c', long)]
    fncode: Option<String>,
    /// print tasks file and exit
    #[arg(short, long)]
    print_tasks: bool,
    /// connections file
    #[arg(short, long)]
    network: Option<PathBuf>,
    #[arg(short, long)]
    /// Run given string as tasks
    task: Option<String>,
    /// Tasks file to run; if `--stdin` is also provided runs this before stdin
    tasks: Option<PathBuf>,
    /// Use stdin for the tasks; reads the whole stdin before execution
    #[arg(short, long, action)]
    stdin: bool,
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();
    let functions = NadiFunctions::new();

    if let Some(func) = args.fnhelp {
        println!("{}", functions.help(&func).unwrap_or_default());
    } else if let Some(func) = args.fncode {
        println!("{}", functions.code(&func).unwrap_or_default());
    } else if args.list_functions {
        functions.list_functions();
    } else if let Some(comp) = args.completion {
        match comp {
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
        }
    } else {
        if let Some(ref tasks) = args.tasks {
            let txt = std::fs::read_to_string(tasks)?;
            execute_tasks(&functions, &txt, &args)?;
        }
        if let Some(ref txt) = args.task {
            execute_tasks(&functions, txt, &args)?;
        }
        if args.stdin {
            let mut txt = String::new();
            std::io::stdin().read_to_string(&mut txt)?;
            execute_tasks(&functions, &txt, &args)?;
        }
    }
    Ok(())
}

fn execute_tasks(functions: &NadiFunctions, txt: &str, args: &CliArgs) -> anyhow::Result<()> {
    let script =
        nadi_core::parser::functions::parse_script_complete(&txt).map_err(anyhow::Error::msg)?;
    if args.print_tasks {
        for fc in &script {
            println!("{}", fc.to_colored_string());
        }
    }

    let mut net = if let Some(ref net) = args.network {
        Network::from_file(net)?
    } else {
        // if network not given start with empty network
        Network::default()
    };
    eprintln!("** Running {} Script **", script.len());
    for fc in &script {
        functions
            .execute(fc, &mut net)
            .map_err(anyhow::Error::msg)?;
    }
    Ok(())
}
