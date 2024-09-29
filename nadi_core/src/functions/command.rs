use nadi_plugin::nadi_internal_plugin;

#[nadi_internal_plugin]
mod command {
    use anyhow::Context;
    use colored::Colorize;
    use nadi_core::nadi_plugin::{network_func, node_func};
    use nadi_core::{Network, NodeInner};
    use std::io::BufRead;
    use std::sync::mpsc::{self, Receiver, Sender};
    use std::thread;
    use string_template_plus::Template;
    use subprocess::Exec;

    /** Run the given template as a shell command.

    Run any command in the shell. The standard output of the command
    will be consumed and if there are lines starting with `nadi:var:`
    and followed by `key=val` pairs, it'll be read as new attributes
    to that node.

    # Arguments:
    - cmd: String Command template to run
    - verbose: bool Show the rendered version of command
    - echo: bool Echo the stdin from the command
        */
    #[node_func(verbose = true, echo = true)]
    fn command(
        node: &mut NodeInner,
        cmd: Template,
        verbose: bool,
        echo: bool,
    ) -> anyhow::Result<()> {
        let cmd = node.render(&cmd)?;
        if verbose {
            println!("$ {cmd}");
        }
        let output = Exec::shell(cmd).stream_stdout()?;
        let buf = std::io::BufReader::new(output);
        for line in buf.lines() {
            let l = line?;
            if l.starts_with("nadi:var:") {
                let var = &l["nadi:var:".len()..];
                let (res, (k, v)) = nadi_core::parser::attrs::attr_key_val(var)
                    .map_err(|e| anyhow::Error::msg(e.to_string()))?;
                match node.attr(&k) {
                    Some(vold) => {
                        if !(vold == &v) {
                            println!("{k}={} -> {}", vold.to_string(), v.to_string())
                        }
                    }
                    None => println!("{k}={}", v.to_string()),
                };
                node.set_attr(&k, v);
            } else {
                if echo {
                    println!("{}", l);
                }
            }
        }
        Ok(())
    }

    /** Run the given template as a shell command for each nodes in the network in parallel.

    Arguments:
    - cmd: String Command template to run
    - workers: Integer Number of workers to run in parallel
    - verbose: bool Show the rendered version of command
    - echo: bool Echo the stdin from the command

    Run any command in the shell. The standard output of the command will
        be ignored. Use the node function `command` for more control.
        */
    #[network_func(workers = 4, verbose = true, echo = true)]
    fn parallel(
        net: &mut Network,
        cmd: Template,
        workers: i64,
        verbose: bool,
        echo: bool,
    ) -> anyhow::Result<()> {
        let commands: Vec<_> = net
            .nodes()
            .map(|n| n.lock().render(&cmd))
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
                    if l.starts_with("nadi:var:") {
                        ctx.send((i, l["nadi:var:".len()..].to_string()))?;
                    } else {
                        if echo {
                            println!("{}", l);
                        }
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

            let (res, (k, v)) = match nadi_core::parser::attrs::attr_key_val(&var) {
                Ok(v) => v,
                Err(e) => {
                    eprintln!("{:?}", e);
                    continue;
                }
            };
            match node.attr(&k) {
                Some(vold) => {
                    if !(vold == &v) {
                        println!("[{name}]\t{k}={vold:?} -> {v:?}")
                    }
                }
                None => println!("[{name}]\t{k}={v:?}"),
            };
            node.set_attr(&k, v);
        }

        for child in children {
            child.join().expect("oops! the child thread panicked")?;
        }

        Ok(())
    }
}
