use nadi_plugin::nadi_internal_plugin;

/// Command plugin to interact with shell
///
/// Put anything related to shells here, because if some plugins are
/// disabled for security reasons it is easier to block the group.
#[nadi_internal_plugin]
mod command {
    use crate::parser;
    use crate::prelude::*;
    use anyhow::Context;
    use colored::Colorize;
    use nadi_core::nadi_plugin::{env_func, network_func, node_func};
    use std::io::BufRead;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use string_template_plus::Template;
    use subprocess::Exec;

    pub fn key_val(txt: &str) -> anyhow::Result<(String, Attribute)> {
        let tokens = parser::tokenizer::get_tokens(txt);
        let attrs = parser::attrs::parse(tokens)?;
        attrs
            .into_iter()
            .map(|v| (v.0.to_string(), v.1))
            .next()
            .context("No values read")
    }

    /// Get environment variable from the shell
    ///
    /// ```task
    /// # will error if "HOME" is empty, as it can't assign None
    /// env.home = shell_env("HOME")
    /// ```
    #[env_func]
    fn shell_env(var: String) -> Option<String> {
        std::env::var(var).ok()
    }

    /// Set environment variable in the shell
    ///
    /// ```task
    /// env set_shell_env("testing", "true");
    /// env assert_eq(shell_env("testing"), "true")
    /// ```
    #[env_func]
    fn set_shell_env(var: String, val: String) {
        std::env::set_var(var, val)
    }

    /** Run the given template as a shell command.

    Run any command in the shell. The standard output of the command
    will be consumed and if there are lines starting with `nadi:var:`
    and followed by `key=val` pairs, it'll be read as new attributes
    to that node.

    For example if a command writes `nadi:var:name="Joe"` to stdout,
    then the for the current node the command is being run for, `name`
    attribute will be set to `Joe`. This way, you can write your
    scripts in any language and pass the values back to the NADI
    system.

    It will also print out the new values or changes from old values,
    if `verbose` is true.

    # Errors
    The function will error if,
    - The command template cannot be rendered,
    - The command cannot be executed,
    - The attributes from command's stdout cannot be parsed properly

    ```task
    network load_str("a -> b");
    node command("echo 'nadi:var:sth={NAME}'");
    node assert_eq(sth, NAME)
    ```

        */
    #[node_func(verbose = true, echo = false)]
    fn command(
        node: &mut NodeInner,
        /// String Command template to run
        cmd: &Template,
        /// Show the rendered version of command, and other messages
        verbose: bool,
        /// Echo the stdout from the command
        echo: bool,
    ) -> anyhow::Result<()> {
        let cmd = node.render(cmd)?;
        run_command_on_node(node, &cmd, verbose, echo)
    }

    /** Run the node as if it's a command if inputs are changed

    This function will not run a command node if all outputs are older
    than all inputs. This is useful to networks where each nodes are
    tasks with input files and output files.
    */
    #[node_func(verbose = true, echo = false)]
    fn run(
        node: &mut NodeInner,
        /// Node Attribute with the command to run
        command: &str,
        /// Node attribute with list of input files
        inputs: &str,
        /// Node attribute with list of output files
        outputs: &str,
        /// Print the command being run
        verbose: bool,
        /// Show the output of the command
        echo: bool,
    ) -> Result<(), String> {
        let cmd: String = node.try_attr(command)?;
        let inputs: Vec<String> = node.try_attr(inputs)?;
        let outputs: Vec<String> = node.try_attr(outputs)?;

        let latest_input = inputs
            .iter()
            .filter_map(|i| {
                let meta = std::fs::metadata(i).ok()?;
                let tm = filetime::FileTime::from_last_modification_time(&meta);
                Some(tm)
            })
            .max();
        let outputs: Option<Vec<_>> = outputs
            .iter()
            .map(|i| {
                let meta = std::fs::metadata(i).ok()?;
                let tm = filetime::FileTime::from_last_modification_time(&meta);
                Some(tm)
            })
            .collect();
        let run = if let Some(outs) = outputs {
            let oldest_output = outs.iter().min();
            latest_input.as_ref() > oldest_output
        } else {
            true
        };
        if run {
            run_command_on_node(node, &cmd, verbose, echo).map_err(|e| e.to_string())
        } else {
            Ok(())
        }
    }

    fn run_command_on_node(
        node: &mut NodeInner,
        cmd: &str,
        verbose: bool,
        echo: bool,
    ) -> anyhow::Result<()> {
        if verbose {
            println!("$ {cmd}");
        }
        let output = Exec::shell(cmd).stream_stdout()?;
        let buf = std::io::BufReader::new(output);
        for line in buf.lines() {
            let l = line?;
            if echo {
                println!("{}", l);
            }
            if let Some(line) = l.strip_prefix("nadi:var:") {
                let (k, v) = key_val(line)?;
                if verbose {
                    match node.attr(&k) {
                        Some(vold) => {
                            if !(vold == &v) {
                                println!("{k}={} -> {}", vold.to_string(), v.to_string())
                            }
                        }
                        None => println!("{k}={}", v.to_string()),
                    };
                }
                node.set_attr(&k, v);
            }
        }
        Ok(())
    }

    /** Run the given template as a shell command for each nodes in the network in parallel.

    Other than parallel execution this is same as the `node` function `command`

    ```task
    network load_str("a -> b");
    network parallel("echo 'nadi:var:sth={NAME}'");
    node assert_eq(sth, NAME)
    ```

    */
    #[network_func(workers = 16, verbose = true, echo = false)]
    fn parallel(
        net: &mut Network,
        /// String Command template to run
        cmd: &Template,
        /// Number of workers to run in parallel
        workers: i64,
        /// Print the command being run
        verbose: bool,
        /// Show the output of the command
        echo: bool,
    ) -> anyhow::Result<()> {
        let commands: Arc<Mutex<Vec<_>>> = Arc::new(Mutex::new(
            net.nodes()
                .enumerate()
                .map(|(i, n)| Ok((i, n.lock().render(cmd)?)))
                .collect::<Result<Vec<_>, anyhow::Error>>()?
                .into_iter()
                .rev()
                .collect(),
        ));

        // todo: put commands in a mutex, and then pop it from each
        // thread until it is exhausted to implement the number of
        // workers thing.

        let (tx, rx): (Sender<(usize, String)>, Receiver<(usize, String)>) = mpsc::channel();
        let mut children = Vec::new();

        for _ in 0..workers {
            let ctx = tx.clone();
            let cmd_lst = commands.clone();
            let child = thread::spawn(move || -> Result<(), anyhow::Error> {
                loop {
                    let cmd = cmd_lst
                        .lock()
                        .map_err(|e| anyhow::Error::msg(e.to_string()))?
                        .pop();
                    if let Some((i, cmd)) = cmd {
                        if verbose {
                            println!("$ {}", cmd.dimmed());
                        }
                        let output = Exec::shell(&cmd)
                            .stream_stdout()
                            .context(format!("Running: {cmd}"))?;
                        let buf = std::io::BufReader::new(output);
                        for line in buf.lines() {
                            let l = line?;
                            if echo {
                                println!("{}", l);
                            }
                            if let Some(line) = l.strip_prefix("nadi:var:") {
                                ctx.send((i, line.to_string()))?;
                            }
                        }
                    } else {
                        break;
                    }
                }
                Ok::<(), anyhow::Error>(())
            });
            children.push(child);
        }
        // since we cloned it, only the cloned ones are dropped when
        // the thread ends
        drop(tx);

        for (i, var) in rx {
            let mut node = net.node(i).unwrap().lock();
            let name = node.name();

            let (k, v) = match key_val(&var) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{:?}", e);
                    continue;
                }
            };
            if verbose {
                match node.attr(&k) {
                    Some(vold) => {
                        if !(vold == &v) {
                            println!("[{name}]\t{k}={vold:?} -> {v:?}")
                        }
                    }
                    None => println!("[{name}]\t{k}={v:?}"),
                };
            }
            node.set_attr(&k, v);
        }

        for child in children {
            child.join().expect("oops! the child thread panicked")?;
        }

        Ok(())
    }

    /** Run the given template as a shell command.

    Run any command in the shell. The standard output of the command
    will be consumed and if there are lines starting with `nadi:var:`
    and followed by `key=val` pairs, it'll be read as new attributes
    to the network. If you want to pass node attributes add node name
    with `nadi:var:name:` as the prefix for `key=val`.

    See `node command.command` for more details as they have
    the same implementation

    The examples below run `echo` command to set the variables, you
    can use any command that are scripting languages (python, R,
    Julia, etc) or individual programs.

    ```task
    network load_str("a -> b");
    network command("echo 'nadi:var:sth=123'");
    network assert_eq(sth, 123)
    network command("echo 'nadi:var:a:sth=123'");
    node[a] assert_eq(sth, 123)
    ```
     */
    #[network_func(verbose = true, echo = false)]
    fn command(
        net: &mut Network,
        /// String Command template to run
        cmd: Template,
        /// Print the command being run
        verbose: bool,
        /// Show the output of the command
        echo: bool,
    ) -> anyhow::Result<()> {
        let cmd = net.render(&cmd)?;
        if verbose {
            println!("$ {cmd}");
        }
        let output = Exec::shell(cmd).stream_stdout()?;
        let buf = std::io::BufReader::new(output);
        for line in buf.lines() {
            let l = line?;
            if echo {
                println!("{}", l);
            }
            if let Some(var) = l.strip_prefix("nadi:var:") {
                if let Some((node, var)) = var.split_once(":") {
                    // node attributes
                    if let Some(n) = net.node_by_name(node) {
                        let mut node = n.lock();
                        let (k, v) = key_val(var)?;
                        if verbose {
                            match node.attr(&k) {
                                Some(vold) => {
                                    if !(vold == &v) {
                                        println!("{k}={} -> {}", vold.to_string(), v.to_string())
                                    }
                                }
                                None => println!("{k}={}", v.to_string()),
                            };
                        }
                        node.set_attr(&k, v);
                    }
                } else {
                    // network attribute
                    let (k, v) = key_val(var)?;
                    if verbose {
                        match net.attr(&k) {
                            Some(vold) => {
                                if !(vold == &v) {
                                    println!("{k}={} -> {}", vold.to_string(), v.to_string())
                                }
                            }
                            None => println!("{k}={}", v.to_string()),
                        };
                    }
                    net.set_attr(&k, v);
                }
            }
        }
        Ok(())
    }
}
