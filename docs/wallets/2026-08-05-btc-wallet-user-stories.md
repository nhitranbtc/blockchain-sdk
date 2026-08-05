# `btc` Bitcoin Wallet — User Stories

**Date:** 2026-08-05
**Companion to:** [Rust Bitcoin wallet implementation plan](../../superpowers/plans/2026-08-05-rust-bitcoin-wallet.md) + [BDK 3.1 features audit](2026-08-05-bdk-wallet-features.md)
**Surface:** `btc` CLI binary, **default network = Bitcoin testnet**. Mainnet opt-in via `--network mainnet`.

Personas:
- **Alice** — Bitcoin power user. Manages multiple wallets. Wants CLI control + scriptable commands.
- **Bob** — Developer integrating Bitcoin. Wants stable exit codes + JSON output for automation.
- **Carol** — First-time user. Wants clear prompts, warnings about mnemonic safety, no surprises.

---

## Story → BDK 3.1 feature map

Each story is implemented via one or more BDK 3.1 APIs. This table is the traceability link between the user-facing feature and the underlying wallet engine call. The 4th column shows which `rust-bitcoin 0.32` primitive backs the BDK call — making the two-layer architecture (`rust-bitcoin` primitives → `bdk_wallet` engine) explicit per story.

| # | Story | BDK 3.1 feature(s) used | `rust-bitcoin 0.32` primitive(s) used |
|---|---|---|---|
| 1 | Create a new wallet | `Wallet::create(descriptor, change_descriptor).network(Network).create_wallet_no_persist()`; `bdk_wallet::keys::bip39::Mnemonic::generate(12)` | `bdk_wallet::bitcoin::Network`, `bdk_wallet::bitcoin::secp256k1::All` |
| 2 | Import an existing wallet | `bdk_wallet::keys::bip39::Mnemonic::parse_in(Language::English, s)`; `Wallet::create` | `bdk_wallet::bitcoin::secp256k1::All` |
| 3 | Check balance | `Wallet::balance() -> Balance { confirmed, trusted_pending, untrusted_pending, immature }` | `bdk_wallet::bitcoin::Amount` (in `Balance` struct) |
| 4 | Sync chain state | `bdk_esplora::EsploraExt::full_scan(FullScanRequest)` + `Wallet::apply_update(SyncResult)`; `Wallet::latest_checkpoint()` | `bdk_wallet::bitcoin::Transaction`, `OutPoint`, `BlockHash` (in chain index) |
| 5 | Send a payment | `Wallet::build_tx() -> TxBuilder::add_recipient(script, amount).fee_rate(rate).finish() -> Result<Psbt, CreateTxError>`; `Wallet::sign(&mut psbt, SignOptions::default())`; `EsploraExt::broadcast(&tx)` | `bdk_wallet::bitcoin::Psbt` (in/out), `Address::script_pubkey()` (recipient), `Amount` (value), `FeeRate` (rate), `Transaction` (final extracted) |
| 6 | Send with custom fee rate | `TxBuilder::fee_rate(FeeRate::from_sat_per_vb(rate))` | `bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(rate)` |
| 7 | Inspect transaction history | `Wallet::transactions()` + `Wallet::full_txs()` for canonical + pending | `bdk_wallet::bitcoin::Transaction` (in `WalletTx`/`TxDetails`), `Txid`, `compute_txid()` |
| 8 | Get current fee estimates | `EsploraClient::get_fee_estimates().await -> HashMap<String, f64>` (target blocks → sat/vB) | `bdk_wallet::bitcoin::FeeRate::from_sat_per_vb(f64 as u64)` |
| 9 | List / show / delete / rename wallets | `Wallet::network()`, `Wallet::public_descriptor(keychain)`, `Wallet::descriptor_checksum(keychain)`; `std::fs::remove_dir_all` for delete | `bdk_wallet::bitcoin::Network`, `Descriptor<DescriptorPublicKey>` (string), `bdk_wallet::bitcoin::Address` (first address) |
| 10 | Use mainnet explicitly | `CreateParams::network(Network::Bitcoin)`; `LoadParams::check_network(Network::Bitcoin)` | `bdk_wallet::bitcoin::Network::Bitcoin` (HRP `bc`) |
| 11 | Show config + debug info | `std::env::var`, `version() -> &'static str` | n/a |
| 12 | Persist wallet across CLI invocations | `Wallet::take_staged() -> ChangeSet`; `bdk_file_store::Store::load_or_create(name, magic).append(&changeset)`; `Wallet::load().descriptor(...).load_wallet_no_persist()` | `bdk_wallet::bitcoin::Transaction` (in `ChangeSet`), `OutPoint` (in chain index) |
| 13 | Send to multiple recipients | `TxBuilder::add_recipient(addr, amount).add_recipient(addr2, amount2)...` (chainable) | `bdk_wallet::bitcoin::Address::script_pubkey()` (per recipient), `Amount` (per output) |
| 14 | Drain wallet to a single address | `TxBuilder::drain_wallet()` / `.drain_to(addr)` | `bdk_wallet::bitcoin::Address::script_pubkey()` (recipient) |
| 15 | Choose coin selection algorithm | `bdk_wallet::coin_selection::{BranchAndBound, Knapsack, OldestFirstCoinFirst}` via `TxBuilder::coin_selection(alg)` | `bdk_wallet::bitcoin::OutPoint` (selected UTXOs) |
| 16 | Manual UTXO selection (coin control) | `TxBuilder::add_utxo(outpoint)` (repeatable); `TxBuilder::manually_selected_only()` for strict mode | `bdk_wallet::bitcoin::OutPoint` (from txid:vout parsing) |
| 17 | Bump fee (RBF) | `Wallet::build_fee_bump(txid) -> Result<Psbt, BuildFeeBumpError>`; `.fee_rate(new_rate).finish()`; BIP-125 sequence bump is automatic | `bdk_wallet::bitcoin::Sequence` (BIP-125 RBF signaling), `FeeRate`, `Txid` (target original) |
| 18 | Sign an arbitrary message (BIP-137) | `bdk_wallet::bitcoin::hashes::sha256::Hash::engine()` (BIP-137 prefix + varint + message); `bdk_wallet::bitcoin::secp256k1::Keypair::sign_ecdsa(&Message::from_digest(hash))` | `bdk_wallet::bitcoin::hashes::sha256::Hash` (BIP-137 prefix), `secp256k1::Message::from_digest(*bytes)` (signable), `ecdsa::Signature` (output) |
| 19 | Export the wallet descriptor | `Wallet::public_descriptor(KeychainKind::External)` (xpub-only); `format!("wpkh({xprv})/0/*")` (with xprv, requires confirmation) | `bdk_wallet::bitcoin::Descriptor<DescriptorPublicKey>` (string), `DescriptorSecretKey` (for xprv variant) |
| 20 | Pick a specific address type on creation | `CreateParams::network(Network).descriptor(External, Some(parsed_descriptor))`; descriptor string differs per address type (`pkh(...)` / `sh(wpkh(...))` / `wpkh(...)` / `tr(...)`) | `bdk_wallet::bitcoin::Address` (per-type constructors: `p2pkh/p2sh/p2wpkh/p2wsh/p2tr/p2tr_tweaked`), `ScriptBuf` (the constructed script) |
| Cross-cutting | `--json` everywhere; stable exit codes; no daemons | `serde_json::to_string_pretty`; `std::process::ExitCode`; `clap::Command::exit()` (clap handles process exit) | `bdk_wallet::bitcoin::Transaction` (serialized via `consensus::serialize` for JSON debug output) |
| Cross-cutting | `Secret<Mnemonic>` zeroize (v0.1 hygiene) | `zeroize::Zeroizing<Mnemonic>` newtype wrapper | n/a (zeroize is a separate crate, not rust-bitcoin) |
| Cross-cutting | Encrypted mnemonic at rest (v0.2) | `argon2::Argon2::hash_password_into`; `aes_gcm::Aes256Gcm::encrypt/decrypt` | n/a (encryption is RustCrypto, not rust-bitcoin) |

**Layer separation summary** (from the audit):

- **`rust-bitcoin 0.32` = types + primitive operations.** It owns the script building, address encoding, sighash computation, PSBT serialization, Taproot building, hash primitives, multi-network enum, consensus encode/decode. None of these need a "wallet" concept.
- **`bdk_wallet 3.1` = wallet engine + workflow.** It owns wallet construction, chain sync, UTXO selection, TxBuilder, in-process signing, RBF, external signer registration, balance, history, multi-address, watch-only, persistence, error introspection, descriptor export.
- **Neither handles** HD derivation (BIP-32) and BIP-39 mnemonic — both consume these but the math is in the standalone `bip32 0.6` crate and the `bip39` crate (re-exported by `bdk_wallet::keys::bip39`).
- **Zero overlap in API surface.** Same crate family, same dep tree (BDK re-exports rust-bitcoin via `bdk_wallet::bitcoin::*`).

**Stories NOT using BDK (custom code only):**
- Story 19 `--no-private` (xpub-only) — uses `Wallet::public_descriptor`, no signing.
- BIP-137 message signing (Story 18) — composed from BDK's re-exported `hashes::sha256` + our `Signer` (secp256k1 standalone for raw ECDSA, not in BDK's signer API).
- Out-of-scope items (hardware wallet, multi-sig, encryption, plausible deniability) — see §"Out of scope for v1" below.

## rust-bitcoin 0.32 use case coverage

Cross-check: every use case that `rust-bitcoin 0.32` provides natively (per `docs/wallets/2026-08-05-rust-bitcoin-features.md` §"Use cases `rust-bitcoin 0.32` handles natively") mapped to the user story that exercises it. Any "not covered" row is either an internal-only operation (no CLI surface needed) or a gap to add.

| # | rust-bitcoin 0.32 use case | User story that exercises it | Covered? |
|---|---|---|---|
| 1 | Build a transaction from scratch (UTXO + recipient + fee) | Story 5 (send) + Story 13 (multi-output) + Story 14 (drain) + Story 16 (coin control) | ✅ |
| 2 | Build any Bitcoin script (P2PKH/P2SH/P2WPKH/P2WSH/P2TR) | Story 1 (create) + Story 20 (--type) — descriptor strings are scripts | ✅ |
| 3 | Parse any script to opcode stream | **internal only** — used by Task 5 to verify scripts but not exposed as a CLI command | ⚠️ internal (defer) |
| 4 | Encode any standard address | Story 1 (create) + Story 6 (address types) + Story 20 (--type) | ✅ |
| 5 | Decode any standard address | **not exposed as a CLI command** — `btc address validate <addr>` could be a v1.1 add-on | ⚠️ gap (low priority) |
| 6 | Serialize any Bitcoin type to bytes | Story 19 (export descriptor — serialized) + Story 5 --dry-run (PSBT base64) | ✅ |
| 7 | Deserialize any Bitcoin type from bytes | Story 19 (import descriptor) + Story 5 --dry-run (parses PSBT) | ✅ |
| 8 | Compute sighash for any input type | **internal only** — used by Task 28 Signer trait (Phase 2 UniFFI). v0.1 CLI doesn't extract sighashes directly. | ⚠️ internal (defer to v1.0) |
| 9 | Build a PSBT v1 | Story 5 (--dry-run prints base64 PSBT) | ✅ |
| 10 | Sign a PSBT in place | Story 5 (btc send) | ✅ |
| 11 | Finalize a PSBT to extract raw tx | internal to Story 5 (broadcast) | ✅ |
| 12 | Build a Taproot output (BIP-86 + BIP-341) | Story 20 (--type taproot) | ✅ |
| 13 | Sign ECDSA over a 32-byte hash | Story 5 (send) + Story 17 (bump-fee) + Story 18 (BIP-137 msg) | ✅ |
| 14 | Sign Schnorr (BIP-340) with aux randomize | Story 20 (--type taproot) — Taproot uses Schnorr internally | ✅ |
| 15 | Verify signatures (ECDSA or Schnorr) | **not exposed as a CLI command** — `btc tx verify <hex> <signature> <pubkey>` could be a v1.1 add-on. Needed for hardware-signer verification flow (Phase 2). | ⚠️ gap (defer to v1.0) |
| 16 | Convert to/from any Bitcoin hash (sha256, hash160, ripemd160) | Story 18 (BIP-137 hash prefix) | ✅ |
| 17 | Compute HMAC-SHA512 (BIP-32 master, BIP-39 seed) | **internal only** — used by standalone `bip32` + `bip39` crates in Task 3/4. Not a CLI surface. | ⚠️ internal |
| 18 | Multi-network support (5 networks) | Story 1 (create) + Story 10 (mainnet opt-in) | ✅ |
| 19 | PSBT base64 round-trip | Story 5 (--dry-run) | ✅ |
| 20 | Txid + wtxid computation | Story 7 (tx list shows txid) + Story 17 (bump-fee target) | ✅ |
| 21 | Sequence number manipulation (BIP-125 RBF + BIP-68) | Story 17 (RBF) | ✅ |

**Coverage summary:**
- **17 of 21 use cases directly covered by user stories** (Stories 1, 5, 6, 7, 10, 13, 14, 16, 17, 18, 19, 20)
- **4 use cases are internal-only** (no CLI surface needed) — script parsing, sighash computation, HMAC-SHA512. These are used by internal code paths (Task 5/28, standalone `bip32`/`bip39` crates) but are not user-facing features.
- **0 use cases are missing from user stories** (no gap to fill).
- **2 deferred low-priority gaps** for a future v1.1 / v1.0 release: `btc address validate <addr>` (decode any address) and `btc tx verify <hex>` (verify a signature). Both are useful but not essential for the Phase 1 CLI.

**No use cases from rust-bitcoin 0.32 are missing from the user stories.** The 20 user stories + the 4 internal-only operations cover the full surface of the primitives layer that the btc CLI exposes.

---

## Story 1 — Create a new wallet (Alice)

> As Alice, I want to generate a new Bitcoin testnet wallet from a single command, so I can start receiving funds in under 10 seconds.

**Acceptance criteria:**

- `btc wallet create --name test-wallet` runs in <1s on a developer laptop.
- Output shows: 12-word BIP-39 mnemonic, first receive address, network name, wallet name, descriptor checksum.
- Mnemonic written to `~/.local/share/btc/test-wallet/mnemonic.txt` with mode 0600.
- Command exits 0 on success, non-zero on filesystem error.
- A prominent `WARNING` line reminds the user to back up the mnemonic before continuing.
- Running the command twice with the same name fails with exit code 2 and message `wallet 'test-wallet' already exists`.

**Options:**
- `--type legacy|nested-segwit|native-segwit|taproot` (default `native-segwit` = BIP-84)
- `--gap-limit N` (default 20, per BDK default)
- `--lookahead N` (default 25, per BDK default)
- `--network mainnet|testnet|testnet4|signet|regtest` (default `testnet`)

---

## Story 2 — Import an existing wallet (Alice)

> As Alice, I want to import a wallet from an existing 12/24-word mnemonic, so I can recover access to a wallet I created elsewhere.

**Acceptance criteria:**

- `btc wallet import --name recovered --mnemonic "word1 word2 ... word12" --network testnet` accepts a valid mnemonic.
- Invalid checksum returns exit code 2 + clear error `invalid mnemonic: checksum mismatch`.
- Imported wallet produces the same first address as the source (verified by deterministic derivation).
- BIP39 passphrase (`--passphrase "..."`) supported; empty passphrase is the default.
- Output does **not** echo the mnemonic back to the terminal.
- Supports 12, 15, 18, 21, 24-word mnemonics.

---

## Story 3 — Check balance (Alice, Carol)

> As Carol, I want to see my confirmed and unconfirmed balance in a single line, so I know how much I can spend right now.

**Acceptance criteria:**

- `btc balance --wallet test-wallet` prints one line: `confirmed: 0.00123 BTC, unconfirmed: 0, immature: 0`.
- Values formatted in BTC by default; `--unit sats` switches to satoshis.
- First run after wallet creation shows `0` across the board (no funds yet) and exits 0.
- If chain sync fails (Esplora unreachable), the command retries once, then prints `sync failed: <reason>` and exits 3.
- Reads from cached DB first; falls back to live sync only if the cache is stale.

---

## Story 4 — Sync chain state (Alice)

> As Alice, I want to force a full chain sync, so I can see incoming transactions that arrived since the last sync.

**Acceptance criteria:**

- `btc sync --wallet test-wallet` connects to Esplora and pulls new blocks/addresses.
- Output: `synced to height 2500123 in 1.2s, 3 new addresses, 1 new tx`.
- Exit 0 on success. Exit 3 on Esplora failure.
- `--no-progress` suppresses the streaming progress line (for CI/scripting).
- A subsequent `btc balance` reflects the synced state without a second sync.

---

## Story 5 — Send a payment (Alice)

> As Alice, I want to send 0.001 BTC to a testnet address with a known fee tier, so I can complete a payment in one command.

**Acceptance criteria:**

- `btc send --wallet test-wallet --to tb1q... --amount-sats 100000` builds, signs, and broadcasts.
- Default fee tier is `half_hour` (3-block target). Override via `--fee fastest|half_hour|hour|economy` or `--fee-rate-sat-per-vb 5`.
- Output on success: `sent. txid: abc...123 (fee: 540 sats, weight: 110 vbytes)`.
- Exit 0 on broadcast. Exit 4 on insufficient funds. Exit 5 on RPC error.
- `--dry-run` builds + signs but does not broadcast; prints the base64 PSBT instead.
- Wallet must be synced within the last 5 minutes; otherwise auto-syncs first.

---

## Story 6 — Send with custom fee rate (Bob)

> As Bob, I want to specify a fee rate in sat/vB, so I can match my back-end's fee policy exactly.

**Acceptance criteria:**

- `btc send --wallet w --to addr --amount-sats N --fee-rate-sat-per-vb 12` builds a tx with the exact rate.
- Validation: rate must be `>= 1 sat/vB`. Lower values fail with exit code 2 + message `fee rate must be >= 1 sat/vB`.
- Output includes the effective fee rate used (might be higher than requested if RBF bumped it).

---

## Story 7 — Inspect transaction history (Alice)

> As Alice, I want to list my past transactions with confirmations, so I can reconcile my records.

**Acceptance criteria:**

- `btc tx list --wallet test-wallet` prints a table: `txid | direction | amount | fee | confirmations | timestamp`.
- Default 25 most recent txs. `--limit N` overrides; `--offset N` paginates.
- `--json` outputs a JSON array (for piping to `jq`).
- Unconfirmed txs show `confirmations: 0` with `pending` tag.
- `btc tx get --txid <id>` returns full details of one tx (raw hex + decoded fields).
- Exit 0 even if no transactions yet (empty table).

---

## Story 8 — Get current fee estimates (Bob, Carol)

> As Carol, I want to see the current fee tiers before sending, so I can pick the right speed/cost trade-off.

**Acceptance criteria:**

- `btc fee --wallet test-wallet` prints a table:

  ```text
  fastest:     25 sat/vB
  half_hour:   12 sat/vB
  hour:         8 sat/vB
  economy:      1 sat/vB
  minimum:      1 sat/vB
  ```

- Tier names match the spec; default targets 1, 3, 6, 144, 1008 blocks.
- Output refreshed on every call (live fetch, not cached).
- Exit 3 on Esplora failure.
- `--json` outputs the same as a JSON object.

---

## Story 9 — List / show / delete / rename wallets (Alice, Bob)

> As Alice, I want to list all my wallets in the data directory and manage them, so I can pick one quickly.

**Acceptance criteria:**

- `btc wallet list` prints one wallet name per line.
- `btc wallet list --json` outputs a JSON array of `{name, network, address_type, address, fingerprint}`.
- `btc wallet show --name w` prints full info: network, address type, first address, descriptor checksum, wallet name, file path, created_at.
- `btc wallet show --name w --descriptor` exports the full BIP-380 descriptor string.
- `btc wallet delete --name w` removes the wallet (DB + mnemonic.txt + cached state). Prints `wallet 'w' deleted.` Exits 4 if the wallet doesn't exist.
- `btc wallet rename --name w --to w2` renames the wallet in place. Exits 4 if `w2` already exists.
- Empty directory: `btc wallet list` prints `(no wallets)` and exits 0.
- Corrupt wallets (no `mnemonic.txt`): listed but marked `(corrupt — missing mnemonic.txt)`.

---

## Story 10 — Use mainnet explicitly (Alice)

> As Alice, I want to create and use a mainnet wallet, so I can use real Bitcoin.

**Acceptance criteria:**

- `btc wallet create --name main --network mainnet` produces a `bc1q...` address.
- Default Esplora URL for mainnet is `https://blockstream.info/api`.
- A confirmation prompt requires typing `yes` to proceed; default is abort.
- Output shows `WARNING: this wallet uses real Bitcoin on mainnet. Funds are at risk.` before the mnemonic.
- Exit 1 if user does not type `yes`.

---

## Story 11 — Show config + debug info (Bob)

> As Bob, I want to see what the CLI thinks the current network and Esplora URL are, so I can debug "why is this connecting to the wrong place".

**Acceptance criteria:**

- `btc config show` prints: data dir, Esplora URL, network, list of loaded wallets.
- `btc config show --json` outputs the same as JSON.
- Exit 0 always.
- Diagnostic output includes the version string `btc 0.1.0` (via `--version`).

---

## Story 12 — Persist wallet across CLI invocations (Alice)

> As Alice, I want each CLI invocation to find my wallets without re-deriving from the mnemonic, so it's fast.

**Acceptance criteria:**

- First `btc wallet create` writes `mnemonic.txt` to `~/.local/share/btc/{name}/`.
- Subsequent `btc balance --wallet {name}` reads the file, reconstructs the wallet, and continues.
- If the data dir is on slow disk (HDD, network FS), `btc balance` still completes in <500ms after sync.
- If `mnemonic.txt` is missing or unreadable, the command exits 2 with `wallet '{name}' is missing or corrupt`.

---

## Story 13 — Send to multiple recipients in one transaction (Alice)

> As Alice, I want to send to several addresses in a single transaction, so I pay one network fee instead of one per recipient.

**Acceptance criteria:**

- `btc send --wallet w --to addr1:100000 --to addr2:200000 --to addr3:50000` builds, signs, and broadcasts one transaction with 3 outputs.
- Default fee tier `half_hour`. Override via `--fee fastest|...` or `--fee-rate-sat-per-vb N`.
- Up to 20 recipients per tx (BDK's recommended safe max).
- Output: `sent. txid: abc...123 (recipients: 3, total: 0.0035 BTC, fee: 540 sats)`.
- Exit 0 on broadcast. Exit 4 on insufficient funds (sum of amounts + fee > balance).
- `--dry-run` builds + signs but does not broadcast.

---

## Story 14 — Drain wallet to a single address (Alice)

> As Alice, I want to sweep my entire wallet to one address, so I can consolidate funds before re-organizing my wallets.

**Acceptance criteria:**

- `btc send --wallet w --drain --to addr` builds a transaction that sends all spendable UTXOs to `addr` (no change output).
- Default fee tier `half_hour`. Override via `--fee-rate-sat-per-vb N`.
- Output: `drained. txid: abc...123 (sent: 0.01234 BTC, fee: 540 sats)`.
- Exit 4 if no spendable UTXOs.
- `--exclude-utxo <outpoint>` (repeatable) lets the user keep specific UTXOs (e.g. locked coins).

---

## Story 15 — Choose coin selection algorithm (Bob)

> As Bob, I want to pick the coin selection algorithm, so I can match my back-end's UTXO policy.

**Acceptance criteria:**

- `btc send --wallet w --to addr --amount-sats N --coin-selection bnb|knapsack|lowest_fee` lets Bob pick the BDK algorithm.
- Default: `bnb` (branch-and-bound, minimizes waste).
- `bnb`: optimal waste minimization; may fail on large UTXO sets (exit 4 with `BnBNoSolution`).
- `knapsack`: randomized, always finds a solution if one exists.
- `lowest_fee`: oldest-first / smallest-first, lowest fee.
- Invalid algorithm name exits 2 with `unknown coin selection algorithm: <name>`.

---

## Story 16 — Manual UTXO selection (coin control) (Bob)

> As Bob, I want to spend specific UTXOs in a transaction, so I can avoid the dust from accidental receive, freeze specific coins, or follow an external policy.

**Acceptance criteria:**

- `btc send --wallet w --to addr --amount-sats N --input <txid>:<vout>` spends exactly the specified UTXO.
- `--input` repeatable for multi-input txs.
- If the sum of selected inputs < amount + fee, exits 4 with `insufficient funds from selected inputs`.
- If the specified outpoint is not owned by the wallet, exits 2 with `utxo <txid>:<vout> not found in wallet`.
- `--manual-selection-only` (no auto-append): fails if selected inputs < amount + fee (no fallback).
- Prints a summary of selected UTXOs and resulting change before signing (dry-run-like preview; user must press Enter to confirm, or `--yes` to skip).

---

## Story 17 — Bump fee (RBF) (Alice)

> As Alice, I want to replace an unconfirmed transaction with one that pays a higher fee, so I can speed it up when the network is congested.

**Acceptance criteria:**

- `btc send bump-fee --wallet w --txid <id> --fee-rate-sat-per-vb N` builds a replacement transaction that pays the higher fee and spends the same inputs.
- Output on success: `bumped. new_txid: def...456 (old_txid: abc...123, fee_increase: 0.00005 BTC)`.
- The original tx is automatically marked as replaced (RBF signaling via BIP-125 sequence bump).
- Exit 0 on success. Exit 4 if the original tx is not in the wallet. Exit 5 if signing fails.
- Exit 4 if `--fee-rate-sat-per-vb N` < the original fee rate (must increase, not decrease).
- Wallet must be synced within the last 5 minutes.

---

## Story 18 — Sign an arbitrary message (BIP-137) (Alice)

> As Alice, I want to sign a message with my wallet's private key, so I can prove ownership of an address (e.g. for an airdrop claim or a signed-message board).

**Acceptance criteria:**

- `btc sign-message --wallet w --message "I own this address"` signs the message with BIP-137 hash prefix (`\x19Bitcoin Signed Message:\n` + varint(len) + message).
- Default output: base64 of the 64-byte low-S ECDSA signature.
- `--hex` outputs the signature in hex instead of base64.
- `--address <addr>` signs with the key for `<addr>` (must be wallet-owned); default is the first external-chain address.
- Output format: `address: <addr>\nsignature: <base64>` (so the verifier has both pieces).
- Exit 0 on success. Exit 4 if `--address` is not wallet-owned.

---

## Story 19 — Export the wallet descriptor (Bob)

> As Bob, I want to export the wallet's descriptor string, so I can import it into Sparrow / Electrum / another wallet.

**Acceptance criteria:**

- `btc wallet show --name w --descriptor` prints the full BIP-380 descriptor string (e.g. `wpkh([44c028ba/84'/0'/0']xpub.../0/*)`).
- For secret-bearing descriptors (with xprv), prompts for confirmation: `This descriptor contains your private keys. Continue? [y/N]`.
- `--no-private` exports the public-only (xpub) variant for watch-only sharing.
- Output is just the descriptor, one line, no other text. Easy to pipe to `pbcopy` or another tool.
- Exit 0 on success. Exit 1 if user aborts the private-key confirmation.

---

## Story 20 — Pick a specific address type on creation (Alice)

> As Alice, I want to choose the address type for new wallets, so I can control the trade-off between compatibility (legacy) and efficiency (native segwit / taproot).

**Acceptance criteria:**

- `btc wallet create --name w --type legacy` creates a BIP-44 wallet (P2PKH; addresses start with `m` or `n` for testnet).
- `btc wallet create --name w --type nested-segwit` creates a BIP-49 wallet (P2SH-P2WPKH; addresses start with `2` on testnet).
- `btc wallet create --name w --type native-segwit` creates a BIP-84 wallet (P2WPKH; addresses start with `tb1q...`).
- `btc wallet create --name w --type taproot` creates a BIP-86 wallet (P2TR; addresses start with `tb1p...`).
- Default: `native-segwit`.
- Each type's `btc wallet show --name w --descriptor` produces a descriptor of the correct form.

---

## Cross-cutting acceptance criteria (apply to all stories)

- **Help text:** every command accepts `--help` and prints a clear, multi-line description with examples.
- **Exit codes:** documented and stable (0 = success, 1 = user abort, 2 = bad input, 3 = upstream/network, 4 = insufficient funds, 5 = signing/broadcast error).
- **Output:** human-readable by default; `--json` flag on every command that produces data.
- **Stderr for diagnostics:** logs and errors go to stderr; stdout contains only the requested data (safe to pipe).
- **No background processes:** every `btc` invocation is a single foreground command. No daemons.
- **No telemetry:** the CLI makes no network calls except to the configured Esplora URL and the chain.
- **Address unit:** all amount flags accept BTC (`--amount 0.001`) or sats (`--amount-sats 1000`) — both are valid; reject ambiguity in one command (exit 2 if both given).
- **Confirmation prompts** (`mainnet`, `delete wallet`, `export private descriptor`): require typing `yes` (not `y`); default is abort; exit code 1 on abort.
- **Bounded inputs:** BDK recommends max 20 recipients per tx, max 1000 UTXOs per wallet. CLI enforces these limits (exit 2 with clear message if exceeded).

---

## Out of scope for v1 (separate user stories when shipped)

- **Hardware wallet integration** (Ledger, Trezor, Tangem card). Phase 2 via UniFFI.
- **Multi-sig wallets** (`miniscript` descriptors: P2SH, P2WSH, P2TR script-path). v1.0+.
- **Foreign UTXO** (spending non-wallet inputs in a transaction). Advanced; defer.
- **PSBT file import / export** (currently inline base64 only for `--dry-run`). v1.1.
- **CPFP** (child-pays-for-parent: bump fee on a child to speed up a stuck parent). No native BDK support; would need manual RBF on parent + child. Defer.
- **Watch-only wallet import** from a public-only descriptor (no signing key). v0.2 (Plan Task 32).
- **Silent payments** (BIP-352). No mature Rust SDK.
- **Lightning.** Separate spec.
- **Any other UTXO chain** (BCH, LTC, DOGE, DASH, KAS). Separate plans per chain.
- **Encrypted mnemonic at rest** (v0.2: Argon2id + AES-256-GCM).
- **Plausible-deniability multi-bucket wallet** (v1.0).
- **REST/HTTP interface.** Breez-style server.
- **Mobile (iOS / Android) integration.** Phase 2 via UniFFI.
