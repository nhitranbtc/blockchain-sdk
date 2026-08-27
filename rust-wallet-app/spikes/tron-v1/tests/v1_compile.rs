//! V1 — compile gate.
//!
//! Plan §Q1: `cargo add prost@0.14 prost-types@0.14 bs58@0.5 tiny-keccak@2.0.2` compiles against
//! workspace. PASS = this file compiles + cargo build -p tron-v1-spike exits 0. The
//! `v1_compile_smoke` assertion below is a trivially-true runtime check that ensures the
//! test binary itself is linked + the build script ran (prost-build generated types).
//!
//! `protoc ≥ 3.12` must be in PATH at build time (CI install dep — see README).

#[test]
fn v1_compile_smoke() {
    // If this binary runs, the build pipeline succeeded: prost-build compiled the
    // vendored proto/core/Tron.proto → Rust types are linked into the spike binary.
    // Trivial assertion kept so the test framework reports a result.
    let _linker_ok = true;
    assert!(_linker_ok);
}

#[test]
fn v1_prost_generated_types_visible() {
    // Generated `proto` module from `proto/core/Tron.proto` (SHA 851575d) is reachable
    // through the spike crate's re-export. `Block` is one of the top-level messages
    // in core/Tron.proto — if codegen ran, `proto::Block` resolves.
    use tron_v1_spike::proto;
    let _type_check: Option<proto::Block> = None;
}

#[test]
fn v1_workspace_deps_resolve() {
    // All four NEW workspace deps must resolve at compile time per plan §B.
    use bs58;
    use prost;
    use prost_types;
    use sha2::{Digest, Sha256};
    use tiny_keccak::{Hasher, Keccak};

    // bs58 round-trip sanity.
    let encoded = bs58::encode([0x42u8]).into_string();
    assert_eq!(bs58::decode(&encoded).into_vec().unwrap(), vec![0x42]);

    // prost::Message trait must be in scope for Q2 round-trip tests.
    fn _assert_message<T: prost::Message>() {}
    _assert_message::<tron_v1_spike::proto::Block>();

    // prost-types well-known types must be reachable.
    let _ts = prost_types::Timestamp {
        seconds: 0,
        nanos: 0,
    };

    // sha2 + tiny-keccak (Keccak-256, NOT SHA3-256) must both be reachable.
    let _ = Sha256::digest(b"x");
    let mut h = Keccak::v256();
    h.update(b"x");
    let mut out = [0u8; 32];
    h.finalize(&mut out);
}
