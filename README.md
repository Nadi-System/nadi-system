# Nadi System

Collection of Utilities to do Network Analysis and Data Integration. 

# Installation
Clone this repository. And build it with cargo:

```bash
git clone https://github.com/Nadi-System/nadi-system
cd nadi-system
cargo build --release
```

The compiled binaries will be in `target/release` directory. You can run `nadi-ide` from there for GUI, or `nadi` for CLI.

Or you can directly do the following:
```bash
cargo run --release --bin nadi-ide
```

Nadi System consists of the following components:
## Nadi Core
Core library in Rust, it has the basic data structures and the logic for the plugin system.

## Nadi Plugin
This is a macro library that facilitates writing nadi plugins.

## Nadi CLI
Command Line Interface (CLI) to run nadi tasks file from terminal. Can visualize the parsed tasks, or run them.

## Nadi IDE
Integrated Development Environment (IDE) for Nadi tasks. You can use this for writing your nadi tasks script, browse documentations, visualize network, run tasks, etc. You do not need to install any other program if you want to use Nadi by itself if you install it.

## Nadi Python Library
You can install this and run nadi from python, you can use python syntax to define your own functions as well as run the functions loaded from nadi plugins. This gives better flexibility for research purposes, and for prototyping.

## Plugins
There are some plugins that are given by default called internal plugins. And some you can get from [`nadi-plugins-rust`](https://github.com/Nadi-System/nadi-plugins-rust) repository. 

## Nadi GIS
Geographic Information System (GIS) tool for nadi. It can help download stream lines (NHDPlus), USGS streamgages, basins, etc as well as run network detection algorithm for detecting network that is the backbone of nadi system.

Nadi GIS is available as a command line utility as well as a QGIS plugin.
