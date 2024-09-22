# Nadi Core Library

The Core library for all the other nadi-system to use as well as for the plugins to use.

The core library (=nadi_core=) will contain the core data structures
and functions/methods to interact with the data structures.

For more info refer [documentation](https://docs.rs/nadi_core/latest/nadi_core/).

# Plugins

Plugins for nadi-system can be written using the nadi_core. 

For plugins:
- Include this crate as dependencies and use it for the data structures.
- Write plugin libraries as `cdylib` crate type so that it compiles to a shared library (`.so`, `.dll`, `.dynlib`, etc).
- Use the macros provided with this crate (reexported from `nadi_plugin` crate) to export the plugin and the functions in it.

For more details on the plugin development and examples, refer to [nadi-plugins-rust](https://github.com/Nadi-System/nadi-plugins-rust) repository.

# TODO
Write a book for the nadi system as a whole, concepts, examples, codes etc.
