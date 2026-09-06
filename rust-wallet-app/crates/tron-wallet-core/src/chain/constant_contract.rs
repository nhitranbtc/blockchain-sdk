//! Response shape for `POST /wallet/triggerconstantcontract`.
//!
//! Lives next to the TronGrid client (rather than inside it) so the
//! serde-derived structs are visible to downstream modules (`trc20::decimals`,
//! `trc20::symbol`, `resource::estimate_energy`) without dragging the
//! `reqwest` client into their type surface.
//!
//! ## Wire-format note
//!
//! TronGrid returns the constant-contract result as a *list* of hex strings
//! (`constant_result`), one entry per `return` parameter of the called
//! function. For a `function f() returns (uint256)` style contract — which
//! is what `balanceOf` / `decimals` / `symbol` / `name` all are — the list
//! has exactly one entry. The helpers in this module always read index 0
//! and refuse the response if it is absent or empty.

use serde::Deserialize;

/// Top-level response of `wallet/triggerconstantcontract`.
///
/// `transaction` and `energy_used` are populated when the call is invoked
/// with `"visible": true` and the network actually executed the call in a
/// VM. They are not strictly required to read the constant result, but the
/// resource model (plan Task 3.8) consumes `energy_used` for fee sizing.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct ConstantContractCall {
    /// Whether the call was rejected before execution (e.g. revert, out of
    /// energy). `result.result == true` is the success shape.
    #[serde(default)]
    pub result: ConstantCallResult,

    /// Per-return parameter, hex-encoded ABI bytes. One entry per return
    /// parameter of the called function. Empty if the call failed.
    #[serde(rename = "constant_result", default)]
    pub constant_result: Vec<String>,

    /// Energy consumed during the simulated call. Used by [`crate::resource`]
    /// for fee-limit sizing (plan Task 3.8).
    #[serde(rename = "energy_used", default)]
    pub energy_used: Option<u64>,

    /// Energy penalty applied on top of `energy_used` for resource exhaustion
    /// or other transient conditions. None on a successful call.
    #[serde(rename = "energy_penalty", default)]
    pub energy_penalty: Option<u64>,

    /// Raw transaction envelope, present when the network had to construct
    /// one even for a constant call. Surface only — we do not act on it.
    #[serde(default)]
    pub transaction: Option<serde_json::Value>,
}

/// `result.result == true` is the success shape for `wallet/triggerconstantcontract`.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ConstantCallResult {
    #[serde(default)]
    pub result: bool,
    #[serde(default)]
    pub code: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

impl ConstantContractCall {
    /// Decode the first entry of `constant_result` as raw bytes.
    ///
    /// Returns an error if the list is empty (the call did not produce a
    /// result — either it failed or the called function returned nothing)
    /// or if the first entry is not valid hex. Both branches surface as
    /// [`crate::error::Error::NodeResponse`] so the call site can use `?`
    /// against the crate-wide `Result` alias.
    pub fn constant_result_bytes(&self) -> Result<Vec<u8>, crate::error::Error> {
        let first = self
            .constant_result
            .first()
            .ok_or_else(|| crate::error::Error::NodeResponse("empty constant_result".into()))?;
        hex::decode(first).map_err(|_| {
            crate::error::Error::NodeResponse("constant_result[0] is not valid hex".into())
        })
    }
}
