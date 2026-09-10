# `sol-wallet-core` v0.1 — public API reference

**Audience:** CLI consumers (`crates/sol/`), FFI binding (mobile + desktop), and any external Rust consumer that links the crate directly.

**Source of truth:** `rust-wallet-app/crates/sol-wallet-core/src/`. This document is generated from the actual `pub` surface (verified against `src/`); the deep-dive (`docs/wallets/2026-09-08-solana-rust-sdks-deep-dive.md`) names the *module shape* but not every function signature.

**Stability:** v0.1. The crate's pub surface is *not* SemVer-frozen. The encrypted wallet file format **is** backward-compatible via `#[serde(default)]` on new fields.

---

## Design pattern

`sol-wallet-core` = **Phantom-equivalent surface** (4 constructors + 3 methods + 1 getter on `Wallet`) wrapping `bip39` + `ed25519-bip32` + `solana_sdk::signer::Keypair`. NO custom HD wrapper module. Path strings NOT exposed to callers; numeric `--account` + `--address-index` flags only.

Mirrors `bitcoin-wallet-core` (`cdylib` + 4-trait PAL) and `polygon-wallet-core` (251 LOC thin wrapper). Solana has no upstream EVM-style `evm-wallet-core` analog so `sol-wallet-core` is fat standalone (~4500 LOC V0.1).

All crypto delegated to:
- `solana-sdk 4.1.0` — Keypair, Pubkey, Message, VersionedTransaction, Signer trait
- `spl-token 9.0.0` — classic SPL instructions
- `spl-token-2022 11.0.0` — Token-2022 instructions
- `spl-associated-token-account 8.0.0` — ATA derivation + create
- `ed25519-bip32 0.4.3` — SLIP-0010 HD derivation (Phantom convention)
- `bip39 2.0` — mnemonic generation + seed derivation

RPC + persistence + encryption + FFI wallet-local.

---

## Module map

The crate follows the 5-layer PAL design (same as `tron-wallet-core` + `bitcoin-wallet-core`). Layers 2-5 are *consumers* of Layer 4 (pure Rust core).

| Layer | Module(s) | Purpose |
|---|---|---|
| 4 — Core | `wallet`, `wallet_manager`, `address`, `tx/{builder,sign,broadcast,summary}`, `crypto`, `error`, `disambig`, `config`, `tokens` | Portable pure Rust. No `tokio::net`, no `reqwest`. |
| 3 — PAL traits | `platform::{storage,info,network,clock}` | 4 trait surfaces the platform layer implements. |
| 2 — Platform impls | `platform::{desktop,android,ios,test}` | Concrete `WalletStorage` / `NetworkClient` / `Clock` / `PlatformInfo` per platform. |
| 1 — Lib root | `lib.rs` | Re-exports the above via `pub mod`. |

---

## `wallet` — Phantom-equivalent surface (PRIMARY API)

The only public Wallet type. Mirrors Phantom's user-facing API exactly.

```rust
pub struct Wallet(solana_sdk::signature::Keypair);
pub struct ReadOnlyWallet(solana_sdk::pubkey::Pubkey);

impl Wallet {
    /// Phantom: "Import secret phrase" → derives `m/44'/501'/0'/0'/0`
    pub fn fromMnemonic(phrase: &str) -> Result<Self>;
    
    /// Phantom: "Add account" → derives `m/44'/501'/{account}'/0'/{address_index}`
    pub fn fromMnemonicAt(phrase: &str, account: u32, address_index: u32) -> Result<Self>;
    
    /// Phantom: "Import private key" (base58 64-byte secret)
    pub fn fromBase58(secret: &str) -> Result<Self>;
    
    /// Phantom: "Watch-only" (read-only, no sign methods)
    pub fn fromPublicKey(pubkey: solana_sdk::pubkey::Pubkey) -> ReadOnlyWallet;
    
    /// Phantom: base58 Ed25519 pubkey (32 bytes)
    pub fn publicKey(&self) -> solana_sdk::pubkey::Pubkey;
    
    /// Phantom: sign arbitrary VersionedTransaction
    pub fn signTransaction(&self, tx: solana_sdk::transaction::VersionedTransaction) 
        -> Result<solana_sdk::transaction::VersionedTransaction>;
    
    /// Phantom: sign arbitrary bytes (Ed25519 over SHA-512 truncated)
    pub fn signMessage(&self, msg: &[u8]) -> solana_sdk::signature::Signature;
}

impl ReadOnlyWallet {
    pub fn publicKey(&self) -> solana_sdk::pubkey::Pubkey;
    // NO sign methods — verifier only
}
```

### Internals (NOT exposed)

Derivation flow hidden behind the API:

```rust
// crates/sol-wallet-core/src/wallet.rs (private)
fn from_mnemonic_internal(phrase: &str, account: u32, address_index: u32) -> Result<Keypair> {
    let mnemonic = bip39::Mnemonic::from_phrase(phrase, Language::English)?;
    let seed = bip39::Seed::new(&mnemonic, "");        // PBKDF2-HMAC-SHA512 2048 rounds
    let master = ed25519_bip32::XPrv::from_seed(&seed);  // SLIP-0010 master
    let path = format!("m/44'/501'/{}'/0'/{}", account, address_index);
    let child = master.derive(&path)?;                 // SLIP-0010 derive
    let secret = Zeroizing::new(child.to_bytes());     // 32-byte secret
    Keypair::try_from(&secret[..32])                   // → solana_sdk::Keypair
}
```

**Public API count: 4** (`fromMnemonic`, `fromMnemonicAt`, `fromBase58`, `fromPublicKey`) + 3 (`publicKey`, `signTransaction`, `signMessage`).

**Why no path strings exposed:**
- Phantom users never see `m/44'/501'/0'/0'/N` — they see "Account 0, Address 0" (numeric).
- Path string `m/44'/501'/{account}'/0'/{address_index}` hardcoded internally.
- Derivation at exactly 5 components, SLIP-44 coin 501 (SOL).
- Advanced users (Ledger HW path `m/44'/501'`, ZIP-32 paths) → use `sol keygen` raw CLI (V0.2 deferred).

**Ed25519 HD xpub limitation:** SLIP-0010 spec lacks parent public key. Watch-only = leaf `Pubkey` only, cannot derive sibling addresses without seed. Phantom uses the same limitation.

---

## `wallet_manager` — multi-wallet CRUD (CLI-only scaffolding)

Phantom has no analog (browser extension, single wallet). CLI needs `--wallet-id <uuid>` to pick from N on-disk wallets.

```rust
pub struct WalletManager {
    storage: Arc<dyn WalletStorage>,      // PAL trait
    cluster: SolanaCluster,
}

pub struct WalletSummary {
    pub id: uuid::Uuid,
    pub name: String,
    pub cluster: SolanaCluster,
    pub pubkey: solana_sdk::pubkey::Pubkey,
    pub created_at: u64,
    pub source: WalletSource,             // Mnemonic | PrivateKey | Imported
}

pub enum WalletSource {
    Mnemonic { words: u8, account: u32, address_index: u32 },
    PrivateKey,
    Imported,
}

impl WalletManager {
    pub fn create_with_mnemonic(
        &self,
        name: &str,
        words: u8,                        // 12 | 15 | 18 | 21 | 24
        account: u32,
        address_index: u32,
        password: &[u8],
    ) -> Result<uuid::Uuid>;
    
    pub fn import_from_phrase(
        &self,
        name: &str,
        phrase: &str,
        account: u32,
        address_index: u32,
        password: &[u8],
    ) -> Result<uuid::Uuid>;
    
    pub fn import_from_pk_file(
        &self,
        name: &str,
        pk_file: &Path,                   // Solflare/Phantom JSON format [u8; 64]
        password: &[u8],
    ) -> Result<uuid::Uuid>;
    
    pub fn unlock(&self, id: uuid::Uuid, password: &[u8]) -> Result<UnlockedWallet>;
    pub fn lock(&self, id: uuid::Uuid) -> Result<()>;
    pub fn summary(&self, id: uuid::Uuid) -> Result<WalletSummary>;  // NO decrypted secret
    pub fn list(&self) -> Result<Vec<WalletSummary>>;
    pub fn delete(&self, id: uuid::Uuid) -> Result<()>;
    pub fn rename(&self, id: uuid::Uuid, new_name: &str) -> Result<()>;
    pub fn pubkey(&self, id: uuid::Uuid) -> Result<solana_sdk::pubkey::Pubkey>;
    pub fn keypair(&self, id: uuid::Uuid) -> Result<solana_sdk::signature::Keypair>;
}

pub struct UnlockedWallet {
    pub id: uuid::Uuid,
    pub pubkey: solana_sdk::pubkey::Pubkey,
    // Wallet data internally, sign methods delegated to `Wallet` (Phantom API)
}
```

**Public API count: 11** (`create_with_mnemonic`, `import_from_phrase`, `import_from_pk_file`, `unlock`, `lock`, `summary`, `list`, `delete`, `rename`, `pubkey`, `keypair`).

**Why `summary` returns only public fields:** R3 mitigation — wallet show / CLI never prints decrypted mnemonic. `summary()` returns id + name + cluster + pubkey + created_at + source. To sign, caller must `unlock(id, password)` which returns an `UnlockedWallet` whose lifetime is bounded.

---

## `address`

```rust
pub fn pubkey_from_bytes(bytes: &[u8; 32]) -> Result<solana_sdk::pubkey::Pubkey>;
pub fn pubkey_to_base58(pubkey: &solana_sdk::pubkey::Pubkey) -> String;
pub fn is_on_curve(pubkey: &solana_sdk::pubkey::Pubkey) -> bool;
pub fn find_pda(seeds: &[&[u8]], program_id: &solana_sdk::pubkey::Pubkey) 
    -> (solana_sdk::pubkey::Pubkey, u8);     // (pda, bump_nonce)
pub fn parse_pubkey(s: &str) -> Result<solana_sdk::pubkey::Pubkey>;  // base58 + is_on_curve check
pub fn pubkey_short(pubkey: &solana_sdk::pubkey::Pubkey) -> String;   // first 4 + last 4 chars
```

**Public API count: 8** (parse + 5 encoders + 2 derivation helpers).

`is_on_curve` rejects PDA-from-bytes footgun (PDA may NOT be on Ed25519 curve). `parse_pubkey` rejects invalid base58 + off-curve PDA bytes as input.

---

## `tx::builder`

```rust
pub fn build_sol_transfer(
    from: &solana_sdk::pubkey::Pubkey,
    to: &solana_sdk::pubkey::Pubkey,
    lamports: u64,
    memo: Option<&str>,
) -> Result<solana_sdk::transaction::VersionedTransaction>;

pub fn build_spl_transfer_checked(
    payer: &solana_sdk::pubkey::Pubkey,
    source_ata: &solana_sdk::pubkey::Pubkey,
    mint: &solana_sdk::pubkey::Pubkey,
    dest_ata: &solana_sdk::pubkey::Pubkey,
    amount: u64,
    decimals: u8,                        // fetched dynamically via unpack_mint
    token_program_id: &solana_sdk::pubkey::Pubkey,  // classic OR Token-2022
    memo: Option<&str>,
) -> Result<solana_sdk::transaction::VersionedTransaction>;

pub fn build_spl_approve(
    owner: &solana_sdk::pubkey::Pubkey,
    ata: &solana_sdk::pubkey::Pubkey,
    mint: &solana_sdk::pubkey::Pubkey,
    delegate: &solana_sdk::pubkey::Pubkey,
    amount: u64,
    decimals: u8,
    token_program_id: &solana_sdk::pubkey::Pubkey,
) -> Result<solana_sdk::transaction::VersionedTransaction>;

pub fn build_spl_close_account(
    owner: &solana_sdk::pubkey::Pubkey,
    ata: &solana_sdk::pubkey::Pubkey,
    destination: &solana_sdk::pubkey::Pubkey,
    token_program_id: &solana_sdk::pubkey::Pubkey,
) -> Result<solana_sdk::transaction::VersionedTransaction>;

pub fn prepend_create_ata(
    payer: &solana_sdk::pubkey::Pubkey,
    owner: &solana_sdk::pubkey::Pubkey,
    mint: &solana_sdk::pubkey::Pubkey,
    token_program_id: &solana_sdk::pubkey::Pubkey,
) -> solana_sdk::instruction::Instruction;

pub fn prepend_compute_budget(
    cu_limit: u32,
    cu_price_micro_lamports: u64,
) -> [solana_sdk::instruction::Instruction; 2];
```

**Public API count: 6** (4 builders + 2 prepends).

`build_spl_transfer_checked` ALWAYS uses `transfer_checked` (with decimals param) — never `transfer` (footgun). `token_program_id` MUST be passed (classic or Token-2022); `disambig::reject_wrong_token_program` validates caller passed correct one based on `mint.owner`.

---

## `tx::sign`

```rust
pub fn sign_sol(
    tx: &mut solana_sdk::transaction::VersionedTransaction,
    keypair: &solana_sdk::signature::Keypair,
) -> Result<()>;

pub fn sign_spl(
    tx: &mut solana_sdk::transaction::VersionedTransaction,
    keypair: &solana_sdk::signature::Keypair,
) -> Result<()>;

pub fn sign_only_sol(
    keypair: &solana_sdk::signature::Keypair,
    message_bytes: &[u8],
) -> solana_sdk::signature::Signature;

pub fn sign_only_spl(
    keypair: &solana_sdk::signature::Keypair,
    message_bytes: &[u8],
) -> solana_sdk::signature::Signature;

pub fn txid(tx: &solana_sdk::transaction::VersionedTransaction) -> String;
```

**Public API count: 5**.

---

## `tx::broadcast`

```rust
pub async fn submit_sol(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &solana_sdk::transaction::VersionedTransaction,
    commitment: solana_sdk::commitment_config::CommitmentConfig,
) -> Result<solana_sdk::signature::Signature>;

pub async fn submit_spl(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &solana_sdk::transaction::VersionedTransaction,
    commitment: solana_sdk::commitment_config::CommitmentConfig,
) -> Result<solana_sdk::signature::Signature>;

pub async fn submit_sol_speedup(
    rpc: &solana_client::rpc_client::RpcClient,
    original_sig: &solana_sdk::signature::Signature,
    new_priority_fee: u64,
    keypair: &solana_sdk::signature::Keypair,
    to: &solana_sdk::pubkey::Pubkey,
    lamports: u64,
) -> Result<solana_sdk::signature::Signature>;     // emits NEW sig (no RBF on Solana)

pub async fn submit_spl_approve(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &solana_sdk::transaction::VersionedTransaction,
) -> Result<solana_sdk::signature::Signature>;

pub async fn simulate(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &solana_sdk::transaction::VersionedTransaction,
) -> Result<solana_client::rpc_response::RpcSimulateTransactionResult>;

pub async fn send_with_retry(
    rpc: &solana_client::rpc_client::RpcClient,
    tx: &solana_sdk::transaction::VersionedTransaction,
    max_attempts: u8,                  // default 3
) -> Result<solana_sdk::signature::Signature>;

pub async fn wait_for_confirm(
    rpc: &solana_client::rpc_client::RpcClient,
    sig: &solana_sdk::signature::Signature,
    commitment: solana_sdk::commitment_config::CommitmentConfig,
    timeout_secs: u64,                 // default 60
    poll_interval_secs: u64,           // default 2
) -> Result<solana_sdk::transaction::TransactionStatus>;
```

**Public API count: 7**.

`send_with_retry` retries on `BlockhashNotFound` with fresh blockhash each attempt (3 attempts default). `submit_sol_speedup` emits NEW signature with higher priority fee (Solana has no RBF).

---

## `chain::client`

Thin wrapper over `solana_client::rpc_client::RpcClient` + retry policy + cluster URL resolution.

```rust
pub struct SolanaClient {
    inner: solana_client::rpc_client::RpcClient,
    cluster: SolanaCluster,
}

impl SolanaClient {
    pub fn new(cluster: SolanaCluster) -> Result<Self>;
    pub fn from_config(config: &SolanaConfig) -> Result<Self>;
    
    pub async fn request_airdrop(&self, pubkey: &solana_sdk::pubkey::Pubkey, lamports: u64) 
        -> Result<solana_sdk::signature::Signature>;
    
    pub async fn get_latest_blockhash(&self) -> Result<solana_sdk::hash::Hash>;
    pub async fn get_balance(&self, pubkey: &solana_sdk::pubkey::Pubkey) -> Result<u64>;
    pub async fn get_account_info(&self, pubkey: &solana_sdk::pubkey::Pubkey) 
        -> Result<solana_sdk::account::Account>;
    pub async fn get_token_account_balance(
        &self, 
        ata: &solana_sdk::pubkey::Pubkey
    ) -> Result<UiTokenAmount>;
    pub async fn get_minimum_balance_for_rent_exemption(&self, data_len: usize) -> Result<u64>;
    pub async fn get_token_accounts_by_owner(
        &self, 
        owner: &solana_sdk::pubkey::Pubkey, 
        mint: &solana_sdk::pubkey::Pubkey
    ) -> Result<Vec<TokenAccount>>;
    pub async fn get_recent_prioritization_fees(&self) -> Result<Vec<PrioritizationFee>>;
    pub async fn get_signature_statuses(
        &self, 
        sigs: &[solana_sdk::signature::Signature]
    ) -> Result<Vec<Option<TransactionStatus>>>;
    pub async fn simulate_transaction(
        &self, 
        tx: &solana_sdk::transaction::VersionedTransaction
    ) -> Result<RpcSimulateTransactionResult>;
}
```

**Public API count: 12** (2 constructors + 10 RPC methods).

---

## `chain::account`

```rust
pub async fn discover_atas(
    rpc: &solana_client::rpc_client::RpcClient,
    owner: &solana_sdk::pubkey::Pubkey,
) -> Result<Vec<(solana_sdk::pubkey::Pubkey, solana_sdk::pubkey::Pubkey)>>;  // (mint, ata)

pub async fn fetch_token_supply(
    rpc: &solana_client::rpc_client::RpcClient,
    mint: &solana_sdk::pubkey::Pubkey,
) -> Result<u64>;

pub async fn fetch_decimals(
    rpc: &solana_client::rpc_client::RpcClient,
    mint: &solana_sdk::pubkey::Pubkey,
) -> Result<u8>;

pub async fn mint_token_program(
    rpc: &solana_client::rpc_client::RpcClient,
    mint: &solana_sdk::pubkey::Pubkey,
) -> Result<solana_sdk::pubkey::Pubkey>;     // returns TokenkegQ or TokenzQdB

pub fn derive_ata_with_program_id(
    owner: &solana_sdk::pubkey::Pubkey,
    mint: &solana_sdk::pubkey::Pubkey,
    token_program_id: &solana_sdk::pubkey::Pubkey,
) -> solana_sdk::pubkey::Pubkey;
```

**Public API count: 5**.

`mint_token_program` is the canonical helper for the Token-2022 vs classic SPL disambig guard. Caller MUST pass `token_program_id` based on result.

---

## `chain::pki`

SPKI pinning per RFC 7469 (Q7). Same shape as `tron-wallet-core/src/chain/pki.rs`.

```rust
pub struct SpkiPinnedVerifier { /* ... */ }

impl SpkiPinnedVerifier {
    pub fn new(pins: HashMap<String, Vec<String>>) -> Result<Self>;  // host → pins
    pub fn verify_pinned(&self, host: &str, cert: &rustls::Certificate) -> Result<()>;
    pub fn extract_spki_digest(cert_der: &[u8]) -> Result<String>;   // SHA-256 of SubjectPublicKeyInfo
    pub fn matches_pin_set(&self, host: &str, digest: &str) -> bool;
}
```

**Public API count: 4**.

V0.1 ships with empty pin set (Solana Labs no-pin policy). Cert transparency + standard rustls verification is the primary defense.

---

## `crypto`

Argon2id KDF + AES-256-GCM encryption (wallet file format).

```rust
pub struct EncryptedWallet {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
    pub tag: [u8; 16],
    pub salt: [u8; 16],
    pub kdf_params: KdfParams,
}

pub struct KdfParams {
    pub m_cost: u32,       // Argon2id memory (KB)
    pub t_cost: u32,       // iterations
    pub p_cost: u32,       // parallelism
}

pub fn encrypt_wallet(
    plaintext: &[u8],
    password: &[u8],
    kdf_params: KdfParams,
) -> Result<EncryptedWallet>;

pub fn decrypt_wallet(
    blob: &EncryptedWallet,
    password: &[u8],
) -> Result<Zeroizing<Vec<u8>>>;

pub fn derive_kdf_key(
    password: &[u8],
    salt: &[u8],
    params: KdfParams,
) -> Result<Zeroizing<Vec<u8>>>;     // Argon2id output

pub fn random_salt() -> [u8; 16];
```

**Public API count: 5** (`EncryptedWallet` + `KdfParams` structs + `encrypt_wallet` + `decrypt_wallet` + `derive_kdf_key` + `random_salt` = 6 including types).

V0.1 default KDF params: `m_cost = 65536` (64 MB), `t_cost = 3`, `p_cost = 1`.

---

## `config`

```rust
pub enum SolanaCluster { MainnetBeta, Devnet, Localnet }

pub struct SolanaConfig {
    pub cluster: SolanaCluster,
    pub rpc_url: String,                       // derived from cluster if None
    pub commitment: solana_sdk::commitment_config::CommitmentConfig,
    pub priority_fee: u64,                     // µLamports per CU
    pub cu_limit: u32,
    pub data_dir: PathBuf,
    pub default_wallet_id: Option<uuid::Uuid>,
}

impl SolanaConfig {
    pub fn load(data_dir: &Path) -> Result<Self>;
    pub fn save(&self) -> Result<()>;
    pub fn rpc_url_for(cluster: SolanaCluster) -> &'static str;
    pub fn set_rpc(&mut self, url: String) -> Result<()>;
    pub fn set_cluster(&mut self, cluster: SolanaCluster) -> Result<()>;
    pub fn set_priority_fee(&mut self, fee: u64);
}
```

**Public API count: 8** (2 enums + 1 struct + 5 methods + 1 RPC URL resolver).

Cluster URLs:
- `MainnetBeta` → `https://api.mainnet-beta.solana.com`
- `Devnet` → `https://api.devnet.solana.com`
- `Localnet` → `http://127.0.0.1:8899` (surfpool default)

---

## `tokens`

Stablecoin registry (bundled JSON via `include_str!`).

```rust
pub struct TokenInfo {
    pub symbol: String,                 // "USDC" | "USDT" | "PYUSD" | "USDS"
    pub name: String,                   // "USD Coin" | "Tether USD" | ...
    pub mint: solana_sdk::pubkey::Pubkey,
    pub decimals: u8,
    pub cluster: SolanaCluster,
    pub token_program: TokenProgram,    // Classic | Token2022
}

pub enum TokenProgram { Classic, Token2022 }

pub struct TokenRegistry { /* ... */ }

impl TokenRegistry {
    pub fn load_mainnet() -> Result<Self>;
    pub fn load_devnet() -> Result<Self>;
    pub fn by_symbol(&self, symbol: &str) -> Option<TokenInfo>;
    pub fn by_mint(&self, mint: &solana_sdk::pubkey::Pubkey) -> Option<TokenInfo>;
    pub fn decimals_for_mint(&self, mint: &solana_sdk::pubkey::Pubkey) -> Option<u8>;
    pub fn mints_for(&self, cluster: SolanaCluster) -> Vec<TokenInfo>;
}
```

**Public API count: 6** (TokenInfo + TokenProgram + TokenRegistry + 5 methods).

V0.1 ships with: USDC mainnet (`EPjFWdd5...`), USDC devnet (`4zMMC9sr...`), USDT mainnet (`Es9vMFrz...`), PYUSD mainnet (`2b1kV6Dk...`, Token-2022).

---

## `disambig`

Cross-cluster + cross-program footgun guards.

```rust
pub fn cluster_for_mint(mint: &solana_sdk::pubkey::Pubkey) -> Option<SolanaCluster>;
pub fn ensure_cluster_matches(
    mint: &solana_sdk::pubkey::Pubkey,
    cluster: SolanaCluster,
) -> Result<()>;
pub fn reject_wrong_token_program(
    mint: &solana_sdk::pubkey::Pubkey,
    passed_program_id: &solana_sdk::pubkey::Pubkey,
    expected_program_id: &solana_sdk::pubkey::Pubkey,
) -> Result<()>;
pub fn split_token_mint(
    mint: &solana_sdk::pubkey::Pubkey,
) -> (String, String);     // (symbol, base58 mint)
pub fn merge_atas(
    atas: &[solana_sdk::pubkey::Pubkey],
) -> solana_sdk::pubkey::Pubkey;     // canonical ATA for cluster
```

**Public API count: 5**.

`reject_wrong_token_program` is the Token-2022 vs classic SPL footgun guard — caller passes `token_program_id` based on `mint.owner`, this validates the choice BEFORE signing. Mismatched `token_program_id` seed produces DIFFERENT ATA address.

---

## `error`

```rust
pub enum Error {
    InvalidMnemonic(String),
    InvalidAddress(String),
    InvalidDerivationPath(String),
    InvalidCluster(String),
    InvalidMint(String),
    InvalidTokenProgram { mint: Pubkey, expected: Pubkey, actual: Pubkey },
    InvalidBlockhash(String),
    BlockhashExpired,
    SignFailed(String),
    TransactionBuild(String),
    BroadcastFailed { code: i32, msg: String },
    Node(String),
    NodeResponse(String),
    WalletNotFound(uuid::Uuid),
    WrongPassphrase,
    WalletLocked(uuid::Uuid),
    Encryption(String),
    Decryption(String),
    Config(String),
    Pal(String),
    Ffi(String),
    Serialization(String),
}
```

**Public API count: 21** (enum variants).

Mapped to CLI exit codes:
- `0` = success
- `1` = usage error (InvalidMnemonic / InvalidAddress / InvalidDerivationPath / InvalidCluster / InvalidMint)
- `2` = input error
- `3` = node error (Node / NodeResponse / BroadcastFailed)
- `4` = wallet/encryption error (WalletNotFound / WrongPassphrase / WalletLocked / Encryption / Decryption)
- `5` = signing/build error (SignFailed / TransactionBuild / BlockhashExpired)
- `6` = config error
- `99` = unknown / panics

---

## `ffi` — C ABI exports (mobile + desktop FFI)

```c
// C functions (extern "C")
const char* sol_version(void);
const char* sol_cluster_default(void);

int sol_wallet_from_mnemonic(
    const char* phrase,
    const char* password,
    uint8_t* out_pubkey,                // 32 bytes
    char* out_error,                    // caller-provided buffer
    size_t error_buf_len
);

int sol_wallet_from_base58(
    const char* base58_secret,          // 64-byte secret as base58
    const char* password,
    uint8_t* out_pubkey,
    char* out_error,
    size_t error_buf_len
);

int sol_wallet_pubkey(
    const uint8_t* wallet_id,           // 16 bytes UUID
    uint8_t* out_pubkey,                // 32 bytes
    char* out_error,
    size_t error_buf_len
);

int sol_balance_native(
    const char* rpc_url,
    const uint8_t* pubkey,              // 32 bytes
    uint64_t* out_lamports,
    char* out_error,
    size_t error_buf_len
);

int sol_balance_spl(
    const char* rpc_url,
    const uint8_t* owner_pubkey,        // 32 bytes
    const uint8_t* mint_pubkey,         // 32 bytes
    uint8_t* out_ata,                   // 32 bytes (derived)
    uint64_t* out_balance_raw,
    uint8_t* out_decimals,
    char* out_error,
    size_t error_buf_len
);

int sol_transfer_sol(
    const char* rpc_url,
    const uint8_t* from_pubkey,
    const uint8_t* to_pubkey,
    uint64_t lamports,
    const char* memo,                   // nullable
    uint8_t* out_signature,             // 64 bytes
    char* out_error,
    size_t error_buf_len
);

int sol_transfer_spl(
    const char* rpc_url,
    const uint8_t* from_pubkey,
    const uint8_t* to_owner_pubkey,
    const uint8_t* mint_pubkey,
    uint64_t amount_raw,
    uint8_t decimals,
    const char* memo,
    uint8_t* out_signature,
    char* out_error,
    size_t error_buf_len
);

int sol_sign_message(
    const uint8_t* wallet_id,
    const uint8_t* message,
    size_t message_len,
    uint8_t* out_signature,             // 64 bytes
    char* out_error,
    size_t error_buf_len
);

int sol_verify_message(
    const uint8_t* pubkey,
    const uint8_t* message,
    size_t message_len,
    const uint8_t* signature,
    int* out_valid,
    char* out_error,
    size_t error_buf_len
);

void sol_free_string(char* s);          // free strings returned by FFI
```

**Public API count: 12** C functions.

All FFI functions use `out_error` buffer pattern (no exceptions across FFI boundary). Strings allocated by FFI must be freed with `sol_free_string`. Wallet password material NEVER crosses FFI boundary — only used internally inside `sol_wallet_from_*` calls, zeroized before return.

---

## `util`

```rust
pub fn atomic_write(path: &Path, bytes: &[u8]) -> Result<()>;
pub fn human_lamports_to_sol(lamports: u64) -> String;     // "1.5 SOL"
pub fn human_token_amount(raw: u64, decimals: u8) -> String;     // "100.5 USDC"
pub fn zeroize_secret(secret: &mut [u8]);     // wrap zeroize::zeroize for FFI compat
```

**Public API count: 4**.

`atomic_write` = `write to .tmp + fsync + rename` (no corruption on panic).

---

## `platform` — PAL traits (4 traits × 14 methods)

Same pattern as `bitcoin-wallet-core` + `tron-wallet-core`.

```rust
pub trait WalletStorage: Send + Sync {
    fn read(&self, key: &str) -> Result<Vec<u8>>;
    fn write(&self, key: &str, value: &[u8]) -> Result<()>;
    fn delete(&self, key: &str) -> Result<()>;
    fn list(&self, prefix: &str) -> Result<Vec<String>>;
    fn exists(&self, key: &str) -> Result<bool>;
}

pub trait PlatformInfo: Send + Sync {
    fn os(&self) -> &'static str;       // "linux" | "macos" | "windows" | "ios" | "android"
    fn arch(&self) -> &'static str;     // "x86_64" | "aarch64"
    fn data_dir(&self) -> Result<PathBuf>;
    fn secure_random(&self, len: usize) -> Result<Vec<u8>>;
}

pub trait NetworkClient: Send + Sync {
    fn http_post(&self, url: &str, body: &[u8], headers: &[(&str, &str)]) 
        -> Result<Vec<u8>>;
    fn http_get(&self, url: &str, headers: &[(&str, &str)]) -> Result<Vec<u8>>;
    fn ws_connect(&self, url: &str) -> Result<Box<dyn WsStream>>;
}

pub trait Clock: Send + Sync {
    fn now_secs(&self) -> u64;
    fn sleep_ms(&self, ms: u64) -> Result<()>;
}

pub trait WsStream: Send + Sync {
    fn send(&mut self, msg: &[u8]) -> Result<()>;
    fn recv(&mut self) -> Result<Vec<u8>>;
    fn close(&mut self) -> Result<()>;
}
```

**Public API count: 4 traits + 14 methods** (5 + 4 + 3 + 3 - 1 WsStream separate).

V0.1 ships platform impls for: `desktop` (Linux/macOS/Windows), `android`, `ios`, `test`.

---

## `lib.rs` — re-exports

```rust
pub use solana_sdk;                 // re-export facade (Keypair, Pubkey, ...)
pub use solana_sdk::signer::Signer; // trait — Wallet::sign_transaction uses
pub use solana_sdk::signature::Signature;
pub use solana_sdk::transaction::VersionedTransaction;

pub mod wallet;
pub mod wallet_manager;
pub mod address;
pub mod crypto;
pub mod config;
pub mod disambig;
pub mod error;
pub mod ffi;
pub mod platform;
pub mod tokens;
pub mod tx;
pub mod util;
```

**Public API count: 12** `pub mod` declarations + 4 `pub use` re-exports.

---

## Total V0.1 public API count: ~148

| Module | API count | Notes |
|---|---|---|
| `lib.rs` | 12 | `pub use` re-exports + `pub mod` declarations |
| `address` | 8 | pubkey encode/decode + PDA derivation |
| `wallet` | 4 | Phantom-equivalent surface (4 constructors) |
| `wallet_manager` | 11 | multi-wallet CRUD (CLI-only) |
| `crypto` | 6 | Argon2id + AES-256-GCM |
| `config` | 10 | SolanaCluster + SolanaConfig |
| `tx::builder` | 6 | SOL + SPL + approve + close + prepend |
| `tx::sign` | 5 | SOL + SPL + sign-only + txid |
| `tx::broadcast` | 7 | submit + simulate + retry + wait |
| `chain::client` | 12 | RPC methods |
| `chain::pki` | 4 | SPKI pinning |
| `chain::account` | 5 | ATA discovery + decimals + program-id |
| `tokens` | 6 | TokenRegistry (stablecoins) |
| `disambig` | 5 | cluster + token-program footgun guards |
| `error` | 21 | Error enum variants |
| `ffi` | 12 | C ABI exports |
| `util` | 4 | atomic_write + human_* + zeroize_secret |
| `platform` | 14 | 4 traits × 14 methods |
| **Total** | **~148** | (Phantom-equivalent surface: HD 8 → Wallet keypair 4, -4 from previous) |

---

## CLI command surface (22 commands × 6 top-level)

```bash
sol wallet create       --words 12|24 --name --cluster --password [--account <N>] [--address-index <N>]
sol wallet import       --name --cluster --password --mnemonic|--mnemonic-file|--private-key-file [--account <N>] [--address-index <N>]
sol wallet show         --id [--json]
sol wallet list         [--json] [--all-clusters]
sol wallet delete       --id
sol wallet rename       --id --to
sol wallet balance      --wallet-id|--address [--token USDC|<addr>]
sol wallet send         --wallet-id|--mnemonic --to <addr>|--to-wallet <name|id> --amount [--unit] [--token] [--cu-limit] [--priority-fee] [--memo] [--dry-run] [--sign-only] [--wait] [--wait-finalized]
sol wallet send-speedup --wallet-id --sig --priority-fee

sol address new         --mnemonic [--mnemonic-file] --account <N> --address-index <N>
sol address pubkey      --wallet-id

sol balance             --address <addr> [--unit sol|lamport]
sol balance             --address <addr> --token USDC|<addr>

sol spl send            --wallet-id|--mnemonic --token USDC|<addr> --to <addr>|--to-wallet --amount [--cu-limit] [--priority-fee] [--memo] [--skip-ata-create] [--skip-memo-required] [--dry-run] [--sign-only] [--wait]
sol spl approve         --wallet-id --token --delegate --amount
sol spl allowance       --token --owner --delegate
sol spl balance         --address --token

sol tx get              --sig
sol tx wait             --sig [--wait-finalized] [--timeout <secs>]

sol config show         [--json]
sol config set-rpc      --url <https://...>
sol config set-cluster  --cluster mainnet|devnet|localnet
```

Phantom UX: `--account <N>` + `--address-index <N>` (numeric only). NO `--path <m/...>` flag. Path `m/44'/501'/{account}'/0'/{address_index}` hardcoded internally (Phantom/Solflare convention).

---

## V0.x deferred

| Surface | Version | Note |
|---|---|---|
| Passpharded mnemonic (BIP-39 25th word) | V0.2 | `bip39::Seed::new(mnemonic, passphrase)` |
| `sign_message` / `verify_message` API exposure | V0.1.5 | `Wallet::signMessage` exists; CLI flag V0.1.5 |
| Hardware wallet (Ledger) | V1.x | `solana-remote-wallet` integration |
| Address Lookup Tables (ALTs) | V0.1.5 | `VersionedTransaction::V0` with LUT |
| Durable nonces | V0.1.5 | `nonce_account` + `advance_nonce_account` |
| Jito tip routing | V0.1.5 | gated `jito` Cargo feature |
| Stake account | V0.2 | `stake::StakeState` program |
| Token-2022 Confidential Transfers | V0.3 | `solana-zk-sdk` |
| NFT (Metaplex) | EXCLUDED | `Metaplex NFT Open Source License v1.0` blocker |
| Watch-only from xpub | NOT POSSIBLE | Ed25519 HD has no xpub |
