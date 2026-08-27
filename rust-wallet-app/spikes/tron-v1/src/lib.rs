//! TRON spike V1–V10 verification harness (Issue #403).
//!
//! Each module is a thin verification primitive for one open question from
//! the TRON Rust SDK deep-dive (PR #402, Issue #399). Real implementation
//! lives in `crates/tron-wallet-core/` per the plan in
//! `docs/superpowers/plans/2026-08-27-tron-wallet-core.md`.

pub mod abi;
pub mod address;
pub mod base58check;
pub mod config;
pub mod keccak;
pub mod protobuf;
pub mod rpc;
pub mod spki;
pub mod tx;

pub mod proto {
    //! Re-export of generated protobuf types from `proto/core/Tron.proto`.
    //!
    //! `prost-build` names the generated file after the proto package
    //! (`protocol` in `Tron.proto`'s `package protocol;` directive).
    include!(concat!(env!("OUT_DIR"), "/protocol.rs"));
}
