//! `sol-wallet-core` — Platform Abstraction Layer (PAL).
//!
//! Four traits × ~14 methods total per
//! `docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md` §L. Phase 6.1 owns
//! the trait definitions + desktop + test impls. iOS + Android impls
//! land with Phase 8 (FFI).

pub mod clock;
pub mod info;
pub mod network;
pub mod storage;

pub use clock::{Clock, MockClock, SystemClock};
pub use info::{PlatformInfo, StaticInfo, SystemDirsInfo};
pub use network::{NetworkClient, ReqwestClient};
pub use storage::{FileWalletStorage, InMemoryStorage, WalletStorage};
