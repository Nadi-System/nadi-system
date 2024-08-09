// use std::{env, path::PathBuf};

fn main() {
    let version = rustc_version::version().unwrap();
    println!("cargo:rustc-env=RUSTC_VERSION={}", version);

    // commented out as the cargo publish fails with this:
    //
    // Use `cbindgen --cpp-compat --lang=C` to generate the header files

    // let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    // let out_dir = env::var_os("OUT_DIR").unwrap();
    // let mut config: cbindgen::Config = Default::default();
    // config.language = cbindgen::Language::C;
    // config.cpp_compat = true;

    // cbindgen::Builder::new()
    //     .with_crate(crate_dir)
    //     .with_config(config)
    //     .generate()
    //     .expect("Unable to generate bindings")
    //     .write_to_file(PathBuf::from(out_dir).join("nadi_core.h"));
}
