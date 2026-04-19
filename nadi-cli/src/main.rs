use clap::{Parser, ValueEnum};
use nadi_core::parser::tokenizer::{ParenCheck, TaskToken, Token};
use nadi_core::tasks::TaskContext;
use nadi_core::{functions::NadiFunctions, network::Network};
use std::sync::mpsc::channel;
use std::thread;
use std::{
    io::{Read, Write},
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
    /// Install the given nadi plugin
    #[arg(short, long)]
    install_plugin: Option<PathBuf>,
    /// Install the given nadi plugin
    #[arg(short = 'I', long)]
    internals_only: bool,
    /// Create the files for a new nadi_plugin
    #[arg(short = 'P', long)]
    new_plugin: Option<String>,
    /// Path to the nadi_core library for the new nadi_plugin
    #[arg(short = 'N', long)]
    nadi_core: Option<PathBuf>,
    /// Show the tasks file, do not do anything
    #[arg(short, long, action, requires = "tasks")]
    show: bool,
    /// Use stdin for the tasks; reads the whole stdin before execution
    #[arg(short = 'S', long, action)]
    stdin: bool,
    /// Open the REPL (interactive session) before exiting
    #[arg(short, long, action)]
    repl: bool,
    /// Follow the path of the tasks file as current path while running it
    #[arg(short = 'F', long)]
    follow_path: bool,
    /// Run given string as task before running the file
    #[arg(short, long, value_name = "TASK_STR")]
    task: Option<String>,
    /// Tasks file to run; if `--stdin` is also provided this runs before stdin
    #[arg(value_name = "TASK_FILE")]
    tasks: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = CliArgs::parse();

    if let Some(p) = &args.new_plugin {
        init_plugin(p, &args.nadi_core)?;
        return Ok(());
    } else if let Some(p) = &args.install_plugin {
        // this has to be done before loading NadiFunctions because
        // otherwise the plugin might be loaded already and can't be
        // replaced
        let dirs = std::env::var("NADI_PLUGIN_DIRS")?;
        let dir = PathBuf::from(dirs.split(":").next().unwrap_or_default());
        let pdir = dir.join(nadi_core::NADI_CORE_VERSION);
        std::fs::copy(
            p,
            pdir.join(p.file_name().expect("plugin does not have filename")),
        )?;
        return Ok(());
    }

    let functions = if args.internals_only {
        NadiFunctions::internals()
    } else {
        NadiFunctions::internals_w_plugins()
    };
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
        let (sender, receiver) = channel();
        let mut tasks_ctx = nadi_core::tasks::TaskContext::new(net, sender);
        thread::spawn(move || {
            for msg in receiver {
                msg.print();
            }
        });
        if let Some(ref txt) = args.task {
            execute_tasks(txt, args.print_tasks, &mut tasks_ctx)?;
        }
        if let Some(ref tasks) = args.tasks {
            let txt = std::fs::read_to_string(tasks)?;
            if args.follow_path {
                if let Some(p) = tasks.parent() {
                    _ = std::env::set_current_dir(p);
                }
            }
            execute_tasks(&txt, args.print_tasks, &mut tasks_ctx)?;
        }
        if args.stdin {
            let mut txt = String::new();
            std::io::stdin().read_to_string(&mut txt)?;
            execute_tasks(&txt, args.print_tasks, &mut tasks_ctx)?;
        }
        if args.repl {
            repl(tasks_ctx);
        }
    }
    Ok(())
}

fn repl(mut ctx: TaskContext) {
    let mut residue = false;
    let mut input = String::new();
    loop {
        if !residue {
            input.clear();
            print!(">>> ");
        } else {
            print!(">.. ");
        }
        residue = false;
        _ = std::io::stdout().flush();
        match std::io::stdin().read_line(&mut input) {
            Ok(_) => {
                let tokens = nadi_core::parser::tokenizer::get_tokens(&input);
                if let Ok(tkns) = Token::validate(tokens.clone()) {
                    if let ParenCheck::Unpaired(_) = ParenCheck::scan(&tkns) {
                        residue = true;
                        continue;
                    }
                }

                let tasks = match nadi_core::parser::tasks::parse(tokens) {
                    Ok(t) => t,
                    Err(e) => {
                        println!("{}", e.user_msg_color(None));
                        continue;
                    }
                };
                for task in tasks {
                    match ctx.execute(task) {
                        Ok(Some(p)) => println!("{p}"),
                        Err(p) => {
                            println!("{}", p);
                            continue;
                        }
                        _ => (),
                    }
                }
            }
            Err(error) => println!("error: {error}"),
        }
    }
}

fn init_plugin(name: &str, nadi_core: &Option<PathBuf>) -> std::io::Result<()> {
    let path = PathBuf::from(name);
    let name = name.replace('-', "_");
    std::fs::create_dir(&path)?;
    let nadi_core = match nadi_core {
        Some(nc) => format!(
            "{{path = {:?}, version={:?}}}",
            // One step up since the Cargo.toml will be inside the plugin dir
            PathBuf::from("..").join(nc),
            nadi_core::NADI_CORE_VERSION,
        ),
        None => format!("{:?}", nadi_core::NADI_CORE_VERSION),
    };
    std::fs::write(
        path.join("Cargo.toml"),
        format!(
            "
[package]
name = \"{name}\"
version = \"0.1.0\"
edition = \"2021\"


[lib]
crate-type = [\"cdylib\"]

# make sure you use the same version of nadi_core, your nadi-system is in
[dependencies]
abi_stable = \"0.11.3\"
nadi_core = {nadi_core}
"
        ),
    )?;
    std::fs::create_dir(path.join("src"))?;
    std::fs::write(
        path.join("src").join("lib.rs"),
        format!(
            "
use nadi_core::nadi_plugin::nadi_plugin;

#[nadi_plugin]
mod {name} {{
    use nadi_core::prelude::*;

    /// The macros imported from nadi_plugin read the rust function you
    /// write and use that as a base to write more core internally that
    /// will be compiled into the shared libraries. This means it'll
    /// automatically get the argument types, documentation, mutability,
    /// etc. For more details on what they can do, refer to nadi book.
    use nadi_core::nadi_plugin::{{node_func, network_func, env_func}};

    /// Example Environment function for the plugin
    ///
    /// You can use markdown format to write detailed documentation for the
    /// function you write. This will be availble from nadi-help.
    #[env_func(pre = \"Message: \")]
    fn echo(message: String, pre: String) -> String {{
         format!(\"{{}}{{}}\", pre, message)
    }}

    /// Example Node function for the plugin
    #[node_func]
    fn node_name(node: &NodeInner) -> String {{
         node.name().to_string()
    }}

    /// Example Network function for the plugin
    ///
    /// You can also write docstrings for the arguments, this syntax is not
    /// a valid rust syntax, but our macro will read those docstrings, saves
    /// it and then removes it so that rust does not get confused. This means
    /// You do not have to write separate documentation for functions.
    #[network_func]
    fn node_first_with_attr(
        net: &Network,
        /// Name of the attribute to search
        attrname: String,
    ) -> Option<String> {{
         for node in net.nodes() {{
             let node = node.lock();
             if node.attr_dot(&attrname).is_ok() {{
                 return Some(node.name().to_string())
             }}
        }}
        None
    }}
}}
",
        ),
    )?;
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
            _ => print!("{}", tok.ty.highlight().colored(tok.content)),
        }
    }
    println!("\n----Parsing Tasks----");
    match nadi_core::parser::tasks::parse(tokens) {
        Ok(tasks) => {
            for task in tasks {
                // println!("{task:?}");
                for tk in nadi_core::parser::tokenizer::get_tokens(&task.to_string()) {
                    print!("{}", tk.ty.highlight().colored(tk.content));
                }
                println!();
            }
        }
        Err(e) => println!("{}", e.user_msg_color(Some(&filename.to_string_lossy()))),
    };
}

fn execute_tasks(txt: &str, print_tasks: bool, tasks_ctx: &mut TaskContext) -> anyhow::Result<()> {
    let tokens = nadi_core::parser::tokenizer::get_tokens(txt);
    let tasks = match nadi_core::parser::tasks::parse(tokens) {
        Ok(t) => t,
        Err(e) => return Err(anyhow::Error::msg(e.user_msg_color(None))),
    };

    for fc in tasks {
        if print_tasks {
            println!("{}", fc);
        }
        match tasks_ctx.execute(fc) {
            Ok(Some(p)) => println!("{p}"),
            Err(p) => return Err(anyhow::Error::msg(p)),
            _ => (),
        }
    }
    Ok(())
}
