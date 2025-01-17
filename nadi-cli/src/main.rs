use std::{io::Read, path::{PathBuf, Path}};
use nadi_core::parser::NadiError;
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
    /// Generate markdown doc for all plugins and functions
    #[arg(short, long)]
    generate_doc: Option<PathBuf>,
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
    /// Show the tasks file, do not do anything
    #[arg(short, long, action, requires="tasks")]
    show: bool,
    /// Use stdin for the tasks; reads the whole stdin before execution
    #[arg(short='S', long, action)]
    stdin: bool,
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

fn show_tasks(filename: &Path) {
    let txt = std::fs::read_to_string(filename).unwrap();
    match nadi_core::parser::tokenizer::get_tokens(&txt){
        Ok(tokens) => {
	    println!("\n----File Tokens----");
            for tok in &tokens {
                tok.colored_print();
            }
	    match nadi_core::parser::tasks::parse(tokens) {
		Ok(tasks) => {
		    println!("\n----Parsed Tasks----");
		    for task in tasks {
			println!("{}", task.to_colored_string());
		    }
		}
		Err(e) => println!("{}", e.user_msg(Some(&filename.to_string_lossy()))),
	    };
	},
        Err(e) => println!("{}", e.user_msg(Some(&filename.to_string_lossy()))),
    }
}

fn execute_tasks(functions: &NadiFunctions, txt: &str, args: &CliArgs) -> anyhow::Result<()> {
    let tokens = nadi_core::parser::tokenizer::get_tokens(&txt)?;
    let tasks = match nadi_core::parser::tasks::parse(tokens) {
	Ok(t) => t,
	Err(e) => return Err(anyhow::Error::msg(e.user_msg(None))),
    };
    if args.print_tasks {
        for fc in &tasks {
            println!("{}", fc.to_colored_string());
        }
    }

    let net = if let Some(ref net) = args.network {
        Some(Network::from_file(net)?)
    } else {
        None
    };
    eprintln!("** Running {} Script **", tasks.len());

    let mut tasks_ctx = nadi_core::tasks::TaskContext::new(net);
    for fc in tasks {
        match tasks_ctx.execute(fc) {
	    Ok(Some(p)) => println!("{p}"),
	    Err(p) => return Err(anyhow::Error::msg(p)),
	    _ => (),
	}
    }
    Ok(())
}
