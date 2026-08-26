//! Issue #351 (cycle 8b, C-1 from #339) — non-TTY fallback verification.
//!
//! `rpassword::prompt_password` (used by `eth`'s `prompt_password()`
//! helper) reads from `/dev/tty` directly in production. In a CI runner
//! without a controlling terminal, the underlying `open` fails with an
//! `io::Error`. We must NOT panic — the chain in `resolve_password_with`
//! maps the error to `Error::InvalidInput` so the CLI surfaces a clean
//! operator-facing message and exits 2.
//!
//! These tests exercise rpassword's documented test seam
//! (`read_password_from_bufread`) to lock in the "no panic on IO
//! failure" property. They are documentation/contract tests against
//! the rpassword crate we depend on — keep them across rpassword
//! version bumps so a silent upgrade that reintroduces a panic surface
//! gets caught here.
//!
//! NOTE: `read_password_from_bufread` is marked deprecated in rpassword
//! 7.x (replaced by `read_password_with_config` + `ConfigBuilder`).
//! The deprecation is for an upcoming 8.x — keep using the deprecated
//! test seam here until the production code (`prompt_password`) also
//! migrates; the test seam stays meaningful as long as the underlying
//! IO contract holds.
#![allow(deprecated)]

use std::io::{BufRead, BufReader, Cursor, Read};

struct FailReader;

impl Read for FailReader {
    fn read(&mut self, _: &mut [u8]) -> std::io::Result<usize> {
        Err(std::io::Error::other("no tty available"))
    }
}

impl BufRead for FailReader {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        Err(std::io::Error::other("no tty available"))
    }
    fn consume(&mut self, _: usize) {}
}

#[test]
fn read_password_from_bufread_propagates_io_error_without_panic() {
    // Mirrors the production CI-runner condition: no /dev/tty. The
    // rpassword reader must return Err rather than panic.
    let mut input = BufReader::new(FailReader);
    let result = rpassword::read_password_from_bufread(&mut input);
    assert!(
        result.is_err(),
        "rpassword reader must return Err on IO failure, got: {result:?}"
    );
}

#[test]
fn read_password_from_bufread_returns_input() {
    // Happy-path contract: a buffered reader delivers the typed bytes
    // (echo is not part of the buffered-reader surface — that's a TTY-
    // mode property exercised by `prompt_password` against `/dev/tty`).
    let mut input = BufReader::new(Cursor::new(b"hunter2\n".to_vec()));
    let pw =
        rpassword::read_password_from_bufread(&mut input).expect("happy-path read should succeed");
    assert_eq!(pw, "hunter2");
}
