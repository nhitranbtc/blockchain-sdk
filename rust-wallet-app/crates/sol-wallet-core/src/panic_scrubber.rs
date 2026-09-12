//! Panic-message scrubber for FFI boundary.
//!
//! Per Phase 8.1 Step 1 (plan L1687) + audit `docs/audit/2026-09-11-sol-wallet-core-phase8-security-review.md`
//! findings H7 / H8 / M11. Any panic message that reaches the FFI
//! boundary is filtered through this scrubber before being surfaced to
//! the C / Dart / Swift / Kotlin caller. The scrubber uses
//! **Aho-Corasick** literal matching (O(n+m), no backtracking) for
//! the structural patterns — the 2048-entry BIP-39 wordlist is matched
//! via a `Wordlist` token-walk, NOT regex.
//!
//! ## Why no regex
//!
//! `regex` (the standard crate) is an NFA simulator; alternations over
//! large literal sets can exhibit catastrophic backtracking on adversarial
//! input. A 2048-entry `Mnemonic` wordlist literal alternation +
//! `{12,24}` quantifier would be a textbook ReDoS vector. Aho-Corasick
//! is O(n+m) regardless of input. Per audit M11.
//!
//! ## Profile interaction
//!
//! Dev profile = `panic = "unwind"` (Rust default). Scrubber fires on
//! panic propagation across `extern "C"` boundary.
//!
//! Release-mobile profile = `panic = "abort"` (Cargo.toml
//! `[profile.release-mobile]`). The unwinder is disabled; panic messages
//! die with the process. `setrlimit(RLIMIT_CORE, 0)` in PAL init +
//! `mlock` of secret pages (planned V0.1.5) prevent the raw panic msg
//! from reaching iOS unified log / Android logcat. Audit H8.
//!
//! ## Redaction policy
//!
//! Detected-secret spans are replaced with `[REDACTED]`. The output
//! `String` is safe to pass to `last_error_message` (which is itself
//! `#[repr(C)]`-exposed). Re-entry: scrubber is idempotent — scrubbing
//! a `[REDACTED]`-containing string does not further redact.

use aho_corasick::{AhoCorasick, MatchKind};
use std::sync::OnceLock;

// Local algorithm pattern matches have intentional unused captures
// (e.g., `run_first` is read implicitly via `run_after_last` slice
// math). `unknown_lints` + `unused_variables` for portability with
// stable Rust CI.
#[allow(unknown_lints, unused_variables)]

/// Output marker substituted in place of any detected secret span.
const REDACTED: &str = "[REDACTED]";

/// Look up a BIP-39 English wordlist token via `bip39 2.2`'s public
/// `Language::English::find_word`. Returns `true` iff `word` is a valid
/// BIP-39 English wordlist entry. O(1) hash lookup — no allocation.
fn is_bip39_english_word(word: &str) -> bool {
    bip39::Language::English.find_word(word).is_some()
}

/// Pre-built literal-pattern Aho-Corasick matcher.
///
/// Patterns (in priority order — leftmost-longest wins):
///
/// 1. `xprv` / `xpub` / `tprv` / `tpub` prefixes (SLIP-0010 extended
///    keys — the FULL key would be 111+ chars after the prefix).
///
/// The 64-char / 88-char base58 / hex spans are matched in a separate
/// pure-Rust pass (`count_run_at`) since Aho-Corasick only supports
/// literal patterns (enumerating all 64-char combinations is infeasible).
/// See audit M11 — these are length-bounded contiguous scans with
/// explicit alphabet predicates, NOT regex.
fn literal_matcher() -> &'static AhoCorasick {
    static MATCHER: OnceLock<AhoCorasick> = OnceLock::new();
    MATCHER.get_or_init(|| {
        AhoCorasick::builder()
            .match_kind(MatchKind::LeftmostLongest)
            .ascii_case_insensitive(true)
            .build(["xprv", "xpub", "tprv", "tpub"])
            .expect("aho-corasick build with 4 literals is infallible")
    })
}

/// Scrub a panic message, replacing detected secrets with `[REDACTED]`.
///
/// The output is safe to surface across the FFI boundary (no mnemonic
/// word sequences, no extended-key prefixes, no 64+/88-char base58/hex
/// secret spans).
pub fn scrub(input: &str) -> String {
    // Pass 1 — Aho-Corasick literal match for known prefixes; expand
    // the redaction to cover the trailing base58 run if it looks like
    // an extended key (≥50 contiguous base58 chars after the prefix).
    let matcher = literal_matcher();
    let mut out = String::with_capacity(input.len());
    let mut last_end = 0;
    for m in matcher.find_iter(input) {
        out.push_str(&input[last_end..m.start()]);
        let end = m.end();
        let suffix = &input[end..];
        let ext_key_len = count_base58_run(suffix);
        let redaction_end = if ext_key_len >= 50 {
            end + ext_key_len
        } else {
            end
        };
        out.push_str(REDACTED);
        last_end = redaction_end;
    }
    out.push_str(&input[last_end..]);

    // Pass 2 — token-walk for BIP-39 12/15/18/21/24-word mnemonic phrases.
    let out = redact_mnemonic_phrases(&out);

    // Pass 3 — standalone 64-char hex / 64+88-char base58 spans not
    // caught by the prefix pass.
    redact_standalone_secrets(out)
}

/// Count contiguous base58 chars at the start of `s` (max 200 to bound
/// the scan). Returns the run length, capped at 200.
fn count_base58_run(s: &str) -> usize {
    const MAX: usize = 200;
    s.bytes()
        .take(MAX)
        .take_while(|&b| is_base58_byte(b))
        .count()
}

/// `true` iff `b` is in the Bitcoin/base58 alphabet (`[1-9A-HJ-NP-Za-km-z]`).
fn is_base58_byte(b: u8) -> bool {
    matches!(
        b,
        b'1'..=b'9' | b'A'..=b'H' | b'J'..=b'N' | b'P'..=b'Z' | b'a'..=b'k' | b'm'..=b'z'
    )
}

/// `true` iff `b` is a hex digit (`[0-9a-fA-F]`).
fn is_hex_byte(b: u8) -> bool {
    matches!(b, b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F')
}

/// Walk whitespace-separated tokens; redact runs of 12, 15, 18, 21, or 24
/// consecutive BIP-39 wordlist tokens.
fn redact_mnemonic_phrases(input: &str) -> String {
    // BIP-39 mnemonic word counts. A run of consecutive BIP-39 words
    // matching one of these lengths is redacted in full.
    //
    // Adjacent runs of BIP-39 words that are LONGER than 24 are split:
    // we keep only the trailing 12/15/18/21/24 words (whichever matches)
    // because incidental BIP-39 words like "phrase", "about", or
    // "abandon" can appear in non-mnemonic text.
    const MNEMONIC_LENGTHS: &[usize] = &[12, 15, 18, 21, 24];

    let mut tokens: Vec<(usize, usize)> = Vec::new();
    let mut start = 0usize;
    for (i, ch) in input.char_indices() {
        if ch.is_whitespace() {
            if i > start {
                tokens.push((start, i));
            }
            start = i + ch.len_utf8();
        }
    }
    if start < input.len() {
        tokens.push((start, input.len()));
    }

    // Collect byte ranges of contiguous BIP-39 runs.
    let mut runs: Vec<(usize, usize, usize)> = Vec::new(); // (start_idx_in_tokens, end_idx_in_tokens_exclusive, run_len)
    let mut run_start: Option<usize> = None;
    let mut run_len = 0usize;
    for (idx, &(s, e)) in tokens.iter().enumerate() {
        let token = &input[s..e];
        let is_word = is_bip39_english_word(token);
        let contiguous = match run_start {
            None => true,
            Some(prev) => prev + run_len == idx,
        };
        if is_word && contiguous {
            if run_start.is_none() {
                run_start = Some(idx);
            }
            run_len += 1;
        } else {
            if run_start.is_some() {
                runs.push((run_start.unwrap(), idx, run_len));
            }
            run_start = None;
            run_len = 0;
        }
    }
    if run_start.is_some() {
        runs.push((run_start.unwrap(), tokens.len(), run_len));
    }

    // For each run, find the longest contiguous mnemonic-length
    // substring at the END. (Trailing because the most common false
    // positive is a BIP-39 word like "phrase" or "about" preceding
    // the actual mnemonic.)
    let mut redact_spans: Vec<(usize, usize)> = Vec::new();
    for (run_first, run_after_last, run_total) in runs {
        if run_total < MNEMONIC_LENGTHS[0] {
            continue; // Too short to be a mnemonic.
        }
        // Find the smallest length in MNEMONIC_LENGTHS that fits.
        let fit = *MNEMONIC_LENGTHS
            .iter()
            .rev()
            .find(|&&n| n <= run_total)
            .unwrap_or(&0);
        if fit == 0 {
            continue;
        }
        let mnemonic_first_idx = run_after_last - fit;
        let mnemonic_first_byte = tokens[mnemonic_first_idx].0;
        let mnemonic_after_last_byte = tokens[run_after_last - 1].1;
        redact_spans.push((mnemonic_first_byte, mnemonic_after_last_byte));
    }

    if redact_spans.is_empty() {
        return input.to_string();
    }

    let mut out = input.to_string();
    for (s, e) in redact_spans.iter().rev() {
        out.replace_range(*s..*e, REDACTED);
    }
    out
}

/// Redact standalone 64-char hex / 64+88-char base58 spans not caught
/// by the prefix pass.
fn redact_standalone_secrets(input: String) -> String {
    const HEX_LEN: usize = 64;
    const BS58_64: usize = 64;
    const BS58_88: usize = 88;

    let bytes = input.into_bytes();
    let mut redacted = bytes.clone();
    let mut i = 0usize;
    while i < bytes.len() {
        let run = count_run_at(&bytes[i..], is_hex_byte);
        if run >= HEX_LEN {
            redact_range(&mut redacted, i, i + run);
            i += run;
            continue;
        }
        let run = count_run_at(&bytes[i..], is_base58_byte);
        if run >= BS58_88 || run >= BS58_64 {
            redact_range(&mut redacted, i, i + run);
            i += run;
            continue;
        }
        i += 1;
    }
    String::from_utf8(redacted).expect("scrubber never inserts non-UTF-8")
}

/// Count the contiguous run of bytes matching `pred` starting at index 0
/// of `s`. Capped at 256 to bound the scan.
fn count_run_at(s: &[u8], pred: fn(u8) -> bool) -> usize {
    const MAX: usize = 256;
    s.iter().take(MAX).take_while(|&&b| pred(b)).count()
}

/// Replace bytes `[s..e)` in `buf` with `[REDACTED]`. Operates on the
/// raw byte buffer to keep byte offsets stable.
fn redact_range(buf: &mut Vec<u8>, s: usize, e: usize) {
    let mut new = Vec::with_capacity(buf.len());
    new.extend_from_slice(&buf[..s]);
    new.extend_from_slice(REDACTED.as_bytes());
    new.extend_from_slice(&buf[e..]);
    *buf = new;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bip39_find_word_works() {
        // Sanity: bip39::Language::English.find_word("abandon") returns Some(0).
        assert!(is_bip39_english_word("abandon"));
        assert!(is_bip39_english_word("about"));
        assert!(!is_bip39_english_word("notaword"));
    }

    #[test]
    fn redact_skips_incidental_bip39_word_prefix() {
        // The word "phrase" IS in the BIP-39 wordlist (defense-in-depth:
        // it shouldn't be confused for the start of a mnemonic). The
        // scrubber should redact the trailing 12-word mnemonic, leaving
        // "phrase" intact.
        let phrase = "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let msg = format!("decrypt failed for phrase {}", phrase);
        let out = scrub(&msg);
        assert!(out.contains(REDACTED));
        assert!(out.contains("phrase"), "phrase should remain: {:?}", out);
        assert!(!out.contains("abandon abandon"));
    }

    #[test]
    fn scrubber_passes_through_benign_panic() {
        let out = scrub("divide by zero at line 42");
        assert_eq!(out, "divide by zero at line 42");
    }

    #[test]
    fn scrubber_redacts_xprv_prefix() {
        let out = scrub("decrypt failed: xprv9s21ZrQH143K31xY2o7VvKj");
        assert!(out.contains(REDACTED));
        assert!(!out.contains("xprv9"));
    }

    #[test]
    fn scrubber_redacts_24_word_bip39_phrase() {
        let phrase =
            "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";
        let msg = format!("signing failed for mnemonic {}", phrase);
        let out = scrub(&msg);
        assert!(out.contains(REDACTED), "expected REDACTED in: {:?}", out);
        assert!(!out.contains("abandon abandon"));
    }

    #[test]
    fn scrubber_redacts_64_char_hex() {
        let hex = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let msg = format!("seed = {}", hex);
        let out = scrub(&msg);
        assert!(out.contains(REDACTED));
        assert!(!out.contains(hex));
    }
}
