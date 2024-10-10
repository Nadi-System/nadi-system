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

pub fn load_library_safe(path: &Path) -> Option<NadiExternalPlugin_Ref> {
    load_library(path)
        .map_err(|e| {
            eprint!("Error loading {path:?}: ");
            print_library_err(e);
        })
        .ok()
}

fn check_library(path: &Path) -> Result<(), LibraryError> {
    let raw_library = abi_stable::library::RawLibrary::load_at(path)?;
    unsafe { abi_stable::library::lib_header_from_raw_library(&raw_library) }
        .and_then(|x| x.check_layout::<NadiExternalPlugin_Ref>())?;
    Ok(())
}

fn print_library_err(err: LibraryError) {
    match err {
	LibraryError::OpenError {
            path,
            ..
	} => eprintln!("Couln't open library {path:?}"),
	LibraryError::GetSymbolError {
            library,
            symbol,
            ..
	} => eprintln!("Plugin invalid {library:?} {symbol:?}"),
	LibraryError::ParseVersionError(_) => eprintln!("Error parsing version"),
	LibraryError::IncompatibleVersionNumber {
            library_name,
            expected_version,
            actual_version,
	} => eprintln!("Incompatible Versions: {library_name} expected {expected_version} got {actual_version}"),
	LibraryError::RootModule {
            module_name,
            version,
	    ..
	} => eprintln!("Plugin Error: {module_name:?} {version}"),
	LibraryError::AbiInstability(_) => eprintln!("ABI not stable"),
	LibraryError::InvalidAbiHeader(_) => eprintln!("Invalid Header"),
	LibraryError::InvalidCAbi {
            expected,
            found,
	} => eprintln!("C ABI Mismatch expected {expected} got {found}"),
	LibraryError::Many(errs) => for err in errs {
	    print_library_err(err);
	},
    }
}

fn _print_library_err_full(err: LibraryError) {
    match err {
	LibraryError::OpenError {
            path,
            err,
	} => eprintln!("Couln't open library {path:?} {err:?}"),
	LibraryError::GetSymbolError {
            library,
            symbol,
            err,
	} => eprintln!("Plugin invalid {library:?} {symbol:?} {err:?}"),
	LibraryError::ParseVersionError(e) => eprintln!("Error parsing version {e:?}"),
	LibraryError::IncompatibleVersionNumber {
            library_name,
            expected_version,
            actual_version,
	} => eprintln!("Incompatible Versions: {library_name} expected {expected_version} got {actual_version}"),
	LibraryError::RootModule {
            err,
            module_name,
            version,
	} => eprintln!("Plugin Error: {err:?} {module_name:?} {version}"),
	LibraryError::AbiInstability(e) => eprintln!("Abi Instable {e:?}"),
	LibraryError::InvalidAbiHeader(h) => eprintln!("Invalid Header {h:?}"),
	LibraryError::InvalidCAbi {
            expected,
            found,
	} => eprintln!("C ABI Mismatch expected {expected} got {found}"),
	LibraryError::Many(errs) => for err in errs {
	    print_library_err(err);
	},
    }
}
