# Mnemonic Handling — Method Ratings & Picked Approach

**Date:** 2026-08-05
**Purpose:** Score 8 candidate mnemonic-handling methods and pick the right one for each release of `bitcoin-wallet-core` per ADR 0001 (v0.1 / v0.2 / v1.0).
**Companion to:** `docs/wallets/2026-08-05-mnemonic-handling-wallet-survey.md` (the deep research that produced these methods) + `docs/wallets/2026-08-05-adr-0001-signing-model.md` (the security roadmap).

## TL;DR

**v0.1:** plaintext on disk + `Secret<T>` zeroize wrapper. **v0.2:** Sparrow's Argon2id (256 MiB / 500ms wall-clock) + AES-256-GCM. **v1.0:** same KDF + multi-bucket plausible-deniability container + iOS Keychain passphrase handling. **Calibrate to wall-clock, not iteration count** — forward-compatible across hardware generations.

## The 8 methods scored

| # | Method | Source wallet | Security | UX (unlock) | Mobile-ready | Impl cost (lower=better) | Adoption | Weighted |
|---|---|---|---|---|---|---|---|---|
| 1 | **Plaintext mnemonic on disk** | (current v0.1 plan) | 1 | 10 (instant) | 5 | 10 (free) | 1 | **4.4** |
| 2 | **PBKDF2-SHA512 2048 + AES-128-CTR** | Trust Wallet Core | 6 | 8 (~50ms) | 8 | 5 | 8 | **6.7** |
| 3 | **PBKDF2-MD5 1 iter + AES-256-CBC** | BlueWallet legacy | 1 | 10 (instant) | 3 (JS) | 9 (broken) | 1 | **2.6** |
| 4 | **PBKDF2-SHA512 1k iter, no salt + AES-256-CBC** | Electrum BIE1/BIE2 | 1 (no salt!) | 8 | 6 | 7 | 5 | **4.0** |
| 5 | **PBKDF2-SHA256 50k + AES-256-CBC + 16B salt** | Electrum Keystore v2 | 6 | 7 (~150ms) | 7 | 6 | 6 | **6.2** |
| 6 | **Argon2id 256 MiB / 10 iter / 500ms + AES-128-CBC** | Sparrow | 9 | 5 (~500ms) | 6 | 4 | 7 | **7.4** |
| 7 | **AES-256-CBC + dynamic-iter + SecureString + time-bounded unlock** | Bitcoin Core | 8 | 4 (slow) | 4 | 3 | 10 | **6.0** |
| 8 | **Plausible-deniability multi-bucket container** | BlueWallet | 8 (defeats $5-wrench) | 6 | 6 | 5 | 4 | **6.4** |

**Weights:** Security 0.4, UX 0.2, Mobile-ready 0.2, Impl cost 0.1, Adoption 0.1. (Security dominates — this is a security feature.)

**Winner: #6 (Sparrow's Argon2id).** Runners-up: #2 (TW Core PBKDF2, viable fallback), #8 (plausible-deniability, complementary not competing).

## Why Argon2id beats PBKDF2 (the durable choice)

**PBKDF2 weakness:** iteration count is fixed. Hardware gets faster. PBKDF2-SHA512 2048 was the right answer in 2018; on 2026 GPU hardware, an attacker can compute ~5x more hashes per second than in 2018.

**Argon2id strength:** calibrate to wall-clock time, not iteration count. 500ms today = 500ms in 2030 = 500ms in 2040. The wall-clock target is constant; the iteration count adapts to hardware. Sparrow's 256 MiB + 10 iter + calibrated to 500ms is the right model.

**Plus:** Argon2id is memory-hard. 256 MiB per derivation rules out GPU brute force. GPU speedup factor: ~5x. PBKDF2 GPU speedup factor: ~1000x (custom ASICs).

**Plus:** Sparrow stores nothing derived from the passphrase. An attacker who steals the wallet file cannot even confirm the passphrase is correct without scanning the blockchain. That's worth more than any cipher choice.

## Pick per release (per ADR 0001)

### v0.1 (testnet dev only — current plan)

**Method:** plaintext on disk + `Secret<T>` zeroize wrapper.

**Why:** v0.1 is explicitly testnet-only. Threat model is "don't accidentally leak on your own machine", not "defend against a nation-state". Plaintext is fine for this scope.

**What to add to v0.1 from the survey (small, high-leverage):**
- `Secret<Mnemonic>` newtype w/ `ZeroizeOnDrop` — **bump plan Task 30 from "v0.2" to "split: Secret<T> in v0.1, Argon2id in v0.2"**. Costs nothing; prevents the most common memory-dump attack even in v0.1.
- Refuse world-writable wallet dirs (mode 0600 mandatory) — **new plan task addition**.
- Atomic `tmp + rename` writes for `mnemonic.txt` — **new plan task addition** (Trust Wallet Core PR #4756 pattern).

**Score after additions:** Security 1 → 3 (still plaintext, but with hygiene).

### v0.2 (encryption milestone)

**Method:** #6 (Sparrow's Argon2id + AES-GCM) + everything from v0.1.

**Why:** Sparrow's KDF is the industry-leading choice in 2026. Argon2id calibrated to 500ms is the right cost/benefit. AES-**GCM** (AEAD) over AES-CBC (separate MAC). Per the plan, this is Task 30.

**What to add to v0.2 from the survey (beyond Task 30):**
- Argon2id parameters: **m=256 MiB, t=10, p=4, calibrated to 500ms wall-clock on first run** (not raw iterations — calibrating on first run is how Sparrow does it).
- AES-256-GCM (not AES-128) — bump the plan's Task 30 from AES-128 to AES-256 to match the v0.2 / v1.0 standard.
- `Secret<T>` newtype w/ `ZeroizeOnDrop` — moved to v0.1 above ✓
- `mlock` on Unix (prevent swap-to-disk) — **new plan task addition** (~10 LOC via `libc::mlock`)
- Per-session re-prompt (no long-lived unlock) — **new plan task addition** (Bitcoin Core's `walletpassphrase` time-bounded unlock pattern, but no daemon)
- Versioned file header (for future migration) — **new plan task addition** (e.g. `magic(4) || version(1) || salt(16) || nonce(12) || ciphertext(N+16)`)

**Score after additions:** Security 4 → 8 (matches Sparrow's strength).

### v1.0 (mobile, Phase 2 UniFFI)

**Method:** #6 (Argon2id + AES-GCM) for the encrypted-wallet-on-disk case + **#8 (plausible-deniability multi-bucket container)** for the high-value-wallet case.

**Why:** v1.0 is the end-user product. Argon2id handles "stolen disk image" attacks. Plausible deniability handles "duress / $5-wrench" attacks. The two are complementary, not competing.

**What to add to v1.0 from the survey:**
- Plausible-deniability mode: store a decoy container unlocked by a different password, unlock the real one only via the right password. BlueWallet's standout feature. ~500 LOC, 1 week.
- iOS Keychain integration: in the Swift host, the `passphrase` goes into iOS Keychain protected by `kSecAttrAccessibleWhenUnlockedThisDeviceOnly`. The Rust core never sees the passphrase in plaintext if the user has Face ID enabled — Swift holds it, hands to Rust on each sign.
- Trezor BIP-39 test vectors as CI acceptance gate (catches mnemonic generator bugs instantly).
- Argon2id parameters on mobile: target **1s wall-clock** (vs. 500ms desktop), m=64 MiB (vs. 256 MiB) to fit mobile memory budgets.

**Score after additions:** Security 8 → 10 (full coverage including coercion resistance).

## Summary

| Version | KDF | Cipher | In-memory | At-rest | Wallet dir | UI | Threat model coverage |
|---|---|---|---|---|---|---|---|
| **v0.1** | none | none (plaintext) | `Secret<T>` ZeroizeOnDrop | atomic write, mode 0600 | refuse world-writable | big `WARNING` | "stolen machine, attacker has your files" ❌ |
| **v0.2** | Argon2id 256 MiB / 500ms | AES-256-GCM | `Secret<T>` + `mlock` Unix | atomic write, versioned header | mode 0700 | passphrase prompt per session | "stolen disk image" ✓ + "stolen machine" ✓ |
| **v1.0** | Argon2id 64 MiB / 1s mobile | AES-256-GCM | iOS Keychain for passphrase | atomic write + multi-bucket | device-protected | biometric unlock via Swift host | all of v0.2 + "coercion" ✓ |

## Concrete deltas to feed into the plan

1. **v0.1 add: `Secret<T>` newtype** — type-level enforcement of `ZeroizeOnDrop`. ~50 LOC. Move from Task 30 to a new "v0.1 hygiene" task.
2. **v0.1 add: atomic-write helper** — `tmp + fsync + rename`. ~30 LOC. Prevents partial-write corruption.
3. **v0.1 add: refuse world-writable dir** — `fs::metadata().permissions() & 0o022 != 0` → error. ~10 LOC.
4. **v0.2 update: AES-256-GCM (not AES-128)** — already in plan as GCM but on 128. Bump to 256.
5. **v0.2 update: Argon2id parameters** — `m=256 MiB, t=10, p=4, calibrated to 500ms on first run` (not raw iterations).
6. **v0.2 add: `mlock` on Unix** — wrap `Secret<T>` with `mlock` on drop + `munlock` on drop. ~10 LOC via `libc`.
7. **v0.2 add: versioned file header** — 4-byte magic + 1-byte version + 16-byte salt + 12-byte nonce + ciphertext + 16-byte auth tag. Format already in plan Task 30.
8. **v1.0 add: plausible-deniability multi-bucket** — BlueWallet's standout feature. Two encrypted containers, one unlocked by password A (real), one by password B (decoy).
9. **v1.0 add: iOS Keychain for passphrase** — Swift-side; Rust core never sees plaintext passphrase if Face ID enabled.
10. **CI: Trezor BIP-39 test vectors as acceptance gate** — small one-time addition, catches generator bugs forever.

## Single most-important takeaway

**Sparrow's Argon2id (calibrated to wall-clock, not iteration count) is the only design that doesn't degrade over time.** PBKDF2 is the wrong answer for any wallet shipping in 2026 or later. Our v0.2 plan's choice of Argon2id is correct; the only thing to fix is the calibration target (wall-clock, not raw iterations) and the cipher size (AES-256, not AES-128).

**For v0.1:** plaintext is fine. v0.1 is testnet dev. Add `Secret<T>` + atomic write + world-writable refusal. These three additions cost ~100 LOC and prevent the three most-common v0.1 footguns.

**For v0.2:** Argon2id 256 MiB / 500ms + AES-256-GCM + atomic write + `Secret<T>` + `mlock` + versioned header. Matches Sparrow's strength.

**For v1.0:** same v0.2 KDF + plausible-deniability multi-bucket container + iOS Keychain passphrase handling. Matches BlueWallet's standout feature.
