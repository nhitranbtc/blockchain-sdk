extern crate cbindgen;

use std::env;
use std::path::Path;

fn main() {
    let crate_dir = env::var("CARGO_MANIFEST_DIR").unwrap();
    let out_dir = env::var("OUT_DIR").unwrap();
    let out_path = Path::new(&out_dir).join("ffi.h");

    // For now: only regenerate ffi.h when explicitly requested via
    // BTC_WALLET_CORE_FFI_GEN=1. The spike only needs the file to exist;
    // full codegen happens in Phase 1 Task 2.
    if env::var("BTC_WALLET_CORE_FFI_GEN").is_ok() {
        let config = cbindgen::Config::from_file(Path::new(&crate_dir).join("cbindgen.toml"))
            .unwrap_or_else(|_| cbindgen::Config::default());
        cbindgen::Builder::new()
            .with_crate(&crate_dir)
            .with_config(config)
            .generate()
            .expect("Unable to generate bindings")
            .write_to_file(&out_path);
        println!("cargo:rerun-if-changed=src/ffi");
    }
}
