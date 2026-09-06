//! `WalletId` — storage-layer handle for a single encrypted wallet blob.
//!
//! Per plan §Phase 5 Task 4.1 + file structure: stable, comparable,
//! not a derivation index. `uuid::Uuid::v4()` as bytes — distinct
//! enough that two `wallet create` runs in the same nanosecond do not
//! collide, and the resulting hex string is short enough for CLI logs.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

/// Storage-layer handle for one encrypted wallet blob.
///
/// **Important:** `WalletId` is *not* a key-derivation index. The
/// derivation path (`m/44'/195'/0'/0/0` etc.) lives inside the
/// encrypted blob. Two wallets with the same mnemonic but different
/// derivation paths still get distinct ids; two `wallet create`
/// invocations with the same mnemonic get distinct ids by design.
///
/// **Length:** 16 bytes (UUID v4). Stored as raw bytes in
/// `WalletStorage::put_atomic`; hex-encoded for CLI + log output
/// (`"9f4a…e7b2"`). The plan's CLI surface treats it as opaque.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WalletId([u8; 16]);

impl WalletId {
    /// Generate a fresh v4 UUID. Uses the OS CSPRNG via `uuid` crate.
    pub fn new() -> Self {
        Self(*uuid::Uuid::new_v4().as_bytes())
    }

    /// Raw bytes (16). Caller is responsible for hex encoding when
    /// displaying; storage layers pass these through verbatim.
    pub fn as_bytes(&self) -> &[u8; 16] {
        &self.0
    }

    /// Lowercase hex (32 chars), no separator. CLI + log format.
    pub fn to_hex(&self) -> String {
        hex::encode(self.0)
    }
}

impl Default for WalletId {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Display for WalletId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl fmt::Debug for WalletId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "WalletId({})", self.to_hex())
    }
}

impl FromStr for WalletId {
    type Err = Error;

    /// Parse lowercase or uppercase 32-char hex. Any other length,
    /// separator, or non-hex character returns `Error::Config`.
    fn from_str(s: &str) -> Result<Self> {
        if s.len() != 32 {
            return Err(Error::Config(format!(
                "wallet id must be 32 hex chars, got {}",
                s.len()
            )));
        }
        let bytes =
            hex::decode(s).map_err(|e| Error::Config(format!("wallet id hex decode: {e}")))?;
        let arr: [u8; 16] = bytes.try_into().map_err(|v: Vec<u8>| {
            Error::Config(format!(
                "wallet id must decode to 16 bytes, got {}",
                v.len()
            ))
        })?;
        Ok(Self(arr))
    }
}
