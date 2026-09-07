//! Library surface for the `tron` CLI crate.
//!
//!
// The crate's primary artefact is the `tron` binary in `bin/tron.rs`, but
//! integration tests in `tests/cli.rs` need to invoke `Cli::try_parse_from`
//! against argv strings — and a binary's modules are not visible to tests
//! unless the crate also exposes a library target. This file re-exports
//! exactly the surface the tests need; nothing else should leak through.

pub mod cli;
