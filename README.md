# Nadi System

Collection of Utilities to do Network Analysis and Data Integration. This is made targeting analysis of point data in a river system but it should work for any network analysis that work on directed tree network.

The main component of the NADI System is the DSL. It can be used for network based data analysis. The repo contains the tools for using nadi through Command Line Interface (CLI), Integrated Development Environment (IDE), `mdbook` (documentation writing tool), or as a python library.

The overall NADI workflow is shown below. This repository contains the components for the Network Analysis part of the system.

![Nadi Workflow](nadi-workflow-simple.svg)

## Theory
The data associated with points in a river system is loaded as a network, and along side the functions loaded through the plugins, the Domain Specific Programming language (DSL) can be used to run different arithmetic, logical, or functional analysis on the metadata of the points.

This allows us to do network based data analysis quickly, with intuitive syntax compared to using general purpose programming language like Python. But the general purpose languages have more flexibility, so a way to couple with them is provided through the plugin mechanism.

## Further Reading
Please refer to the [NADI Book](https://nadi-system.github.io/) for details on the key concepts, the syntax of the language, and other components of the NADI System.

For developers the API documentation is on [docs.rs](https://docs.rs/nadi_core/latest/nadi_core/index.html).

# Usage Instructions

Video Demo: https://www.youtube.com/watch?v=qKsrigRrPKo
Web User Manual: https://nadi-system.github.io
PDF User Manual: https://nadi-system.github.io/data/nadi-book.pdf
Dev Reference: https://docs.rs/nadi_core/latest/nadi_core/

# Installation

Prebuilt binaries are available for windows in the releases page. Use the `nadi-ide` binary for the GUI, refer to Nadi Book for details on other binaries.

![Nadi IDE Screenshot](ide-screenshot.png)

If you want to build it from source, which works on Linux, Windows, MacOS and Android (Termux, but without the IDE), clone this repository. And build it with cargo as follows:

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

# Tests
You can run tests using the cargo command, this will run the tests in all the components as well as their documentation.

```bash
cargo test
```

## Components
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

# Contributing
You can contribute to NADI System even without coding experience in Rust by reporting bugs, suggesting features, and helping with documentation.

Please refer to [CONTRIBUTING.md](./CONTRIBUTING.md) for further details on specific roles and tasks you can do.

# Future Work
- Implement timeseries data types with gaps
- Add syntax to access timeseries data directly,
  + `$` for series, and `$$` for timeseries might work, with `$` also
    falling back to get series data without time from timeseries, if
    there is no series with that name.
  + Also means we have to add a generic data type for task system, enum of (Attribute, Series, Timeseries, etc)
- Only internal plugins can be run without dot syntax. All external plugins need the plugin name to access the functions.
- Add user defined functions. Maybe only environmental function for now? See if node/network function can be done easily.
- Add for loop, for looping through Array and Table Keys.
- Implement `.0` type syntax for array member access like with attrmap member access.
- Write a `nadi-server` CLI tool, that opens a server. It opens up an API where users can send tasks to run, and it can return the output from that. It should have `mutable` and `immutable` option. In the immutable option users can only run immutable functions.
- Write editor modes for nadi, maybe using `lsp` so users can expand it to their editors with minimal work.
  - Currently, besides the NADI IDE, there are some syntax highlight available for web (through highlight.js), and sublime syntax files in `extra/syntax-highlight` directory.
  - There is no intelligent analysis of scripts, you can only run it to get errors. Better error handling while parsing, and then 

