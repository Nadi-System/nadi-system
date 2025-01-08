use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod command {
    use crate::parser;
    use crate::prelude::*;
    use anyhow::Context;
    use colored::Colorize;
    use nadi_core::nadi_plugin::{network_func, node_func};
    use std::io::BufRead;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::thread;
    use string_template_plus::Template;
    use subprocess::Exec;

    pub fn key_val(txt: &str) -> anyhow::Result<(String, Attribute)> {
        let tokens = parser::tokenizer::get_tokens(&txt)?;
        let attrs = parser::attrs::parse(tokens)?;
        attrs
            .into_iter()
            .map(|v| (v.0.to_string(), v.1))
            .next()
            .context("No values read")
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

    # Arguments
    - `cmd`: String Command template to run
    - `verbose`: bool Show the rendered version of command
    - `echo`: bool Echo the stdin from the command

    # Errors
    The function will error if,
    - The command template cannot be rendered,
    - The command cannot be executed,
    - The attributes from command's stdout cannot be parsed properly
        */
    #[node_func(verbose = true, echo = false)]
    fn command(
        node: &mut NodeInner,
        cmd: &Template,
        verbose: bool,
        echo: bool,
    ) -> anyhow::Result<()> {
        let cmd = node.render(cmd)?;
        run_command_on_node(node, &cmd, verbose, echo)
    }

    /** Run the node as if it's a command if inputs are changed

    This function will not run a command node if all outputs are older
    than all inputs. This is useful to networks where each nodes are
    tasks with input files and output files.

    # Arguments
    - `command`: Node Attribute with the command to run
    - `inputs`: Node attribute with list of input files
    - `outputs`: Node attribute with list of output files
    - `verbose`: print the command being run
    - `echo`: Show the output of the command
    */
    #[node_func(verbose = true, echo = false)]
    fn run(
        node: &mut NodeInner,
        command: &str,
        inputs: &str,
        outputs: &str,
        verbose: bool,
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

    # Warning
    Currently there is no way to limit the number of parallel
    processes, so please be careful with this command if you have very
    large number of nodes.

    # Arguments
    - `cmd`: String Command template to run
    - `workers`: Integer Number of workers to run in parallel
    - `verbose`: bool Show the rendered version of command and variable changes
    - `echo`: bool Echo the stdin from the command
     */
    #[network_func(_workers = 4, verbose = true, echo = false)]
    fn parallel(
        net: &mut Network,
        cmd: &Template,
        _workers: i64,
        verbose: bool,
        echo: bool,
    ) -> anyhow::Result<()> {
        let commands: Vec<_> = net
            .nodes()
            .map(|n| n.lock().render(cmd))
            .collect::<Result<Vec<_>, anyhow::Error>>()?;
        let (tx, rx): (Sender<(usize, String)>, Receiver<(usize, String)>) = mpsc::channel();
        let mut children = Vec::new();

        for (i, cmd) in commands.into_iter().enumerate() {
            let ctx = tx.clone();
            let child = thread::spawn(move || -> Result<(), anyhow::Error> {
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
    to that node.

    See `node command.command` for more details as they have
    the same implementation

    # Arguments
    - `cmd`: String Command template to run
    - `verbose`: bool Show the rendered version of command
    - `echo`: bool Echo the stdin from the command
     */
    #[network_func(verbose = true, echo = false)]
    fn command(net: &mut Network, cmd: Template, verbose: bool, echo: bool) -> anyhow::Result<()> {
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
        Ok(())
    }
}
