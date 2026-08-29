//! `polygon config` handlers — Issue #426 / T6d-3.
//!
//! Per `docs/superpowers/plans/2026-08-28-polygon-cli-interface-design.md`
//! §5.8 + §6.x. Batch G (TDD): `config_show`.
//!
//! Pure resolution — no RPC, no signing. Reads env vars + CLI flags
//! and prints the resolved configuration. The RPC URL is **redacted**
//! if it contains credentials (matches `eth` CLI's `redact_rpc_url`
//! pattern at `eth/src/handlers.rs:983-1030`).

use std::path::PathBuf;

/// Redact credentials from an RPC URL if present.
///
/// `https://user:pass@host.example.com/rpc` → `https://***:***@host.example.com/rpc`.
/// Idempotent: re-redacting a redacted URL is a no-op (the `***:***`
/// marker contains no further credentials to leak).
///
/// **RFC 3986 compliance:** per §3.2.1, the userinfo sub-component is
/// terminated by the first `@` that appears before the first `/` (or
/// end of string if no path). `@` in path / query / fragment must NOT
/// be treated as the userinfo delimiter (paths are not userinfo, and
/// passwords cannot legally contain raw `@` — it would be percent-
/// encoded as `%40`). The algorithm therefore looks for `@` only in
/// the "authority" segment (between `://` and the first `/`).
pub fn redact_rpc_url(url: &str) -> String {
    let Some(scheme_end) = url.find("://") else {
        return url.to_string();
    };
    let after_scheme = &url[scheme_end + 3..];
    // Authority = up to first '/' (or end of string if no path).
    let authority_end = after_scheme.find('/').unwrap_or(after_scheme.len());
    let authority = &after_scheme[..authority_end];
    // Userinfo = up to last '@' in authority (user:pass@host form).
    let Some(at_idx) = authority.rfind('@') else {
        return url.to_string();
    };
    let prefix = &url[..=scheme_end + 2]; // includes "://"
    let host = &authority[at_idx..];
    let path = &after_scheme[authority_end..];
    format!("{prefix}***:***{host}{path}")
}

/// T6d-3 handler: `polygon config show [--json]`.
///
/// Per design doc §5.8. Returns the formatted output (text or JSON)
/// as a `String` so unit tests can verify the structure + redaction
/// without capturing stdout. The `main.rs` dispatch prints the
/// returned value. Returns `Result<()>` for forward compatibility
/// (future redaction failures + network resolution can be added
/// without a signature change).
pub fn config_show(
    rpc_url: Option<&str>,
    data_dir: Option<&PathBuf>,
    json: bool,
) -> polygon_wallet_core::Result<String> {
    let resolved_rpc = rpc_url
        .map(redact_rpc_url)
        .unwrap_or_else(|| "<default-amoy-rpc>".into());
    let resolved_dir: String = data_dir
        .map(|p| p.display().to_string())
        .unwrap_or_else(|| "<XDG_DATA_HOME/polygon>".into());
    let resolved_pin = "<T8 reserved>";

    let out = if json {
        let obj = serde_json::json!({
            "network": "amoy",
            "rpc_url": resolved_rpc,
            "data_dir": resolved_dir,
            "pin_spki": resolved_pin,
        });
        obj.to_string()
    } else {
        format!(
            "network: amoy\nrpc_url: {resolved_rpc}\ndata_dir: {resolved_dir}\npin_spki: {resolved_pin}\n"
        )
    };
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::{config_show, redact_rpc_url};

    #[test]
    fn redact_rpc_url_strips_basic_credentials() {
        let redacted = redact_rpc_url("https://user:pass@rpc.example.com/path");
        assert_eq!(redacted, "https://***:***@rpc.example.com/path");
        assert!(!redacted.contains("user"));
        assert!(!redacted.contains("pass"));
    }

    #[test]
    fn redact_rpc_url_passthrough_no_credentials() {
        let url = "https://rpc.example.com/path";
        assert_eq!(redact_rpc_url(url), url);
    }

    #[test]
    fn redact_rpc_url_idempotent() {
        let once = redact_rpc_url("https://user:pass@rpc.example.com/path");
        let twice = redact_rpc_url(&once);
        assert_eq!(once, twice, "redaction must be idempotent");
    }

    #[test]
    fn redact_rpc_url_handles_user_only() {
        let redacted = redact_rpc_url("https://token@rpc.example.com/path");
        assert_eq!(redacted, "https://***:***@rpc.example.com/path");
        assert!(!redacted.contains("token"));
    }

    #[test]
    fn redact_rpc_url_passthrough_no_scheme() {
        let url = "not-a-url";
        assert_eq!(redact_rpc_url(url), url);
    }

    /// Regression for finding #2 (HIGH): RFC 3986 — `@` in path
    /// components must NOT be confused with the userinfo delimiter.
    /// `find('@')` (first occurrence) is correct; `rfind('@')` would
    /// mis-handle URLs like `https://host.example.com/path@more` by
    /// treating `@more` as the userinfo boundary.
    #[test]
    fn redact_rpc_url_at_sign_in_path_not_treated_as_userinfo() {
        let url = "https://host.example.com/path@more";
        assert_eq!(
            redact_rpc_url(url),
            url,
            "URL with `@` in path must NOT be redacted (no userinfo)"
        );
    }

    #[test]
    fn config_show_text_mode_emits_lines() {
        let out = config_show(Some("https://user:pass@rpc.example.com"), None, false)
            .expect("config_show text ok");
        assert!(out.contains("network: amoy"));
        assert!(out.contains("rpc_url: https://***:***@rpc.example.com"));
        assert!(out.contains("data_dir: <XDG_DATA_HOME/polygon>"));
        assert!(out.contains("pin_spki: <T8 reserved>"));
        assert!(!out.contains("user:pass"));
    }

    #[test]
    fn config_show_json_mode_emits_object() {
        let out = config_show(Some("https://user:pass@rpc.example.com"), None, true)
            .expect("config_show json ok");
        let parsed: serde_json::Value =
            serde_json::from_str(&out).expect("output must be valid JSON");
        assert_eq!(parsed["network"], "amoy");
        assert_eq!(parsed["rpc_url"], "https://***:***@rpc.example.com");
        assert_eq!(parsed["data_dir"], "<XDG_DATA_HOME/polygon>");
        assert_eq!(parsed["pin_spki"], "<T8 reserved>");
    }

    #[test]
    fn config_show_default_rpc_placeholder_when_none() {
        let out = config_show(None, None, false).expect("ok");
        assert!(out.contains("rpc_url: <default-amoy-rpc>"));
    }

    #[test]
    fn redact_rpc_url_preserves_port_and_path() {
        let redacted = redact_rpc_url("https://user:pass@rpc.example.com:8443/v1/key");
        assert_eq!(redacted, "https://***:***@rpc.example.com:8443/v1/key");
    }
}
