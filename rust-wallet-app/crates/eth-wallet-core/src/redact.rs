//! Redact sensitive substrings (RPC URLs) from upstream error displays
//! before they reach user-visible log lines or operator stderr.
//!
//! Per L12 H-6 finding from PR #337 review: alloy's transport error
//! `Display` embeds the full RPC URL, which an attacker-controlled node
//! could weaponize for log poisoning or operator phishing. The same
//! concern applies to reqwest + custom transport errors. This helper
//! replaces `http://...` / `https://...` substrings with a fixed
//! `<rpc-url-redacted>` marker before the rest of the error reaches
//! the `Error::Rpc(String)` variant.

/// Redact `http://` / `https://` substrings from the `Display` of `e`,
/// returning a new `String` safe to surface in `Error::Rpc(...)` and
/// subsequent `println!` / `eprintln!` lines. Conservative — over-redacts
/// rather than under-redacts (skips up to next whitespace, quote, `)`, or
/// end-of-string).
pub fn redact_rpc_url(e: impl std::fmt::Display) -> String {
    let raw = e.to_string();
    let mut out = String::with_capacity(raw.len());
    let bytes = raw.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if (i + 7 <= bytes.len() && &bytes[i..i + 7] == b"http://")
            || (i + 8 <= bytes.len() && &bytes[i..i + 8] == b"https://")
        {
            out.push_str("<rpc-url-redacted>");
            // Skip past the scheme.
            i += if &bytes[i..i + 7] == b"http://" { 7 } else { 8 };
            // Skip until whitespace, quote, ')', or end.
            while i < bytes.len()
                && !bytes[i].is_ascii_whitespace()
                && bytes[i] != b'"'
                && bytes[i] != b'\''
                && bytes[i] != b')'
            {
                i += 1;
            }
        } else {
            // Push one UTF-8 char (could be multi-byte).
            let ch_end = i + 1;
            // Defensive: don't push partial multi-byte.
            out.push_str(std::str::from_utf8(&bytes[i..ch_end]).unwrap_or("?"));
            i = ch_end;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_https_url() {
        let s = redact_rpc_url("transport: https://eth.example.com/rpc");
        assert_eq!(s, "transport: <rpc-url-redacted>");
    }

    #[test]
    fn redacts_http_url() {
        let s = redact_rpc_url("error sending request: http://127.0.0.1:8545 failed");
        assert_eq!(s, "error sending request: <rpc-url-redacted> failed");
    }

    #[test]
    fn no_url_unchanged() {
        let s = redact_rpc_url("connection refused");
        assert_eq!(s, "connection refused");
    }

    #[test]
    fn multiple_urls_all_redacted() {
        let s = redact_rpc_url("from http://a.example to https://b.example");
        assert_eq!(s, "from <rpc-url-redacted> to <rpc-url-redacted>");
    }

    #[test]
    fn url_at_string_end() {
        let s = redact_rpc_url("at https://rpc.example/path");
        assert_eq!(s, "at <rpc-url-redacted>");
    }

    #[test]
    fn url_inside_quotes_redacted() {
        let s = redact_rpc_url("err: \"https://rpc.example/path\"");
        assert_eq!(s, "err: \"<rpc-url-redacted>\"");
    }
}
