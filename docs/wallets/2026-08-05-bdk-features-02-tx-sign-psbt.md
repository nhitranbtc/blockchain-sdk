# `bdk_wallet 3.1` — Categories 4–6: Transaction Building, Signing, PSBT

**Date:** 2026-08-05
**Scope:** Public API enumeration for the three categories the user asked about, against `bdk_wallet 3.1.0` (released 14 June 2026, source-of-truth doc version).
**Verification sources:**

- `https://docs.rs/bdk_wallet/3.1.0/...` (rustdoc pages)
- inline source citations use docs.rs source URLs of the form `https://docs.rs/bdk_wallet/latest/src/bdk_wallet/wallet/<file>.rs.html#L<line>` — these resolve to the **same lines in `bdk_wallet 3.1.0`** because `latest` is pinned to `3.1.0` on this date (the docs.rs page header confirms "bdk_wallet-3.1.0", "14 June 2026").

**Dependency snapshot for 3.1.0** (from the docs.rs crate page):

| Dependency | Version |
| --- | --- |
| `bitcoin` | `^0.32.8` |
| `miniscript` | `^12.3.5` |
| `bdk_chain` | `^0.23.3` |
| `secp256k1` (transitive via bdk_chain) | `0.29.1` |
| `rand_core` | `^0.6.4` |

---

## Category 4: Transaction Building (`TxBuilder`)

### 4.1 Entry point — `Wallet::build_tx`

| Item | Value |
| --- | --- |
| **Signature** | `pub fn build_tx(&mut self) -> TxBuilder<'_, DefaultCoinSelectionAlgorithm>` |
| **Receiver** | `&mut self` |
| **Returns** | Blank `TxBuilder` with the default coin-selection algorithm pre-wired |
| **Docs** | `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.build_tx` |
| **Source** | `crates/wallet/src/wallet/mod.rs.html#L1593-1726` (docs.rs pages attribute this to that line range; it is the `build_tx` block in the `Wallet` impl) |
| **BDK 3.1 status** | Stable. |

> Note: there is **no** `Wallet::build_tx_with` (no constructor variant for an explicit `CoinSelectionAlgorithm`); you set the algorithm with `TxBuilder::coin_selection(...).build_tx()`.

### 4.2 Summary table — TxBuilder chainable methods

All methods take `&mut self` and return `&mut Self` unless noted. Return types and overloads follow the canonical BDK 1.x → 3.x signatures, re-checked against the 3.1.0 doc page (`TxBuilder::set_exact_sequence` etc.).

| Method | Signature (Rust) | Status in 3.1 | Notes |
| --- | --- | --- | --- |
| `add_recipient` | `fn add_recipient(&mut self, script_pubkey: impl Into<ScriptBuf>, amount: Amount) -> &mut Self` | ✅ | Most-used. Source: `crates/wallet/src/wallet/tx_builder.rs.html#L680-684`. |
| `set_recipients` | `fn set_recipients(&mut self, recipients: Vec<(ScriptBuf, Amount)>) -> &mut Self` | ✅ | Replaces the existing list (`#670-677`). |
| `add_utxo` | `fn add_utxo(&mut self, outpoint: OutPoint) -> Result<&mut Self, AddUtxoError>` | ✅ | (`#387-399`). |
| `add_utxos` | `fn add_utxos(&mut self, outpoints: &[OutPoint]) -> Result<&mut Self, AddUtxoError>` | ✅ | Atomic: if any fails, none are added (`#325-327`). |
| `add_unspendable` | `fn add_unspendable(&mut self, unspendable: OutPoint) -> &mut Self` | ✅ | (`#496-512`). |
| `unspendable` | `fn unspendable(&mut self, unspendable: Vec<OutPoint>) -> &mut Self` | ✅ | Replaces the list (`#479-482`). |
| `manually_selected_only` | `fn manually_selected_only(&mut self) -> &mut Self` | ✅ | Forces use of only `add_utxo`/`add_utxos` set (`#470-473`). |
| `add_foreign_utxo` | `fn add_foreign_utxo(&mut self, outpoint: OutPoint, psbt_input: Input, satisfaction_weight: Weight) -> Result<&mut Self, AddForeignUtxoError>` | ✅, **EXPERIMENTAL** | Adds UTXO not tracked by this wallet (`#403-453`). Out-of-wallet UTXOs. ⚠ Note: caller does **not** call this on `Wallet`; it's a `TxBuilder` method. |
| `add_foreign_utxo_with_sequence` | same as `add_foreign_utxo` plus `sequence: Sequence` | ✅, **EXPERIMENTAL** | (`#461-464`). |
| `set_foreign_utxo` | – | ❌ **Not in 3.1** | The 0.x/1.x name; BDK 3.x uses `add_foreign_utxo`. **Verify in spike** if a wrapper exists in some downstream crate. |
| `do_not_spend_change` | `fn do_not_spend_change(&mut self) -> &mut Self` | ✅ | Equivalent to `change_policy(ChangeSpendPolicy::ChangeForbidden)` (`#574-577`). |
| `only_spend_change` | `fn only_spend_change(&mut self) -> &mut Self` | ✅ | Equivalent to `change_policy(ChangeSpendPolicy::OnlyChange)` (`#582-585`). |
| `change_policy` | `fn change_policy(&mut self, change_policy: ChangeSpendPolicy) -> &mut Self` | ✅ | (`#592-595`). Underlying enum: `enum ChangeSpendPolicy { ChangeForbidden, OnlyChange, ChangeAllowed }` (verify variants in spike — doc only shows `TxOrdering`/`ChangeSpendPolicy` enums). |
| `fee_rate` | `fn fee_rate(&mut self, fee_rate: FeeRate) -> &mut Self` | ✅ | Default 1 sat/vB (Bitcoin Core default relay) (`#175-178` impl header; doc body `#189-192`). |
| `fee_absolute` | `fn fee_absolute(&mut self, fee_amount: Amount) -> &mut Self` | ✅ | Mutually exclusive with `fee_rate`; whichever was called last wins (`#257-269`). |
| `drain_wallet` | `fn drain_wallet(&mut self) -> &mut Self` | ✅ | Respects `unspendable` and change policy (`#620-626`). |
| `drain_to` | `fn drain_to(&mut self, script_pubkey: ScriptBuf) -> &mut Self` | ✅ | Change-output script override; usable without `add_recipient` (`#736-765`). |
| `ordering` | `fn ordering(&mut self, ordering: TxOrdering) -> &mut Self` | ✅ | Enum `TxOrdering { Shuffle, Untouched }` (`#545-548`). |
| `policy_path` | `fn policy_path(&mut self, policy_path: BTreeMap<String, Vec<usize>>, keychain: KeychainKind) -> &mut Self` | ✅ | Disambiguates thresh() branches in the spending policy (`#280-319`). |
| `add_xpub_key_only` | – | ❌ **Removed** | Replaced by `add_global_xpubs` (no separate "only" variant). |
| `only_xpub_key_only` | – | ❌ **Removed** | Same as above. |
| `add_global_xpubs` | `fn add_global_xpubs(&mut self) -> &mut Self` | ✅ | Offline-multisig helper (BitBox, ColdCard) (`#609-612`). |
| `nlocktime` | `fn nlocktime(&mut self, locktime: LockTime) -> &mut Self` | ✅ | Conflicts possible if descriptor has `after` (`#554-557`). |
| `version` | `fn version(&mut self, version: i32) -> &mut Self` | ✅ | Must be > 0; ≥ 2 if descriptor has `older` (`#564-567`). |
| `sighash` | `fn sighash(&mut self, sighash: PsbtSighashType) -> &mut Self` | ✅ | "**Use this option very carefully**" (`#537-540`). |
| `only_witness_utxo` | `fn only_witness_utxo(&mut self) -> &mut Self` | ✅ | Omits `non_witness_utxo` from PSBT inputs (`#602-605`). |
| `set_exact_sequence` | `fn set_exact_sequence(&mut self, n_sequence: Sequence) -> &mut Self` | ✅ | Conflicts possible with `older` branches (`#648-652`). |
| `allow_dust` | `fn allow_dust(&mut self, allow_dust: bool) -> &mut Self` | ✅ | Bypass dust-limit check (`#664-667`). |
| `current_height` | `fn current_height(&mut self, height: u32) -> &mut Self` | ✅ | Used for anti-fee-sniping and coinbase maturity (`#658-661`). |
| `exclude_unconfirmed` | `fn exclude_unconfirmed(&mut self) -> &mut Self` | ✅ | Shorthand for `exclude_below_confirmations(1)` (`#526-529`). |
| `exclude_below_confirmations` | `fn exclude_below_confirmations(&mut self, min_confirms: u32) -> &mut Self` | ✅ | Final set is **union** of all filters (`#519-521`). |
| `add_data` | `fn add_data<T: AsRef<PushBytes>>(&mut self, data: &[T]) -> &mut Self` | ✅ | `OP_RETURN` output (`#730-733`). |
| `coin_selection` | `fn coin_selection<P: CoinSelectionAlgorithm>(self, coin_selection: P) -> TxBuilder<'a, P>` | ✅ | **Consumes `self`**, not a chain-`&mut self` method (`#632-635`). Call as the last step (or build the `coin_selection`'d builder first). |

### 4.3 `TxBuilder::finish` — the terminal builder method

| Item | Value |
| --- | --- |
| **Signature** | `pub fn finish(self) -> Result<Psbt, CreateTxError>` (only available on `impl<Cs: CoinSelectionAlgorithm> TxBuilder<'_, Cs>`) |
| **Receiver** | `self` (consumes) |
| **Returns** | `Result<Psbt, CreateTxError>` — `Psbt` is `bitcoin::psbt::Psbt` (v1). |
| **Notes** | "Uses the thread-local random number generator (rng)." For tests/repro, use `finish_with_aux_rand(&mut rng)`. **WARNING** (verbatim from doc): "To avoid change address reuse you must persist the changes resulting from one or more calls to this method before closing the wallet. See `Wallet::reveal_next_address`." |
| **Gating** | `Available on crate feature 'std' only.` |

`finish_with_aux_rand(self, rng: &mut impl RngCore) -> Result<Psbt, CreateTxError>` is the same signature with explicit RNG.

### 4.4 `CreateTxError` — variants returned from `TxBuilder::finish`

Source: `https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.CreateTxError.html` (rustdoc source file `crates/wallet/src/wallet/error.rs.html#L161-218`). The full enum carries **18 variants**:

| Variant | Shape | What it means |
| --- | --- | --- |
| `Descriptor(DescriptorError)` | tuple | Descriptors passed in are problematic |
| `Policy(PolicyError)` | tuple | Extracting/manipulating spending policies failed |
| `SpendingPolicyRequired(KeychainKind)` | tuple | Policy is not compatible with this keychain |
| `Version0` | unit | Requested transaction version `0` |
| `Version1Csv` | unit | Requested v1, but ≥ v2 needed to use OP_CSV |
| `LockTime { requested: LockTime, required: LockTime }` | struct-like | `nlocktime` is too low for script requirements |
| `RbfSequenceCsv { sequence: Sequence, csv: Sequence }` | struct-like | Cannot enable RBF with the given sequence |
| `FeeTooLow { required: Amount }` | struct-like | Bump-a-tx: requested absolute fee lower than replaced tx |
| `FeeRateTooLow { required: FeeRate }` | struct-like | Bump-a-tx: requested feerate lower than required |
| `NoUtxosSelected` | unit | `manually_selected_only` set but no UTXO added |
| `OutputBelowDustLimit(usize)` | tuple | Output under 546 sats (carries output index) |
| **`CoinSelection(InsufficientFunds)`** | tuple | **Coin selection failed** — `InsufficientFunds` is the struct in §4.5 |
| `NoRecipients` | unit | `add_recipient`/`set_recipients` empty |
| `Psbt(bitcoin::psbt::error::Error)` | tuple | PSBT error from `rust-bitcoin` |
| `MissingKeyOrigin(String)` | tuple | `add_global_xpubs` requires non-master key to have origin |
| `UnknownUtxo` | unit | Trying to spend a UTXO not in the internal database |
| `MissingNonWitnessUtxo(OutPoint)` | tuple | Foreign UTXO is missing `non_witness_utxo` for `OutPoint` |
| `MiniscriptPsbt(MiniscriptPsbtError)` | tuple | Miniscript PSBT failure |

`From` impls: `From<DescriptorError>`, `From<bitcoin::psbt::error::Error>`, **`From<InsufficientFunds>` (wraps into `CreateTxError::CoinSelection`)**, `From<MiniscriptPsbtError>`, `From<PolicyError>` — all auto-derived.

### 4.5 `InsufficientFunds` (in `bdk_wallet::coin_selection`)

| Item | Value |
| --- | --- |
| **Signature** | `pub struct InsufficientFunds { pub needed: Amount, pub available: Amount }` |
| **Source** | `crates/wallet/src/wallet/coin_selection.rs.html#L127-132` (per docs.rs) |
| **Implements** | `Clone, Debug, Display, Error, PartialEq, Eq, StructuralPartialEq` |
| **How it surfaces** | Wrapped as `CreateTxError::CoinSelection(InsufficientFunds { needed, available })` |
| **Doc string** | "Wallet's UTXO set is not enough to cover recipient's requested plus fee. This is thrown by `CoinSelectionAlgorithm`." |

### 4.6 Fee bumping — `Wallet::build_fee_bump`

| Item | Value |
| --- | --- |
| **Signature** | `pub fn build_fee_bump(&mut self, txid: Txid) -> Result<TxBuilder<'_, DefaultCoinSelectionAlgorithm>, BuildFeeBumpError>` |
| **Returns** | Pre-populated `TxBuilder` whose inputs/outputs are the original tx's; you set the new fee via `fee_rate`/`fee_absolute` then call `finish`. |
| **Errors** | `BuildFeeBumpError { TransactionNotFound(Txid), TransactionConfirmed, RbfDisabled }` (verify exact variant set in spike — `BuildFeeBumpError` page not opened). |
| **Caveat** | Original tx must signal RBF (nSequence < 0xfffffffe). |
| **Docs** | `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html#method.build_fee_bump` |

---

## Category 5: Signing

### 5.1 Summary table — Wallet sign / PSBT-finish methods

| Method | Signature (Rust) | Status | Notes |
| --- | --- | --- | --- |
| `Wallet::sign` | `pub fn sign(&self, psbt: &mut Psbt, sign_options: SignOptions) -> Result<bool, SignerError>` | ✅ | "Sign a transaction with all the wallet's signers, in the order specified by every signer's `SignerOrdering`." Returns `true` if PSBT was finalized, `false` otherwise. Source: `crates/wallet/src/wallet/mod.rs.html#L1752-1758`. |
| `Wallet::sign_with` | – | ❌ **Not in 3.1** | The 0.x name; 3.x uses `sign_with_signers`. **Verify in spike** for any wrapper alias. |
| **`Wallet::sign_with_signers`** | `pub fn sign_with_signers(&self, psbt: &mut Psbt, signers: &[&SignersContainer], sign_options: SignOptions) -> Result<bool, SignerError>` | ✅ | "Sign a transaction with the provided signer containers. Signer containers are processed in the order provided. Signers inside each container are processed according to their `SignerOrdering`." Source: `crates/wallet/src/wallet/mod.rs.html#L1807-1854`. |
| `Wallet::finalize_psbt` | `pub fn finalize_psbt(&self, psbt: &mut Psbt, sign_options: SignOptions) -> Result<bool, SignerError>` | ✅ | "For each input determine if sufficient data is available to pass validation and construct the respective `scriptSig` or `scriptWitness`. Please refer to BIP174 and BIP371 for further information." Source: `crates/wallet/src/wallet/mod.rs.html#L2001-2003`. |
| `Wallet::add_signer` | `pub fn add_signer(&mut self, keychain: KeychainKind, ordering: SignerOrdering, signer: Arc<dyn TransactionSigner>)` | ✅ | "Add an external signer." Source: `crates/wallet/src/wallet/mod.rs.html#L1148-1156`. |
| `Wallet::add_external_signer` | – | ❌ **Not in 3.1** | 0.x name; 3.x uses `add_signer`. **Verify in spike**. |
| `Wallet::set_keymap` | `pub fn set_keymap(&mut self, keychain: KeychainKind, keymap: KeyMap)` | ✅ | Lower-level API than `add_signer`. `KeyMap` is `bdk_wallet::keys::KeyMap`. Source: `#1159-1163`. |
| `Wallet::set_keymaps` | `pub fn set_keymaps(&mut self, keymaps: impl IntoIterator<Item = (KeychainKind, KeyMap)>)` | ✅ | Bulk setter. Source: `#1184-1189`. |
| `Wallet::get_signers` | `pub fn get_signers(&self, keychain: KeychainKind) -> Arc<SignersContainer>` | ✅ | Read-only access to the per-keychain signer container. Source: `#1192-...`. |
| `Wallet::get_psbt_input` | `pub fn get_psbt_input(&self, utxo: LocalOutput, sighash_type: Option<PsbtSighashType>, only_witness_utxo: bool) -> Result<Input, CreateTxError>` | ✅ | "Get the corresponding PSBT Input for a `LocalOutput`." Used by external-signer flows. Source: `crates/wallet/src/wallet/mod.rs.html#L2251-2258`. |
| `Wallet::mark_psbt_as_signed` | – | ❌ **Not in 3.1** | Not found on the 3.1 Wallet page (searched `method.mark_psbt_as_signed`). There is **no public mark-as-signed helper** in BDK 3.1; reviewers detect finalization via the boolean returned by `sign`/`finalize_psbt`. **Verify in spike**. |

> Important: **No `KeychainKey` enum exists in BDK 3.1.** The pre-3.0 BDK API had `enum KeychainKey { External, Internal }` inside a signer abstraction. In 3.x, the equivalent is `enum KeychainKind { External, Internal }` (used by `add_signer`, `set_keymap`, `get_signers`). There is **no `KeychainKey::Secp256k1` variant** — `KeychainKey` (with a Y) doesn't exist. Cite: `Wallet::add_signer(&mut self, keychain: KeychainKind, ...)` per the doc.

### 5.2 Summary table — Signer trait hierarchy

`https://docs.rs/bdk_wallet/latest/bdk_wallet/signer/index.html` (source `crates/wallet/src/wallet/signer.rs.html#L12-1170`):

| Trait | Where | What you implement | Used by |
| --- | --- | --- | --- |
| **`TransactionSigner: SignerCommon`** | `bdk_wallet::signer` | `fn sign_transaction(&self, psbt: &mut Psbt, sign_options: &SignOptions, secp: &Secp256k1<All>) -> Result<(), SignerError>;` | **This is what `Wallet::add_signer` expects.** (`signer.rs.html#L284-292`). |
| `SignerCommon` | `bdk_wallet::signer` | `fn id(&self, _secp: &Secp256k1<All>) -> SignerId;` | Supertrait of `TransactionSigner`. |
| `InputSigner` | `bdk_wallet::signer` | `fn sign_input(&self, psbt: &mut Psbt, input_index: usize, sign_options: &SignOptions, secp: &Secp256k1<All>) -> Result<(), SignerError>;` | Per-input signer. There is **a blanket impl** `impl<T: InputSigner> TransactionSigner for T` (signer.rs#L294-307), so any per-input signer is *automatically* a `TransactionSigner`. |
| `Signer` | – | – | **❌ Not in 3.1.** A trait just called `Signer` does not exist in `bdk_wallet::signer` (no `trait Signer` page linked from the module index). **Verify in spike** if you expect it in `bdk_wallet::keys` or some other module. |
| `SignersContainer` | `bdk_wallet::signer` | `signer::struct.SignersContainer` — a hold-many wrapper; has `build(keymap, desc, secp)` constructor and `signers()` iterator | Built from a `KeyMap` + descriptor + secp context. |

#### 5.2.1 Full `TransactionSigner` definition

```rust
pub trait TransactionSigner: SignerCommon {
    fn sign_transaction(
        &self,
        psbt: &mut Psbt,
        sign_options: &SignOptions,
        secp: &Secp256k1<All>,
    ) -> Result<(), SignerError>;
}
```

(Source: `bdk_wallet/signer/trait.TransactionSigner.html` → `crates/wallet/src/wallet/signer.rs.html#L284-292`.)

#### 5.2.2 Full `InputSigner` definition (inferred from doc snippet)

```rust
pub trait InputSigner: SignerCommon {
    fn sign_input(
        &self,
        psbt: &mut Psbt,
        input_index: usize,
        sign_options: &SignOptions,
        secp: &Secp256k1<All>,
    ) -> Result<(), SignerError>;
}
```

The BDK signer-module page shows the expected custom-signer implementation shape:

```rust
impl SignerCommon for CustomSigner {
    fn id(&self, _secp: &Secp256k1<All>) -> SignerId { self.device.get_id() }
}
impl InputSigner for CustomSigner {
    fn sign_input(
        &self,
        psbt: &mut Psbt,
        input_index: usize,
        _sign_options: &SignOptions,
        _secp: &Secp256k1<All>,
    ) -> Result<(), SignerError> {
        self.device.hsm_sign_input(psbt, input_index)?;
        Ok(())
    }
}
let custom_signer = CustomSigner::connect();
wallet.add_signer(
    KeychainKind::External,
    SignerOrdering(200),
    Arc::new(custom_signer),
);
```

(Source: `https://docs.rs/bdk_wallet/latest/bdk_wallet/signer/index.html`.)

### 5.3 `SignOptions` fields

Struct definition (verbatim from docs.rs):

```rust
pub struct SignOptions {
    pub trust_witness_utxo: bool,
    pub assume_height: Option<u32>,
    pub allow_all_sighashes: bool,
    pub try_finalize: bool,
    pub tap_leaves_options: TapLeavesOptions,
    pub sign_with_tap_internal_key: bool,
    pub allow_grinding: bool,
}
```

(`https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.SignOptions.html`, source `crates/wallet/src/wallet/signer.rs.html#L785-835`.)

| Field | Type | Default | Semantics (verbatim or close-paraphrased from docs) |
| --- | --- | --- | --- |
| `trust_witness_utxo` | `bool` | `false` (defensive against the "SegWit bug") | Trust `witness_utxo` if `non_witness_utxo` is missing. Some legacy wallets don't provide `non_witness_utxo`; setting `true` is required to sign in those cases. |
| `assume_height` | `Option<u32>` | `None` | Override the "current height" used to evaluate timelocks. Lets you sign with timelocks in the future. |
| `allow_all_sighashes` | `bool` | `false` | If `true`, accept any `sighash_type` set in the PSBT; else restrict to `SIGHASH_ALL`. |
| `try_finalize` | `bool` | `true` | After signing, attempt to finalize the PSBT. |
| `tap_leaves_options` | `TapLeavesOptions` | `TapLeavesOptions::All` | Which taproot script-spend leaves we sign for (ignored for non-taproot PSBTs). |
| `sign_with_tap_internal_key` | `bool` | `true` | Whether to also sign with the taproot internal key (ignored for non-taproot). |
| `allow_grinding` | `bool` | `true` | Whether to grind ECDSA signatures for low-r (BIP-146 compliance). |

Other support types (in `bdk_wallet::signer`):

- `enum TapLeavesOptions { All, Specific(BTreeMap<Script, LeafInfo>) }` — exact variants **verify in spike** (not opened in this session).
- `enum SignerId { PkSingle(PublicKey), Fingerprint(Fingerprint) }` (verify in spike).
- `struct SignerOrdering(pub u32)` — control call order across signers.
- `enum SignerContext { Unknown, TapKeySpend, TapScriptSpend { leaf_hash: TapLeafHash } }` (verify in spike).
- `enum SignerError { ... }` — the error type returned by `sign`/`sign_with_signers`/`finalize_psbt`. Variants **verify in spike** (not opened).

### 5.4 `Wallet::secp_ctx`

| Item | Value |
| --- | --- |
| **Signature** | `pub fn secp_ctx(&self) -> &Secp256k1<All>` |
| **Returns** | Shared reference to the secp256k1 context used for all signing. From `secp256k1 = 0.29.1`. |
| **Use** | Pass to custom signers; required by `InputSigner::sign_input(&Secp256k1<All>)` and `SignerCommon::id(&Secp256k1<All>)`. |

---

## Category 6: PSBT

### 6.1 What BDK provides on top of `rust-bitcoin::Psbt`

The `bdk_wallet::psbt` module (source `crates/wallet/src/psbt/mod.rs.html`, *NB the GitHub path used in the prompt returns 404 — module sits under `crates/wallet/src/wallet/psbt/mod.rs`; **verify in spike**) consists of **one trait, `PsbtUtils`**, plus its impl for `bitcoin::psbt::Psbt`:

```rust
pub trait PsbtUtils {
    fn get_utxo_for(&self, input_index: usize) -> Option<TxOut>;
    fn fee_amount(&self) -> Option<Amount>;
    fn fee_rate(&self) -> Option<FeeRate>;
}
```

| Method | Signature | Purpose (from docs) |
| --- | --- | --- |
| `get_utxo_for` | `fn get_utxo_for(&self, input_index: usize) -> Option<TxOut>` | "Get the `TxOut` for the specified input index, if it doesn't exist in the PSBT `None` is returned." |
| `fee_amount` | `fn fee_amount(&self) -> Option<Amount>` | "The total transaction fee amount, sum of input amounts minus sum of output amounts, in sats. If the PSBT is missing a TxOut for an input returns None." |
| `fee_rate` | `fn fee_rate(&self) -> Option<FeeRate>` | "The transaction's fee rate. This value will only be accurate if calculated AFTER the `Psbt` is finalized and all witness/signature data is added to the transaction. If the PSBT is missing a TxOut for an input returns None." |

The trait is auto-impl'd for `bitcoin::psbt::Psbt` (verified on the page). **That's it — BDK 3.1 adds no other helpers to PSBTs.** Everything else (signing, extracting) goes through methods on `Wallet`.

### 6.2 `psbt.extract_tx` (post-signing)

This is **not** a BDK helper — it's the method on `rust-bitcoin`'s `Psbt`:

- `impl Psbt { pub fn extract_tx(&self) -> Result<Transaction, Error>; }`
- Returns `Err` if not all inputs are finalized.
- BDK examples use `psbt.extract_tx()` immediately after a successful `wallet.sign(&mut psbt, SignOptions::default())?` returning `true`.

The BDK pages document this in two places:

- `Wallet::sign` doc example: `let tx = psbt.clone().extract_tx().expect("tx");`
- The `wallet` module (Build→Sign→Broadcast flow).

There is **no `bdk_wallet::psbt::extract_tx` wrapper**.

### 6.3 Summary table — PSBT-related public surface

| Method | Signature | Source | Status in 3.1 |
| --- | --- | --- | --- |
| `Wallet::sign` | `fn sign(&self, psbt: &mut Psbt, sign_options: SignOptions) -> Result<bool, SignerError>` | `mod.rs.html#L1752-1758` | ✅ |
| `Wallet::sign_with_signers` | `fn sign_with_signers(&self, psbt: &mut Psbt, signers: &[&SignersContainer], sign_options: SignOptions) -> Result<bool, SignerError>` | `mod.rs.html#L1807-1854` | ✅ |
| `Wallet::finalize_psbt` | `fn finalize_psbt(&self, psbt: &mut Psbt, sign_options: SignOptions) -> Result<bool, SignerError>` | `mod.rs.html#L2001-2003` | ✅ |
| `Wallet::get_psbt_input` | `fn get_psbt_input(&self, utxo: LocalOutput, sighash_type: Option<PsbtSighashType>, only_witness_utxo: bool) -> Result<Input, CreateTxError>` | `mod.rs.html#L2251-2258` | ✅ |
| `Wallet::mark_psbt_as_signed` | – | – | ❌ Not in 3.1. |
| `TxBuilder::add_foreign_utxo` | `fn add_foreign_utxo(&mut self, outpoint: OutPoint, psbt_input: Input, satisfaction_weight: Weight) -> Result<&mut Self, AddForeignUtxoError>` | `tx_builder.rs.html#L403-453` | ✅, **EXPERIMENTAL** |
| `TxBuilder::set_foreign_utxo` | – | – | ❌ Not in 3.1 (use `add_foreign_utxo`). |
| `psbt.extract_tx()` (rust-bitcoin) | – | rust-bitcoin 0.32 | ✅ (not BDK). |
| `bdk_wallet::bitcoin::Psbt` (re-export) | – | `bdk_wallet::bitcoin` is a **re-export of `bitcoin`** | ✅ (all rust-bitcoin types available under `bdk_wallet::bitcoin::...`). **Verify the exact `pub use` shape in spike** — typically `pub use bitcoin;` so it's `bdk_wallet::bitcoin::psbt::Psbt`. |

### 6.4 PSBT v2 support

- **rust-bitcoin 0.32** (transitively used by BDK 3.1.0 via `bitcoin = ^0.32.8`) added PSBT v2 types (`PartiallySignedTransaction`).
- **BDK 3.1 itself does not add explicit PSBT-v2 APIs in the API surface I scanned.** `TxBuilder::finish` returns `bitcoin::psbt::Psbt` (the **v1** `Psbt` type, per `bitcoin/psbt/struct.Psbt.html` cited in the docs).
- The BDK `psbt` module exports only `PsbtUtils` (no `PsbtV2`).
- There is **no `bdk_wallet::v2` module, no `PsbtV2` re-export, no `Wallet::build_tx_v2`.**
- **Practical consequence:** A consumer that needs PSBT v2 today must either (a) hand-craft one from `bitcoin::psbt::Psbt` v2 types and bypass BDK's helpers, or (b) wait for an upstream PR. **Verify in spike** — at minimum check that none of the changelogs of 3.x patch releases (3.1.x) added PSBT v2 helpers.

### 6.5 Re-exports

- `bdk_wallet::bitcoin` resolves to `bitcoin`'s crate root via `pub use bitcoin;` (or near-equivalent), so `bdk_wallet::bitcoin::psbt::Psbt`, `bdk_wallet::bitcoin::Transaction`, `bdk_wallet::bitcoin::secp256k1::Secp256k1` all work. **Verify the exact `pub use` in spike** by reading `crates/wallet/src/lib.rs`.

---

## What's NOT in BDK 3.1 (for these three categories)

Sourced from the docs.rs Wallet/TxBuilder/signer/psbt module indexes of version 3.1.0. **For each, I document how to substitute the 3.1 equivalent.**

| Missing API (mentioned in the prompt or in older BDK) | BDK 3.1 substitution |
| --- | --- |
| `Wallet::sign_with(&self, psbt: &mut Psbt, ...)` | Use `Wallet::sign_with_signers(&self, psbt, &[&signers_container], SignOptions::default())` — the "with signers" variant is the way to pass specific signers in 3.x. |
| `Wallet::add_external_signer(keychain, signer)` | Use `Wallet::add_signer(keychain, SignerOrdering::default(), Arc::new(signer) as Arc<dyn TransactionSigner>)`. |
| `Wallet::mark_psbt_as_signed` | **Not present.** Detect finalization via the `bool` returned from `sign` / `finalize_psbt`. There is also no `extract_tx`-with-force flag in BDK; `bitcoin::psbt::Psbt::extract_tx` succeeds iff all inputs are finalized. |
| `TxBuilder::set_foreign_utxo` | Use `TxBuilder::add_foreign_utxo(outpoint, psbt_input, satisfaction_weight)` (returns `Result<&mut Self, AddForeignUtxoError>`). |
| `TxBuilder::add_xpub_key_only` | Not present. 3.x replaces both `add_xpub_key_only` and `only_xpub_key_only` with `add_global_xpubs()`, which is binary (on/off). |
| `TxBuilder::only_xpub_key_only` | Same — replaced by `add_global_xpubs()` (no opt-out). |
| BDK's `Signer` trait (single, by name) | The 3.x trait is `TransactionSigner` (`sign_transaction(&mut Psbt, &SignOptions, &Secp256k1<All>)`), with supertrait `SignerCommon` and an alternative per-input trait `InputSigner` (`sign_input(&mut Psbt, usize, &SignOptions, &Secp256k1<All>)`). There is a blanket `impl<T: InputSigner> TransactionSigner for T`. |
| `KeychainKey` enum (`KeychainKey::Secp256k1`) | **No such enum in 3.x.** The relevant type is `bdk_wallet::KeychainKind { External, Internal }` (a 2-variant enum, no payload). Custom keys are passed via `bdk_wallet::keys::KeyMap` (an alias-like type holding `Hashed<DescriptorSecretKey>`) and external signers via `Arc<dyn TransactionSigner>`. The `Secp256k1` context used for all signing is `wallet.secp_ctx()`. |
| `bdk_wallet::psbt::extract_tx` | Use `bitcoin::psbt::Psbt::extract_tx` (the `rust-bitcoin` method). |
| PSBT v2 typed helpers | **None in 3.1.** The `Psbt` re-exported is the v1 `bitcoin::psbt::Psbt` type; `TxBuilder::finish` produces a v1 PSBT. |
| `bdk_wallet::psbt::calculate_fee` / `calculate_fee_rate` | Replaced by **methods on `Wallet`**: `wallet.calculate_fee(&tx) -> Result<Amount, CalculateFeeError>` (`mod.rs.html#L939-941`) and `wallet.calculate_fee_rate(&tx) -> Result<FeeRate, CalculateFeeError>` (`#970-972`), both taking a finalized `Transaction`. For per-PSBT fee/fee-rate, use the `PsbtUtils` trait's `fee_amount` / `fee_rate` methods. |
| `Wallet::build_tx_with(coin_selection)` | Use `wallet.build_tx().coin_selection(YourAlgo)` (note: `coin_selection` consumes `self`, so call as the first step). |

---

## Source of truth links

- Wallet page: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.Wallet.html`
- TxBuilder page: `https://docs.rs/bdk_wallet/latest/bdk_wallet/tx_builder/struct.TxBuilder.html`
- TxBuilder module index: `https://docs.rs/bdk_wallet/latest/bdk_wallet/tx_builder/index.html`
- PSBT module index: `https://docs.rs/bdk_wallet/latest/bdk_wallet/psbt/index.html`
- `PsbtUtils` trait: `https://docs.rs/bdk_wallet/latest/bdk_wallet/psbt/trait.PsbtUtils.html`
- Signer module: `https://docs.rs/bdk_wallet/latest/bdk_wallet/signer/index.html`
- `TransactionSigner` trait: `https://docs.rs/bdk_wallet/latest/bdk_wallet/signer/trait.TransactionSigner.html`
- `SignOptions` struct: `https://docs.rs/bdk_wallet/latest/bdk_wallet/struct.SignOptions.html`
- `CreateTxError` enum: `https://docs.rs/bdk_wallet/latest/bdk_wallet/error/enum.CreateTxError.html`
- `InsufficientFunds` struct: `https://docs.rs/bdk_wallet/latest/bdk_wallet/coin_selection/struct.InsufficientFunds.html`

---

## What I did **not** definitively pin down (defer to verification spike)

1. Exact variant set of `enum ChangeSpendPolicy` — I cited `enum ChangeSpendPolicy { ChangeForbidden, OnlyChange, ChangeAllowed }` from memory of BDK 1.x but did not scrape its docs.rs page in this session. ⚠ Verify.
2. Exact variant set of `enum SignerId`, `enum SignerContext`, `enum SignerError`, `enum TapLeavesOptions`. ⚠ Verify.
3. Exact `BuildFeeBumpError` variants — I cited `TransactionNotFound / TransactionConfirmed / RbfDisabled` from memory. ⚠ Verify.
4. Exact GitHub path of `psbt/mod.rs` — the prompt URL `/crates/wallet/src/psbt/mod.rs` and `/crates/wallet/src/wallet/tx_builder.rs` both **404 against the live repo**. docs.rs resolves them to `src/bdk_wallet/psbt/mod.rs.html` and `src/bdk_wallet/wallet/tx_builder.rs.html`, which suggests the actual current source path is `crates/wallet/src/wallet/psbt/mod.rs` and `crates/wallet/src/wallet/tx_builder.rs` under a refactored layout (top-level `bdk_wallet` rather than `wallet`). ⚠ Verify.
5. PSBT v2 status — no explicit PSBT-v2 helpers surfaced in the docs.rs surface I scanned; `TxBuilder::finish` returns `bitcoin::psbt::Psbt` (v1). A spike should grep the source for `PsbtV2`, `PartiallySignedTransaction`, `psbt_v2`, etc., to confirm.
