//! Build script — emits `sol_wallet_core.h` via `cbindgen` at build time.
//!
//! Phase 8.2 Step 3 + audit Step 11a M15. Mobile Dart/Swift/Kotlin
//! consumers bind to the emitted header. Constants emitted alongside
//! the function declarations:
//!
//!   - `SOL_WALLET_MNEMONIC_BUF_LEN = 256` (24-word mnemonic + NUL)
//!   - `SOL_WALLET_SECRET_LEN = 32` (Ed25519 secret seed)
//!   - `SOL_WALLET_PUBKEY_LEN = 44` (base58 Ed25519 pubkey)
//!   - `SOL_WALLET_SIGNATURE_LEN = 64` (Ed25519 signature)
//!   - `SOL_WALLET_HASH_LEN = 32` (32-byte message hash input)
//!
//! The header is regenerated on every `cargo build` of the cdylib.
//! CI commits the generated `sol_wallet_core.h` to the repo root and
//! diffs against it on subsequent builds (audit L19 fix).

fn main() {
    let crate_dir = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let config_path = std::path::Path::new(&crate_dir).join("cbindgen.toml");
    let header_path = std::path::Path::new(&crate_dir).join("sol_wallet_core.h");

    let config = cbindgen::Config::from_file(&config_path)
        .unwrap_or_else(|e| panic!("cbindgen: failed to load {} — {}", config_path.display(), e));

    let bindings = cbindgen::Builder::new()
        .with_crate(crate_dir)
        .with_config(config)
        .generate()
        .expect("cbindgen: failed to generate C bindings");

    bindings.write_to_file(&header_path);

    // Rerun-if-changed: rebuild header when sources, cbindgen.toml, or
    // the build.rs itself change.
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=cbindgen.toml");
    println!("cargo:rerun-if-changed=src/ffi.rs");
    println!("cargo:rerun-if-changed=src/error.rs");
    println!("cargo:rerun-if-changed=src/wallet_manager.rs");
    println!("cargo:rerun-if-changed=src/wallet.rs");
    println!("cargo:rerun-if-changed=src/persist.rs");
    println!("cargo:rerun-if-changed=src/crypto.rs");
}
