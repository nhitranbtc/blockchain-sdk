//! Type-safe Esplora base URL.
//!
//! Consolidates the URL validation rules that used to live inline in
//! `EsploraClient::new` (issues #35, #36). Validating at the type level
//! means the rest of the codebase cannot accidentally construct a
//! client from a malformed URL — the constructor would refuse to
//! compile (the newtype signature requires an `EsploraUrl`, not a
//! raw `&str`).
//!
//! **Validation rules** (failing any one returns `Error::Esplora`):
//!
//! 1. Input parses as a `reqwest::Url`.
//! 2. Scheme is `https` (NOT `http`, `ftp`, `file`, etc.).
//! 3. No embedded userinfo: rejects `user@`, `user:pass@`, and
//!    bare `@` in the authority segment.
//!
//! **Defense against credential leak**: the error message redacts
//! any userinfo before formatting, so the literal password never
//! reaches `Error::Esplora(...).Display`. See `redact_userinfo`.
//!
//! **Trailing-slash normalization**: `EsploraUrl::new` appends `/` to
//! the path if missing, so `reqwest::Url::join("address/{addr}/utxo")`
//! produces the correct path (without it, `join` replaces the last
//! segment — `https://host.com/api` + `address/x` →
//! `https://host.com/address/x`).

use std::str::FromStr;

use crate::error::Error;

/// Type-safe Esplora base URL.
///
/// Wraps `reqwest::Url` after applying the validation rules documented
/// on the module. The inner field is private; use [`Self::as_url`] to
/// get a reference.
#[derive(Debug, Clone)]
pub struct EsploraUrl(reqwest::Url);

impl EsploraUrl {
    /// Build an `EsploraUrl` from a raw string, applying all
    /// validation rules.
    ///
    /// # Errors
    ///
    /// Returns [`Error::Esplora`] if:
    /// - `raw` is not a valid URL.
    /// - `raw` carries userinfo (`user@`, `user:password@`, bare `@`).
    ///   The error message is redacted via [`redact_userinfo`] so the
    ///   literal password never reaches `Display`.
    /// - `raw` is not an `https://` URL.
    pub fn new(raw: &str) -> Result<Self, Error> {
        let mut url = reqwest::Url::parse(raw)
            .map_err(|e| Error::Esplora(format!("invalid esplora url: {e}")))?;
        // Reject embedded credentials (per issue #35).
        // - `username() != ""` catches `user@host`
        // - `password().is_some()` catches `user:password@host`
        // - `raw_after_scheme_has_at` catches `https://@host/` (the
        //   WHATWG parser yields empty username and None password
        //   for that form, slipping past the typed checks)
        let has_userinfo =
            !url.username().is_empty() || url.password().is_some() || raw_after_scheme_has_at(raw);
        if has_userinfo {
            let redacted = redact_userinfo(raw);
            return Err(Error::Esplora(format!(
                "esplora url must not contain userinfo (username/password): {redacted}"
            )));
        }
        if url.scheme() != "https" {
            return Err(Error::Esplora(format!(
                "esplora url must use https:// scheme, got: {}",
                url.scheme()
            )));
        }
        // Trailing-slash normalization for `Url::join` semantics.
        if !url.path().ends_with('/') {
            url.set_path(&format!("{}/", url.path()));
        }
        Ok(Self(url))
    }

    /// Borrow the inner parsed URL.
    pub fn as_url(&self) -> &reqwest::Url {
        &self.0
    }

    /// Consume and return the inner parsed URL.
    pub fn into_inner(self) -> reqwest::Url {
        self.0
    }
}

impl FromStr for EsploraUrl {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::new(s)
    }
}

impl AsRef<reqwest::Url> for EsploraUrl {
    fn as_ref(&self) -> &reqwest::Url {
        &self.0
    }
}

impl std::fmt::Display for EsploraUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Display the redacted form so this type never accidentally
        // leaks credentials through a Debug/Display impl.
        write!(f, "{}", redact_userinfo(self.0.as_str()))
    }
}

/// True if the raw input string has an `@` between the `://` delimiter
/// and the first `/` (or end of string). Belt-and-braces check for the
/// `https://@host/` form — WHATWG URL parser yields `username() == ""`
/// and `password() == None` for that form, which would otherwise slip
/// past `username().is_empty() || password().is_some()`.
///
/// Conservative: returns true on any `@` in the authority segment,
/// including those inside IPv6 brackets — IPv6 with userinfo is not a
/// real-world Esplora pattern and rejecting it is the safe side.
fn raw_after_scheme_has_at(raw: &str) -> bool {
    let Some(scheme_end) = raw.find("://") else {
        return false;
    };
    let after_scheme = &raw[scheme_end + 3..];
    let auth_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    after_scheme[..auth_end].contains('@')
}

/// Redact userinfo from a URL string for safe inclusion in error
/// messages. Replaces the `user[:password]@` segment (if any) with
/// `***@`. If no userinfo is present, returns the input unchanged.
///
/// Conservative: operates on the raw string (not the parsed
/// `reqwest::Url`), so percent-encoded forms are also caught.
pub(crate) fn redact_userinfo(raw: &str) -> String {
    let Some(scheme_end) = raw.find("://") else {
        return raw.to_string();
    };
    let after_scheme = &raw[scheme_end + 3..];
    let auth_end = after_scheme
        .find(['/', '?', '#'])
        .unwrap_or(after_scheme.len());
    if !after_scheme[..auth_end].contains('@') {
        return raw.to_string();
    }
    // Split authority at the LAST `@` (in case `@` appears inside the
    // password). Replace everything before it with `***@`.
    let last_at_in_auth = after_scheme[..auth_end]
        .rfind('@')
        .expect("auth segment contains '@'");
    let prefix = &raw[..scheme_end + 3];
    let _userinfo = &after_scheme[..last_at_in_auth];
    let host_and_rest = &after_scheme[last_at_in_auth..];
    // `host_and_rest` starts with `@`; preserve everything from there
    // (host:port, then path/query/fragment).
    format!("{prefix}***{host_and_rest}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_https_url() {
        let u = EsploraUrl::new("https://blockstream.info/testnet/api").unwrap();
        assert_eq!(u.as_url().as_str(), "https://blockstream.info/testnet/api/");
    }

    #[test]
    fn parse_https_url_with_trailing_slash() {
        let u = EsploraUrl::new("https://blockstream.info/testnet/api/").unwrap();
        assert_eq!(u.as_url().as_str(), "https://blockstream.info/testnet/api/");
    }

    #[test]
    fn join_after_trailing_slash_preserves_path() {
        let u = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let joined = u.as_url().join("fee-estimates").unwrap();
        assert_eq!(
            joined.as_str(),
            "https://blockstream.info/api/fee-estimates"
        );
    }

    #[test]
    fn reject_invalid_url() {
        let err = EsploraUrl::new("not a url").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        assert!(err.to_string().contains("invalid esplora url"));
    }

    #[test]
    fn reject_http_scheme() {
        let err = EsploraUrl::new("http://blockstream.info/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        assert!(err.to_string().contains("https://"));
    }

    #[test]
    fn reject_ftp_scheme() {
        let err = EsploraUrl::new("ftp://example.com/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
    }

    #[test]
    fn reject_password_in_url() {
        let err = EsploraUrl::new("https://attacker:p4ssw0rd@blockstream.info/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(msg.contains("password"), "msg = {msg}");
        // Redaction: literal password never appears.
        assert!(!msg.contains("p4ssw0rd"), "password leaked into err: {msg}");
        assert!(!msg.contains("attacker"), "username leaked into err: {msg}");
    }

    #[test]
    fn reject_user_only_in_url() {
        let err = EsploraUrl::new("https://user@blockstream.info/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(msg.contains("username"), "msg = {msg}");
        assert!(!msg.contains("user@"), "username leaked into err: {msg}");
    }

    #[test]
    fn reject_bare_at_in_authority() {
        let err = EsploraUrl::new("https://@blockstream.info/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(msg.contains("userinfo"), "msg = {msg}");
    }

    #[test]
    fn reject_percent_encoded_userinfo() {
        let err = EsploraUrl::new("https://attacker%40evil.example:p4ssw0rd@blockstream.info/api")
            .unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(!msg.contains("p4ssw0rd"), "password leaked: {msg}");
    }

    #[test]
    fn reject_ipv6_authority_with_userinfo() {
        let err = EsploraUrl::new("https://user:pass@[::1]:443/api").unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
        let msg = err.to_string();
        assert!(!msg.contains(":pass@"), "password leaked: {msg}");
    }

    #[test]
    fn from_str_impl() {
        let u: EsploraUrl = "https://blockstream.info/testnet/api".parse().unwrap();
        assert_eq!(u.as_url().as_str(), "https://blockstream.info/testnet/api/");
    }

    #[test]
    fn from_str_rejects_bad_url() {
        let err = "http://example.com/api".parse::<EsploraUrl>().unwrap_err();
        assert!(matches!(err, Error::Esplora(_)));
    }

    #[test]
    fn display_redacts_userinfo() {
        let u = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let displayed = format!("{u}");
        assert_eq!(displayed, "https://blockstream.info/api/");
        assert!(!displayed.contains("***"));
    }

    #[test]
    fn redact_userinfo_replaces_user_pass() {
        let r = redact_userinfo("https://user:p4ss@host/api");
        assert_eq!(r, "https://***@host/api");
    }

    #[test]
    fn redact_userinfo_replaces_user_only() {
        let r = redact_userinfo("https://user@host/api");
        assert_eq!(r, "https://***@host/api");
    }

    #[test]
    fn redact_userinfo_no_userinfo_unchanged() {
        let r = redact_userinfo("https://host/api");
        assert_eq!(r, "https://host/api");
    }

    #[test]
    fn redact_userinfo_handles_query() {
        let r = redact_userinfo("https://user:pass@host/api?token=x");
        assert_eq!(r, "https://***@host/api?token=x");
    }

    #[test]
    fn as_ref_returns_url() {
        let u = EsploraUrl::new("https://blockstream.info/api").unwrap();
        let url_ref: &reqwest::Url = u.as_ref();
        assert_eq!(url_ref.as_str(), "https://blockstream.info/api/");
    }
}
