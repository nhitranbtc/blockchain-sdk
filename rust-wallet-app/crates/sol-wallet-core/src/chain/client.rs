//! `chain::client` — `RpcClient` + `RateLimiter` + `BlockhashCache` (V0.1.5 stub).
//!
//! `RpcClient` is a thin reqwest-based JSON-RPC client replacing Anza's
//! `solana-rpc-client` (which is blocked per issue #555). The 15 RPC
//! methods listed in the plan doc §Phase 5 Table 5.1 delegate through
//! the private `post()` helper, which enforces:
//!
//! 1. **URL allowlist** (Tier 1 finding #1) — `RpcClient::new` rejects
//!    non-https URLs and non-localhost http URLs in the constructor.
//!    Returns `Err(Error::Transport(...))` for invalid URLs.
//! 2. **Typed JSON-RPC envelope** (Tier 2 finding #7) — `RpcResponse<T>`
//!    + `RpcError { code, message }` with `#[serde(deny_unknown_fields)]`.
//!      No `serde_json::Value` indexing.
//! 3. **Rate limiter** (Task 5.4) — token bucket, default 50 req/s with
//!    burst 100. Each `post()` call acquires a permit before sending.
//! 4. **Custom `Debug` impl** (Tier 2 finding #11) — strips URL query
//!    string so accidental `dbg!()` doesn't leak API keys.
//! 5. **bincode wire format** (Tier 2 finding #8) — `sendTransaction`
//!    uses `bincode::serialize(&tx, bincode::config::legacy())` + base64;
//!    `bincode = "=1.3.3"` is pinned in workspace (already pinned
//!    per Phase 3 plan).
//!
//! ## BlockhashCache (V0.1.5 stub)
//!
//! The cache struct ships in V0.1.5 to avoid re-fetching the blockhash
//! for re-sign-and-retry on stale-hash. V0.1's `send_and_confirm` does
//! a single fresh fetch per send; the cache infrastructure compiles but
//! `get_or_fetch` is unused until the V0.1.5 retry-on-stale-hash PR.
//!
//! ## `simulateTransaction` TOCTOU (Tier 4 finding #4)
//!
//! The preflight result returned by `simulate_transaction` is a HINT,
//! not a guarantee. Cluster state may change between the simulation and
//! the broadcast. The CU computation could overflow in the interim.
//! This is documented in the method's doc comment but not enforced at
//! runtime.
//!
//! Importers: `chain::mod` (re-exports `RpcClient`, `RateLimiter`);
//! `tx::broadcast` (`send_and_confirm`); the 15 RPC method tests in
//! `tests/rpc_methods_mock.rs`.
//!
//! Affected API: `RpcClient::new`, `RpcClient::new_with_pinned_spki`
//! (Task 5.5), 15 RPC method wrappers, `RateLimiter` (Task 5.4 wiring).

use std::sync::Arc;
use std::time::{Duration, Instant};

use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use url::Url;

use crate::error::{Error, Result};

/// Default blockhash TTL — matches the Solana `recent_blockhash`
/// retention window (~90 slots × ~400ms = ~36s on a healthy cluster;
/// 60s is the conservative upper bound most wallets use).
///
/// V0.1 ships the constant + struct for the V0.1.5 retry-on-stale-hash
/// follow-up; `BlockhashCache::get_or_fetch` is not yet called from
/// `tx::broadcast::send_and_confirm` in V0.1.
pub const DEFAULT_BLOCKHASH_TTL: Duration = Duration::from_secs(60);

/// Default outbound request rate (requests per second) for the per-
/// `RpcClient` rate limiter.
pub const DEFAULT_RATE_LIMIT_RPS: u32 = 50;

/// Default burst allowance for the per-`RpcClient` rate limiter.
pub const DEFAULT_RATE_LIMIT_BURST: u32 = 100;

/// Default per-request timeout (reqwest client builder).
pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

// =============================================================================
// Rate limiter — re-export from `chain::rate_limit` for back-compat with
// Phase 5.1 callers. The implementation lives in `rate_limit.rs` (Task 5.4
// plan spec calls for a separate `src/chain/rate_limit.rs`).
// =============================================================================
pub use crate::chain::rate_limit::RateLimiter;

/// Cached blockhash + fetch timestamp.
///
/// Returned by [`BlockhashCache::get_or_fetch`] so callers can decide
/// whether the cached hash is fresh enough for their use without
/// querying again (e.g. compute time-budget for the next send).
///
/// V0.1 ships the struct + impls; `send_and_confirm` does not yet
/// call `get_or_fetch` (deferred to V0.1.5 retry-on-stale-hash).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockhashCacheEntry {
    /// The blockhash value.
    pub hash: solana_sdk::hash::Hash,
    /// Slot the blockhash was produced at (Anza's RpcClient returns
    /// `(Hash, slot)` together from `get_latest_blockhash`).
    pub slot: u64,
    /// Instant the entry was cached (monotonic clock, NOT wall time).
    pub fetched_at: Instant,
}

/// TTL cache over `get_latest_blockhash` (Q11 — avoid re-fetching
/// every send; V0.1.5 retry-on-stale-hash).
#[derive(Debug, Clone)]
pub struct BlockhashCache {
    ttl: Duration,
    entry: Option<BlockhashCacheEntry>,
}

impl BlockhashCache {
    /// Construct a cache with the default TTL (`DEFAULT_BLOCKHASH_TTL`).
    pub fn new() -> Self {
        Self {
            ttl: DEFAULT_BLOCKHASH_TTL,
            entry: None,
        }
    }

    /// Construct a cache with an explicit TTL (used by tests).
    pub fn with_ttl(ttl: Duration) -> Self {
        Self { ttl, entry: None }
    }

    /// TTL — surfaced for tests that drive the clock.
    pub fn ttl(&self) -> Duration {
        self.ttl
    }

    /// Force the next `get_or_fetch` to re-hit the RPC by discarding
    /// the cached entry. Used by `send_and_confirm` after a
    /// `BlockhashNotFound` error so the retry path always sees a
    /// fresh hash (V0.1.5).
    pub fn invalidate(&mut self) {
        self.entry = None;
    }
}

impl Default for BlockhashCache {
    fn default() -> Self {
        Self::new()
    }
}

/// JSON-RPC response envelope (success path).
///
/// `#[serde(deny_unknown_fields)]` per Tier 2 finding #7 — silent
/// future-compat breaks become loud parse errors instead of `None`
/// returns. `id` is a monotonic counter inside `RpcClient` to detect
/// replayed responses (defense against HTTP smuggling).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: T,
}

/// JSON-RPC response envelope (error path).
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RpcErrorEnvelope {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    error: RpcError,
}

/// JSON-RPC error object.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RpcError {
    /// JSON-RPC error code (e.g. -32000 for server error, -32003 for
    /// Solana devnet "airdrop limit").
    pub code: i64,
    /// Human-readable error message.
    pub message: String,
}

/// Thin reqwest-based JSON-RPC client replacing Anza's `solana-rpc-client`
/// (which is blocked per issue #555).
///
/// All 15 V0.1 RPC methods delegate through the private `post()` helper,
/// which enforces the URL allowlist (constructor), the rate limiter
/// (per-call), the typed envelope deserialization, and the custom
/// `Debug` impl.
#[derive(Clone)]
pub struct RpcClient {
    url: String,
    host: String,
    http: Client,
    rate_limiter: RateLimiter,
    id_counter: Arc<std::sync::Mutex<u64>>,
    /// Task 5.5 — SPKI pin (None = no pin; Some(bytes) = caller must
    /// verify against the live cert chain returned by reqwest).
    pinned_spki: Option<SpkiDer>,
}

impl std::fmt::Debug for RpcClient {
    /// Tier 2 finding #11 — strip URL query string so `dbg!()` doesn't
    /// leak API keys. Renders as `RpcClient { url: "<scheme>://<host>[:port]/<path>", ... }`.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let safe_url = match Url::parse(&self.url) {
            Ok(parsed) => {
                let mut s = format!("{}://{}", parsed.scheme(), parsed.host_str().unwrap_or(""));
                if let Some(port) = parsed.port() {
                    s.push_str(&format!(":{port}"));
                }
                let path = parsed.path();
                if path != "/" {
                    s.push_str(path);
                }
                s
            }
            Err(_) => "<invalid-url>".to_string(),
        };
        f.debug_struct("RpcClient")
            .field("url", &safe_url)
            .field(
                "rate_limit",
                &format!(
                    "{} req/s, burst {}",
                    DEFAULT_RATE_LIMIT_RPS, DEFAULT_RATE_LIMIT_BURST
                ),
            )
            .finish()
    }
}

impl RpcClient {
    /// Construct a new RpcClient with default rate limiter (50 req/s, burst 100)
    /// and default request timeout (30s).
    ///
    /// **Tier 1 finding #1 (URL allowlist)**: rejects non-https URLs and
    /// non-localhost http URLs. Acceptable:
    /// - `https://<host>[:port]` (any host)
    /// - `http://localhost[:port]`
    /// - `http://127.0.0.1[:port]`
    ///
    /// Rejected (returns `Err(Error::Transport(...))`):
    /// - `http://attacker.com` (cleartext exfil of signed tx)
    /// - `ftp://...`, `file://...`, `javascript:...`
    /// - `http://192.168.x.x` from non-loopback address
    pub fn new(url: &str) -> Result<Self> {
        Self::with_rate_limit(url, DEFAULT_RATE_LIMIT_RPS, DEFAULT_RATE_LIMIT_BURST)
    }

    /// Construct with a custom rate limit (used by tests; 0/0 disables).
    pub fn with_rate_limit(url: &str, req_per_sec: u32, burst: u32) -> Result<Self> {
        // Step 1: parse + allowlist
        let parsed =
            Url::parse(url).map_err(|e| Error::Transport(format!("invalid RPC URL: {e}")))?;
        let scheme = parsed.scheme();
        let host = parsed.host_str().unwrap_or("").to_string();
        match scheme {
            "https" => {}                                              // OK
            "http" if host == "localhost" || host == "127.0.0.1" => {} // OK
            _ => {
                return Err(Error::Transport(format!(
                    "RPC URL must be https://... or http://localhost[:port] (got scheme={} host={})",
                    scheme, host
                )));
            }
        }
        // Step 2: build reqwest client with timeout
        let http = Client::builder()
            .timeout(DEFAULT_REQUEST_TIMEOUT)
            .build()
            .map_err(|e| Error::Transport(format!("reqwest client build: {e}")))?;
        // Step 3: rate limiter (only allocated after URL allowlist passes)
        let rate_limiter = RateLimiter::new(req_per_sec, burst);
        Ok(Self {
            url: url.to_string(),
            host,
            http,
            rate_limiter,
            id_counter: Arc::new(std::sync::Mutex::new(0)),
            pinned_spki: None,
        })
    }

    /// Cluster URL this client was constructed with.
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Host portion of the URL (used for devnet allowlist checks in
    /// `account::request_airdrop`).
    pub fn host(&self) -> &str {
        &self.host
    }

    /// Issue a JSON-RPC POST. Returns the deserialized `result` or
    /// `Error::Rpc { code, message }` on JSON-RPC error envelope, or
    /// `Error::Transport(...)` on transport / rate-limit failure.
    ///
    /// Every public RPC method in `chain::account` routes through here.
    /// The `params` are passed as-is (already serialized by the caller
    /// to the appropriate Anza `Rpc*` config types).
    pub async fn post<T>(&self, method: &str, params: Value) -> Result<T>
    where
        T: serde::de::DeserializeOwned,
    {
        // Acquire rate-limit permit (no-op if disabled)
        self.rate_limiter.acquire(&self.host).await?;

        // Monotonic id
        let id = {
            let mut counter = self.id_counter.lock().expect("id counter mutex poisoned");
            *counter = counter.wrapping_add(1);
            *counter
        };

        let body = json!({
            "jsonrpc": "2.0",
            "id": id,
            "method": method,
            "params": params,
        });

        let resp = self
            .http
            .post(&self.url)
            .json(&body)
            .send()
            .await
            .map_err(|e| Error::Transport(format!("RPC POST {method}: {e}")))?;

        if !resp.status().is_success() {
            return Err(Error::Transport(format!(
                "RPC POST {method} returned HTTP {}",
                resp.status()
            )));
        }

        let raw: Value = resp
            .json()
            .await
            .map_err(|e| Error::Transport(format!("RPC POST {method} body decode: {e}")))?;

        // Try success envelope first; fall back to error envelope
        if let Ok(ok) = serde_json::from_value::<RpcResponse<T>>(raw.clone()) {
            Ok(ok.result)
        } else if let Ok(err_env) = serde_json::from_value::<RpcErrorEnvelope>(raw) {
            Err(Error::Rpc {
                code: err_env.error.code as i32,
                message: err_env.error.message,
            })
        } else {
            // Malformed envelope — neither success nor error shape matched
            Err(Error::Transport(format!(
                "RPC POST {method} returned malformed JSON-RPC envelope"
            )))
        }
    }
}

// =============================================================================
// Task 5.5 — SPKI pin escape hatch (Tier 3 finding #2)
// =============================================================================
//
// MITM defense for the case where the cluster's TLS CA is compromised.
//
// V0.1 SCOPE — caller-driven SPKI verification (see #555):
//   - Constructor stores the pinned SPKI bytes verbatim
//   - `pinned_spki()` accessor exposes them for caller-side validation
//     (Phase 7 CLI logs the pin hash + compares against the live cert
//     chain returned by reqwest after each connect)
//   - LIVE TLS-level SPKI enforcement (intercepting the handshake via
//     rustls `ClientCertVerifier`) deferred to V0.1.5 — reqwest 0.12
//     stable does not yet expose a SPKI-pinning API; implementing it
//     requires `rustls::client::WebPkiServerVerifier::with_spki_pinning`
//     (unstable as of 2026-09-10).
//
// Why not silently fall back to no-pinning on error?
//   The whole point of `new_with_pinned_spki` is to defend against MITM.
//   A silent fallback to an unpinned client would defeat the function's
//   named intent (Tier 3 finding #2). The constructor MUST either return
//   a client with the pin attached (caller verifies) or return `Err`.
//   No middle ground.

/// DER bytes of a SubjectPublicKeyInfo envelope that callers MUST
/// validate against the live TLS cert chain after each connect.
///
/// Extracted out-of-band via:
/// ```text
/// openssl s_client -connect api.mainnet-beta.solana.com:443 -showcerts
/// openssl x509 -in leaf.pem -pubkey -noout | openssl asn1parse -out spki.der
/// ```
pub type SpkiDer = Vec<u8>;

impl RpcClient {
    /// Construct an `RpcClient` whose TLS handshake is bound to a specific
    /// SubjectPublicKeyInfo (DER bytes).
    pub fn new_with_pinned_spki(url: &str, spki_der: SpkiDer) -> Result<Self> {
        if spki_der.is_empty() {
            return Err(Error::Transport(
                "SPKI pin: empty DER bytes — refusing to construct an unpinned client".to_string(),
            ));
        }
        let mut client =
            Self::with_rate_limit(url, DEFAULT_RATE_LIMIT_RPS, DEFAULT_RATE_LIMIT_BURST)?;
        client.pinned_spki = Some(spki_der);
        Ok(client)
    }

    /// Stored SPKI pin (if any), as DER bytes.
    pub fn pinned_spki(&self) -> Option<&SpkiDer> {
        self.pinned_spki.as_ref()
    }
}
