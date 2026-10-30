//! The documented table in `docs/wallets/CONTEXT.md` IS the specification.
//!
//! This test parses the "Common Error Categories (cross-chain)" table out of
//! the document and asserts it matches `wallet_errors::ErrorCategory`
//! exactly — same rows, same order, same labels, same exit codes.
//!
//! The document lives at `<repo>/docs/wallets/CONTEXT.md`; from this crate's
//! manifest directory that is three levels up.

use std::path::PathBuf;

use wallet_errors::ErrorCategory;

fn context_md_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../docs/wallets/CONTEXT.md")
}

fn documented_rows() -> Vec<(String, i32)> {
    let text = std::fs::read_to_string(context_md_path()).expect("read CONTEXT.md");
    let mut rows = Vec::new();
    let mut in_table = false;
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("| Category | Exit code") {
            in_table = true;
            continue;
        }
        if in_table {
            if line.starts_with('|') {
                let mut cells = line.split('|').map(str::trim);
                cells.next();
                let Some(label) = cells.next() else { continue };
                let Some(code) = cells.next() else { continue };
                if label.is_empty() || !code.chars().all(|c| c.is_ascii_digit()) {
                    continue;
                }
                rows.push((label.to_string(), code.parse().expect("numeric exit code")));
            } else if !line.is_empty() {
                in_table = false;
            }
        }
    }
    rows
}

#[test]
fn context_md_table_matches_the_code_table() {
    let documented = documented_rows();
    assert_eq!(
        documented.len(),
        ErrorCategory::ALL.len(),
        "CONTEXT.md must document exactly one row per ErrorCategory"
    );
    for (category, (label, code)) in ErrorCategory::ALL.iter().zip(documented.iter()) {
        assert_eq!(category.as_str(), label, "category label drift");
        assert_eq!(category.exit_code(), *code, "exit code drift for {label}");
    }
}

#[test]
fn exit_codes_are_the_documented_six() {
    assert_eq!(ErrorCategory::Success.exit_code(), 0);
    assert_eq!(ErrorCategory::User.exit_code(), 1);
    assert_eq!(ErrorCategory::Network.exit_code(), 2);
    assert_eq!(ErrorCategory::InsufficientFunds.exit_code(), 3);
    assert_eq!(ErrorCategory::Signing.exit_code(), 4);
    assert_eq!(ErrorCategory::Broadcast.exit_code(), 5);
}

#[test]
fn display_prints_the_documented_label() {
    assert_eq!(ErrorCategory::User.to_string(), "User error");
}
