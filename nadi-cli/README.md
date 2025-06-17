# CLI Tool for nadi
Documentation: https://nadi-system.github.io/

This can run nadi task files, syntax highlight them for verifying them, generate markdown documentations for the plugins. The documentations included in the nadi book ([Function List](https://nadi-system.github.io/plugins/index.html) and each plugin's page like [Attributes Plugin `attributes`](https://nadi-system.github.io/plugins/attributes.html)) are generated with that. The documentation on each plugin functions comes from their docstrings in the code, please refer to how to write plugins section of the book for details on that.

The REPL is available with `nadi --repl` option. Refer to [Examples](https://nadi-system.github.io/learn-examples.html) for example nadi scripts.


The available options are shown below.

```
Usage: nadi [OPTIONS] [TASK_FILE]

Arguments:
  [TASK_FILE]  Tasks file to run; if `--stdin` is also provided this runs before stdin

Options:
  -C, --completion <FUNC_TYPE>   list all functions and exit for completions [possible values: node, network, env]
  -c, --fncode <FUNCTION>        print code for a function
  -f, --fnhelp <FUNCTION>        print help for a function
  -g, --generate-doc <DOC_DIR>   Generate markdown doc for all plugins and functions
  -l, --list-functions           list all functions and exit
  -n, --network <NETWORK_FILE>   network file to load before executing tasks
  -p, --print-tasks              print tasks before running
  -P, --new-plugin <NEW_PLUGIN>  Create the files for a new nadi_plugin
  -N, --nadi-core <NADI_CORE>    Path to the nadi_core library for the new nadi_plugin
  -s, --show                     Show the tasks file, do not do anything
  -S, --stdin                    Use stdin for the tasks; reads the whole stdin before execution
  -r, --repl                     Open the REPL (interactive session) before exiting
  -t, --task <TASK_STR>          Run given string as task before running the file
  -h, --help                     Print help
  -V, --version                  Print version
```
