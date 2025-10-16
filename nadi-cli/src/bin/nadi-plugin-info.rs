use nadi_core::abi_stable::library;
use nadi_core::abi_stable::package_version_strings;
use nadi_core::plugins::{NadiExternalPlugin_Ref, NadiPlugin};

// This is a simple binary that can read your plugin information

// The plan is for this to list the plugins, functions and such, but
// that won't work if there is ABI error, so need to figure that out.
fn main() {
    println!("Running Nadi Plugin CLI: {}", package_version_strings!());
    let args: Vec<String> = std::env::args().skip(1).collect();
    for arg in &args {
        let lib = match library::lib_header_from_path(arg.as_ref()) {
            Ok(l) => l,
            Err(e) => {
                println!("{e}");
                return;
            }
        };
        let version = lib.version_strings();
        let name = lib.root_mod_consts().name();
        println!("Path: {arg}");
        println!("Lib Name: {name}");
        println!("version: {version}");

        let rootmod = match lib.init_root_module::<NadiExternalPlugin_Ref>() {
            Ok(l) => {
                println!("Compatible: Yes");
                l
            }
            Err(e) => {
                println!("Compatible: No");
                println!("Compatibility Error: {e}");
                continue;
            }
        };
        println!("Plugin Name: {}", rootmod.name());
    }
}
