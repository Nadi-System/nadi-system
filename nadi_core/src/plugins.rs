use std::path::Path;

use crate::functions::NadiFunctions;

use abi_stable::library::LibraryError;

use abi_stable::{declare_root_module_statics, package_version_strings};
use abi_stable::{
    library::RootModule, sabi_types::version::VersionStrings, std_types::RString, StableAbi,
};

#[repr(C)]
#[derive(StableAbi)]
#[sabi(kind(Prefix))]
pub struct NadiExternalPlugin {
    pub register_functions: extern "C" fn(&mut NadiFunctions),
    pub plugin_name: extern "C" fn() -> RString,
}

pub trait NadiPlugin {
    fn register(&self, func: &mut NadiFunctions);
    fn name(&self) -> RString;
}

impl NadiPlugin for NadiExternalPlugin_Ref {
    fn register(&self, func: &mut NadiFunctions) {
        self.register_functions().unwrap()(func);
    }
    fn name(&self) -> RString {
        self.plugin_name().unwrap()()
    }
}

impl RootModule for NadiExternalPlugin_Ref {
    // The name of the dynamic library
    const BASE_NAME: &'static str = "nadi_plugins";
    // The name of the library for logging and similars
    const NAME: &'static str = "nadi_plugins";
    // The version of this plugin's crate
    const VERSION_STRINGS: VersionStrings = package_version_strings!();

    // Implements the `Rootule::root_module_statics` function, which is the
    // only required implementation for the `Rootule` trait.
    declare_root_module_statics! {NadiExternalPlugin_Ref}
}

pub fn load_library(path: &Path) -> Result<NadiExternalPlugin_Ref, LibraryError> {
    check_library(path)?;
    abi_stable::library::lib_header_from_path(path)
        .and_then(|x| x.init_root_module::<NadiExternalPlugin_Ref>())
    // the following returns the first one on repeat call
    // NadiExternalPlugin_Ref::load_from_file(path)
}

fn check_library(path: &Path) -> Result<(), LibraryError> {
    let raw_library = abi_stable::library::RawLibrary::load_at(path)?;
    unsafe { abi_stable::library::lib_header_from_raw_library(&raw_library) }
        .and_then(|x| x.check_layout::<NadiExternalPlugin_Ref>())?;
    Ok(())
}
