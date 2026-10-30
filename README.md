<div align="center">

# blockchain-sdk

**Multi-chain Rust wallet SDK — Bitcoin, Ethereum, Polygon, TRON, and Solana**

*Cross-platform libraries (rlib + cdylib FFI) + CLI binaries + Flutter desktop host.*

[![MIT License](https://img.shields.io/badge/License-MIT-yellow.svg)](rust-wallet-app/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.98.1%2B-blue.svg)](rust-wallet-app/rust-toolchain.toml)
[![Workspace Version](https://img.shields.io/badge/workspace-0.2.0-orange.svg)](rust-wallet-app/Cargo.toml)
[![MSRV](https://img.shields.io/badge/MSRV-1.85-blueviolet.svg)](rust-wallet-app/Cargo.toml)

A workspace of 15 Rust crates under [rust-wallet-app/](rust-wallet-app/), plus a markdown research + planning layer under [docs/](docs/) covering SDK deep-dives, user-story matrices, and per-chain implementation plans. A Flutter desktop host ([wallet-desktop/](wallet-desktop/)) consumes the libraries over a C ABI.

</div>

---

## 📑 Contents

1. [What it is](#-what-it-is)
2. [Features](#-features)
3. [Quick Start](#-quick-start)
4. [Supported Chains](#-supported-chains)
5. [Architecture](#-architecture)
6. [Security](#-security)
7. [Documentation](#-documentation)
8. [Contributing](#-contributing)
9. [License & Acknowledgements](#-license--acknowledgements)

---

## 📖 What it is

A multi-chain Rust wallet SDK for teams that want to embed or extend Bitcoin, Ethereum, Polygon, TRON, or Solana wallet logic in their own applications — desktop CLIs for operator workflows, cdylib FFI for Dart / Swift / Kotlin hosts, and a Flutter desktop reference UI.

The repository is also a research + planning workspace: [docs/wallets/](docs/wallets/) holds SDK deep-dives and user stories per chain, and [docs/superpowers/plans/](docs/superpowers/plans/) holds per-chain implementation plans.

---

## ✨ Features

- **Five chain libraries** — one `*-wallet-core` per chain, each building as `rlib + cdylib` (FFI).
- **Five matching CLI binaries** — `btc`, `eth`, `polygon`, `tron`, `sol`.
- **Cross-chain umbrella trait** — `chain-traits` defines `ChainWallet`, `ChainId`, `ChainError`.
- **Flutter desktop host** — Riverpod + go_router, FFI-only.
- **At-rest encryption** — Argon2id KDF + AES-256-GCM, `Zeroizing`-wrapped keys.
- **SPKI-pinned TLS** — parallel implementations across BTC, EVM, and TRON.
- **Stable 0..=5 exit codes** with per-chain `classify()` and a `--json` flag for machine-readable output on list / show / sync / tx-list / config-show.
- **CI quality gates** — fmt, clippy `-D warnings`, `cargo deny`, `cargo audit`, `cargo geiger`.

Per-command flag tables, environment variables, and per-chain caveats live in [rust-wallet-app/README.md](rust-wallet-app/README.md).

---

## 🚀 Quick Start

**Prerequisites:** Rust **1.98.1** (pinned via [rust-wallet-app/rust-toolchain.toml](rust-wallet-app/rust-toolchain.toml)). Optional: Foundry/Anvil for Ethereum e2e, Flutter 3.22+ for the desktop host.

```bash
# Clone and build
git clone <repo-url> blockchain-sdk
cd blockchain-sdk/rust-wallet-app
cargo build --workspace

# Run a CLI (Bitcoin example)
cargo build --release -p btc
./target/release/btc --help

# Cross-compile for mobile (size-optimised: panic=abort, opt-level=z, lto, strip=symbols)
cargo build --profile release-mobile --target <triple>
```

That's enough to get a binary on disk. For test runs, environment-variable inventory, and per-command flag tables, see [rust-wallet-app/README.md](rust-wallet-app/README.md).

---

## 🔗 Supported Chains

| Chain | Library | CLI | Underlying stack |
|---|---|---|---|
| Bitcoin | `bitcoin-wallet-core` | `btc` | BDK 3.1 + rust-bitcoin 0.32 + secp256k1 + bip39 |
| Ethereum | `eth-wallet-core` | `eth` | alloy v1.8.3 (signers + provider + RPC + ABI) |
| Polygon | `polygon-wallet-core` | `polygon` | shares `evm-wallet-core` with Ethereum |
| TRON | `tron-wallet-core` | `tron` | vendored `anychain-{core,tron,kms}` @ commit `cf3aa2d` |
| Solana | `sol-wallet-core` | `sol` | Anza SDK 4.1.0 lineage + Phantom-equivalent Wallet API + SPL/Token-2022 |

The umbrella `chain-traits` crate defines the cross-chain `ChainWallet` trait, `ChainId`, and `ChainError` that per-chain crates implement.

---

## 🏗 Architecture

```text
.
├── docs/                                  # research + planning (markdown only)
│   ├── agents/
│   ├── superpowers/plans/
│   └── wallets/
├── rust-wallet-app/                       # Cargo workspace (the implementation)
│   ├── Cargo.toml
│   ├── crates/
│   │   ├── wallet-errors/                  # shared error categories + exit-code table
│   │   ├── chain-traits/
│   │   ├── bitcoin-wallet-core/
│   │   ├── btc/                            # CLI binary
│   │   ├── evm-wallet-core/                # canonical EVM engine
│   │   ├── eth-wallet-core/
│   │   ├── eth/                            # CLI binary
│   │   ├── polygon-wallet-core/
│   │   ├── polygon/                        # CLI binary
│   │   ├── tron-wallet-core/
│   │   ├── tron/                           # CLI binary
│   │   ├── sol-wallet-core/
│   │   ├── sol/                            # CLI binary
│   │   └── anychain-vendored/              # vendored anychain-{core,tron,kms}
│   ├── scripts/
│   └── README.md                           # detailed CLI reference
├── wallet-desktop/                        # Flutter desktop UI (FFI-only)
├── scripts/                                # repo-level smoke scripts (polygon)
├── tasks/                                  # internal lessons log (not normative)
├── .github/workflows/                      # 7 CI workflows
├── CHANGELOG.md                            # Keep-a-Changelog
└── CLAUDE.md                               # project conventions for AI agents
```

Each chain ships a `*-wallet-core` library (rlib + cdylib) and a thin CLI binary. EVM chains share `evm-wallet-core`; `eth-wallet-core` and `polygon-wallet-core` are import-path wrappers around it.

---

## 🔒 Security

The primitives below are **implemented** in the libraries that list them. Coverage is per-chain; read the table literally.

| Primitive | BDK Bitcoin | EVM (ETH / Polygon) | TRON | Solana |
|---|---|---|---|---|
| Argon2id wallet-file KDF | ✓ | ✓ | ✓ | ✓ |
| AES-256-GCM at-rest | ✓ | ✓ | ✓ | ✓ |
| `Zeroizing<_>` wrap on raw sk | ✓ | ✓ | ✓ | ✓ |
| SPKI-pinned TLS | ✓ | ✓ | ✓ | n/a (Anza uses rustls-native-certs) |
| Encrypted blob mode `0o600` (Unix) | ✓ | ✓ | ✓ | ✓ |
| Mnemonic never plaintext at rest | ✓ | ✓ | ✓ | ✓ |

**Known gaps (verified, not assumed-uniform):**

- The 4-trait Platform Abstraction Layer (`WalletStorage` / `PlatformInfo` / `NetworkClient` / `Clock`) is implemented in `tron-wallet-core` and `sol-wallet-core` only — `bitcoin-wallet-core` and `evm-wallet-core` use ad-hoc file I/O today ([docs/wallets/CONTEXT.md](docs/wallets/CONTEXT.md)).
- The `btc` CLI exit-code logic is bespoke and does **not** use the cross-chain 0..=5 classifier ([docs/wallets/CONTEXT.md](docs/wallets/CONTEXT.md)).
- [rust-wallet-app/deny.toml](rust-wallet-app/deny.toml) retains 7 known advisory exceptions (`bincode 1.x`, `rustls-pemfile 2.x`, `rustls-webpki 0.101.7`, `tokio-tar 0.3.1`, `paste 1.0.15`, `libsecp256k1 0.7.2`, `encoding 0.2.33`) with rationale.

The single permitted `unsafe` block lives in `bitcoin-wallet-core` (`Secret::into_inner` via `ptr::read + mem::forget`) and is audited by `cargo geiger` in CI.

---

## 📚 Documentation

- **Domain glossary** — [docs/wallets/CONTEXT.md](docs/wallets/CONTEXT.md)
- **SDK deep-dives + user stories** — [docs/wallets/](docs/wallets/)
- **Implementation plans** — [docs/superpowers/plans/](docs/superpowers/plans/)
- **Workspace + CLI flag reference** — [rust-wallet-app/README.md](rust-wallet-app/README.md)
- **Solana crate** — [rust-wallet-app/crates/sol-wallet-core/README.md](rust-wallet-app/crates/sol-wallet-core/README.md)
- **Flutter host** — [wallet-desktop/README.md](wallet-desktop/README.md)
- **Changelog** — [CHANGELOG.md](CHANGELOG.md)

---

## 🤝 Contributing

Read [CLAUDE.md](CLAUDE.md) for project conventions and the per-task pipeline, then check the relevant chain's [implementation plan](docs/superpowers/plans/) and [SDK deep-dive](docs/wallets/). Code style: `cargo fmt`, `cargo clippy -- -D warnings`, `cargo geiger`. Docs: conform to [.markdownlint.json](.markdownlint.json).

CI workflows live under [.github/workflows/](.github/workflows/). PRs route review per [.github/CODEOWNERS](.github/CODEOWNERS).

---

## ⚖️ License & Acknowledgements

Distributed under the MIT License — see [rust-wallet-app/LICENSE](rust-wallet-app/LICENSE).

- Bitcoin: [BDK](https://github.com/bitcoindevkit/bdk), [rust-bitcoin](https://github.com/rust-bitcoin/rust-bitcoin), [rust-secp256k1](https://github.com/rust-bitcoin/rust-secp256k1), [bip39](https://github.com/rust-bitcoin/bip39).
- Ethereum / Polygon: [alloy](https://github.com/alloy-rs/alloy).
- TRON: [0xcregis/anychain](https://github.com/0xcregis/anychain) (vendored at commit `cf3aa2d`).
- Solana: [Anza SDK](https://github.com/anza-xyz/solana-sdk) (4.1.0 lineage).
- Foundational research and threat-modeling: [docs/wallets/](docs/wallets/) deep-dives and [docs/superpowers/plans/](docs/superpowers/plans/) implementation plans.

<div align="center">

⭐ If this helps you, consider starring the repo.

</div>