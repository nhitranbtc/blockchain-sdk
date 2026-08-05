# BIP-39 Mnemonic Handling Survey — 5 Wallets

**Date:** 2026-08-05
**Purpose:** Inform the design of `bitcoin-wallet-core` (Rust Bitcoin wallet CLI, ADR 0001)
**Scope:** 5 wallets — Trust Wallet Core (Rust), BlueWallet (iOS RN), Electrum (Python+Qt),
Sparrow (Java desktop), Bitcoin Core (C++ reference).

> Note on prior art for in-memory safety: I also referenced the `zeroize` crate
> (`docs.rs/zeroize`) and the `seedlock` crate (mlock + zeroize + constant-time `PartialEq`)
> as Rust patterns to model v1.0 behavior on.

---

## 1. Trust Wallet Core (Rust + C++ core, multi-platform)

**Repo:** https://github.com/trustwallet/wallet-core · ⭐ ~3.0k+ · actively maintained
**Languages:** C++ core, Rust FFI bindings, Swift/Kotlin/Java wrappers. PR #4384
(merged 2025-04-30, "+687 -136 in 19 files") migrated BIP-39 mnemonic handling
**from C to Rust** (`trezor-crypto/crypto/bip39.c` → `rust/tw_mnemonic`). The current
main branch uses the Rust implementation.

### 1.1 Mnemonic generation

- **Library:** internal Rust module (`rust/tw_mnemonic`) + trezor-crypto-derived C
  fallback still present in tags. Rust path is the production path post-PR #4384.
- **Entropy source:** OS secure RNG via `getrandom()` (Rust `rand`).
- **Word count:** **12 or 24** (128-bit or 256-bit entropy). Strength must be
  `128 | 160 | 192 | 224 | 256` and a multiple of 32 bits. Pre-Rust C path in
  `trezor-crypto/crypto/bip39.c::mnemonic_generate`:
  ```c
  if (strength % 32 || strength < 128 || strength > 256) return 0;
  uint8_t data[32] = {0};
  random_buffer(data, 32);
  const char *r = mnemonic_from_data(data, strength / 8, buf, buflen);
  memzero(data, sizeof(data));
  ```
- **Validation:** checksum enforced. `mnemonic_check()` accepts 12/15/18/21/24
  words; truncated 4/5/6/7/8 bits of SHA-256 over entropy compared to last word.
- **Wordlist:** English (BIP-39). Configurable check via
  `HDWallet(mnemonic, passphrase, check=true)`.

### 1.2 Mnemonic storage at rest

Trust Wallet Core **does not store the mnemonic itself on disk**. The trust-keystore
format (`StoredKey`) stores encrypted **private keys / derived addresses / xpub**, not
the BIP-39 phrase. There are two storage modes:

- **`StoredKey` JSON file** with optional encryption. From `swift/Sources/KeyStore.swift`
  and `StoredKey.h`:
  - Cipher: AES-128-CTR (also AES-256-CBC variant depending on `StoredKeyEncryption`).
  - KDF: **scrypt** (recommended) or PBKDF2. See PR #4755
    (`feat(StoredKey): Add API to fix and regenerate weak encryption parameters`,
    merged May 2026): "Adds logic to detect weak Scrypt parameters (like empty salt)
    and regenerate them with secure, random values".
  - File path: caller-chosen (iOS: app sandbox; Android: app sandbox; desktop: OS
    app-data dir). Atomic write via `storeWithTemporaryFile` (PR #4756, May 2026):
    "Writes the key JSON to `temporaryPath` first, then renames it to `path` in a
    single atomic operation".
- **Platform keystore integration (iOS/Android):** the iOS Trust Wallet app and
  Android Trust Wallet app wrap `KeyStore` (Swift) / `KeyStore` (Kotlin) and call
  `import(mnemonic:name:encryptPassword:coins:)` with the user's password; the
  StoredKey file ends up in the app sandbox. **The OS Keychain is not directly used
  by wallet-core itself** — the host app is responsible for placing the encrypted
  file inside its protected container (iOS app sandbox + optional Keychain item,
  Android `EncryptedSharedPreferences` / `AndroidKeyStore`-backed).

For comparison: the sibling **TWAK** (Trust Wallet Agent Kit, `developer.trustwallet.com/developer/agent-sdk/key-management.md`)
uses `AES-256-GCM` with PBKDF2-derived key for `~/.twak/wallet.json`. That is
**not** Trust Wallet Core itself but uses it as the BIP-39 source.

### 1.3 Mnemonic display

The mnemonic is returned to the host UI as a UTF-8 string via
`TWHDWalletMnemonic()` (`src/interface/TWHDWallet.cpp`); the host renders the
write-down flow. The core library itself has no UI; the standard pattern in the
Trust Wallet mobile apps is **show all words once on a dedicated screen, force
verification by re-entering a random subset**. (No UI code in wallet-core.)

### 1.4 Passphrase support (BIP-39 "25th word")

Full BIP-39 passphrase support via the `passphrase` constructor parameter
(`HDWallet(int strength, const std::string& passphrase)`,
`HDWallet(const std::string& mnemonic, const std::string& passphrase, ...)`).
From `trezor-crypto/crypto/bip39.c::mnemonic_to_seed`:
```c
PBKDF2_HMAC_SHA512_CTX ctx;
pbkdf2_hmac_sha512_Init(&ctx, (const uint8_t *)mnemonic, mnemoniclen,
                        (const uint8_t *)passphrase, passphraselen,
                        BIP39_PBKDF2_ROUNDS);
```
**2048 rounds, HMAC-SHA512**, with salt `"mnemonic" + passphrase`. Passphrase
is truncated at 256 chars internally. The seed result is cached in a small
LRU (`USE_BIP39_CACHE`) keyed by `(mnemonic, passphrase)` strings.

### 1.5 In-memory handling

- **C++ path (legacy):** `mnemonic_generate`, `mnemonic_from_data`, `mnemonic_to_seed`
  all `memzero()` working buffers before return
  (`trezor-crypto/crypto/bip39.c::mnemonic_generate` calls `memzero(data, 32)`,
  `mnemonic_from_data` calls `memzero(bits, sizeof(bits))`).
- **HDWallet class destructor:** `virtual ~HDWallet()` runs `TW::memzero` on
  the seed (64 bytes) and intermediate buffers (`src/HDWallet.cpp`,
  `memzero(buf, MnemonicBufLength)`).
- **Rust path (current):** uses `zeroize` and explicit drop semantics from
  the Rust standard library and `zeroize` crate idioms (see
  `src/HDWallet.cpp` comments around `TW::memzero(secret, CARDANO_SECRET_LENGTH)`).
- **No `mlock`** — neither path pins memory; they rely on zeroize-on-drop only.

### 1.6 Backup & recovery

- **Backup:** the encrypted `StoredKey` JSON file is the backup — losing the
  password loses access; losing the file loses the wallet. Trust Wallet mobile
  apps additionally support **exporting the unencrypted mnemonic** as a
  user-initiated action (manual "Backup Recovery Phrase" flow).
- **No explicit versioning header** in the StoredKey JSON for the mnemonic
  layer; cipher/kdf params are stored per-payload. The Keystore JSON
  format itself is **byte-for-byte identical** across the recent changes
  per the BC-risk audit on PR #4756 ("The keystore JSON format is byte-for-byte
  identical").

### 1.7 Sign-in flow

Pure library — no auth flow. Host apps (Trust Wallet mobile) layer in
biometric/passcode unlock of the host app container. The wallet-level
unlock is `decrypt(password:)` on `StoredKey`.

### 1.8 Multi-wallet

**One seed → many accounts.** `HDWallet::getKey(coin, derivationPath)` derives
per-coin keys (BIP-44/49/84 style paths are convention, not enforced).
The Multi-Coin Wallet structure is documented in `developer.trustwallet.com/.../wallet-core-usage`:
"a single recovery phrase" → "accounts for many coins".

### 1.9 Star count + last commit

Active. Recent merges on the master branch in 2026 include #4755 (fix weak
encryption params, May 2026), #4756 (atomic file storage, May 2026), and #4384
(Rust migration, Apr 2025).

---

## 2. BlueWallet (iOS / Android, React Native)

**Repo:** https://github.com/BlueWallet/BlueWallet · ⭐ ~2.5k+ · actively maintained
**Stack:** React Native (TypeScript + JavaScript), Swift/Kotlin native modules,
`realm` DB, AsyncStorage, iOS/Android Keychain.

### 2.1 Mnemonic generation

BIP-39 generation via the **`blue_modules/encryption.ts`** layer + a JS-side
BIP-39 library (`bip39`). Standard 12 words (optionally 24 in advanced flows).
Entropy comes from `react-native-get-random-values` → iOS `SecRandomCopyBytes`
/ Android `SecureRandom`.

### 2.2 Mnemonic storage at rest

BlueWallet stores **the entire encrypted wallet blob** (including mnemonic,
addresses, txs) in a Realm DB that is itself wrapped by a password-encrypted
envelope. The encryption module is `blue_modules/encryption.ts` — a faithful
port of the **legacy CryptoJS@4.x `AES.encrypt()` default** (kept bit-identical
so existing on-device wallets remain readable across the library swap).

From `blue_modules/encryption.ts`:
```ts
// "Salted__" — OpenSSL envelope magic
const SALT_MAGIC = new Uint8Array([0x53,0x61,0x6c,0x74,0x65,0x64,0x5f,0x5f]);
const SALT_LEN = 8, KEY_LEN = 32, IV_LEN = 16;

export function encrypt(data: string, password: string): string {
  const salt = randomBytes(SALT_LEN);
  const kdf = evpBytesToKeyMd5(stringToUint8Array(password), salt, KEY_LEN + IV_LEN);
  const key = kdf.subarray(0, KEY_LEN);
  const iv  = kdf.subarray(KEY_LEN);
  const ciphertext = cbc(key, iv).encrypt(stringToUint8Array(data));
  return uint8ArrayToBase64(concatUint8Arrays([SALT_MAGIC, salt, ciphertext]));
}
```

Summary of the cipher stack:
- **KDF:** OpenSSL `EVP_BytesToKey` with **MD5, 1 iteration** (intentionally
  matches CryptoJS default — the comment in source: "MD5 is intentional: it
  matches the legacy OpenSSL format. The cryptographic weakness of MD5 is not
  relevant here — the function is only used as a deterministic byte-stretcher;
  the password's entropy is what protects the wallet, not MD5").
- **Cipher:** AES-256-CBC, PKCS7 padding, 8-byte random salt.
- **Wire format:** base64(`Salted__` ‖ salt ‖ ciphertext).
- **OS keystore:** BlueWallet supports **Plausible Deniability** via multiple
  encrypted "buckets" (`https://bluewallet.io/docs/plausible-deniability/`):
  "BlueWallet stores your wallets in an encrypted container on your device.
  With Plausible Deniability, the app keeps multiple encrypted containers
  (called buckets). Each password unlocks exactly one of them."
- The iOS Keychain is used by the native `KeychainAccess` module for
  ancillary secrets (e.g. Lightning node seed) but the **wallet mnemonic blob
  is in the encrypted Realm container**, not directly in Keychain.
- The maintainer comment on the GitHub issue thread
  (`Ability to disable Encryption`, #201): "the storage is really encrypted,
  you can check the source code. If you'll manage to extract data from
  jailbroken iphone or from rooted android you should see only encrypted
  gibberish".

### 2.3 Mnemonic display

Native UI screen (React component `screen/wallets/pleaseBackupLnd.js` for
Lightning, plus the standard BIP-39 backup screen for on-chain) — shows all
words on one screen, requires the user to toggle a "I have written it down"
checkbox, then prompts a re-entry verification (word indices 3, 7, 11 etc.),
matching the standard Bitcoin Design Guide flow.

### 2.4 Passphrase support (BIP-39 25th word)

**Yes, supported.** Import flow: "Import with a passphrase, if your backup
uses a BIP39 passphrase (25th word)". The passphrase is **prompted at import
time only** — once the wallet is created, the encrypted blob contains the
BIP-39 seed (which already incorporates the passphrase via PBKDF2), so no
re-prompt is needed to spend.

### 2.5 In-memory handling

This is the weakest link in BlueWallet. The mnemonic is held as a JS
**string** for the lifetime of an unlocked wallet — JS strings are immutable,
GC-managed, copied across heap on concat. **It cannot be zeroed.** The
`blue_modules/encryption.ts` KDF key buffer is zeroed via `out.set(prev...)`
in-place but the password itself remains as a JS string argument.

This is the explicit motivation for the Rust pattern from `seedlock`:
> "a javascript string is immutable, garbage collected, and copied all over
> the heap. **you cannot zero a mnemonic in javascript.** every js wallet has
> this hole and none of them can close it from js."

### 2.6 Backup & recovery

- **Manual export** of the encrypted backup file (`.btcw` extension, encrypted
  with the same AES-256-CBC envelope as on-device storage) — can be sent over
  Signal/email/etc.
- **Plaintext mnemonic export** in the wallet details screen — user-initiated
  "Please Backup" flow shows the words, copies them to clipboard optionally.
- **Import via QR scan** of a BIP-39 mnemonic QR.
- **Lightning (LND) seed** is backed up separately via a 24-word aezeed
  (Lightning-specific), encrypted with the wallet password.
- **Plausible Deniability** is the headline differentiator: each encrypted
  bucket is fully independent; lose the main password = lose main wallets;
  lose decoy password = lose decoy; reveal only decoy under duress.

### 2.7 Sign-in flow

- **Password Protected** (encrypts the storage) + optional **Biometrics**
  (Touch ID / Face ID) as a **gate on the app**, not on the encryption key.
  Per `https://bluewallet.io/features/`: "Biometric security (touch ID, Face ID)
  is not safe, so you will have an additional password to encrypt your wallet
  instead."
- Under the hood the biometric is a Keychain item guard on iOS (passes
  through `KeychainAccess`); the underlying encrypted blob is still
  password-AES.

### 2.8 Multi-wallet

**One mnemonic per on-chain wallet** in most cases. A single BlueWallet install
can hold N wallets, each with its own mnemonic. There is also a **Vault** type
(watch-only) and **Lightning** type (LNDHub / LND node). Multiple BIP-39
passphrases of the same mnemonic = different wallets (advanced user choice).

### 2.9 Star count + last commit

Very active; the codebase merges PRs weekly. The `blue_modules/encryption.ts`
refactor to drop CryptoJS for `@noble/ciphers/aes` was a recent migration that
explicitly preserves the on-disk wire format.

---

## 3. Electrum (Python desktop + Qt / Android Kotlin)

**Repo:** https://github.com/spesmilo/electrum · ⭐ ~3k+ · actively maintained
**Stack:** Python core, Qt GUI (PyQt5/PyQt6 for desktop), Kotlin/QML for Android.

### 3.1 Mnemonic generation

Electrum has two seed types:

- **Old/legacy seed:** 13 words from Electrum's own wordlist, ~132 bits of
  entropy (`is_seed()` enforces this). The seed generator (in `lib/wallet.py`
  via `make_seed`) loops:
  ```python
  ss = "%040x"%(entropy+nonce)
  s = hashlib.sha256(ss.decode('hex')).digest().encode('hex')
  # we keep only 13 words, that's approximately 139 bits of entropy
  words = mnemonic.mn_encode(s)[0:13]
  seed = ' '.join(words)
  if is_seed(seed): break
  nonce += 1
  ```
- **BIP-39 seed:** standard 12 or 24 words, handled via the `bip39_to_seed`
  helper:
  ```python
  @classmethod
  def bip39_to_seed(mnemonic: str, *, passphrase: Optional[str]) -> bytes:
      import hashlib
      passphrase = passphrase or ""
      PBKDF2_ROUNDS = 2048
      mnemonic = normalize('NFKD', ' '.join(mnemonic.split()))
      passphrase = bip39_normalize_passphrase(passphrase)
      return hashlib.pbkdf2_hmac('sha512', mnemonic.encode('utf-8'),
                                 b'mnemonic' + passphrase.encode('utf-8'),
                                 iterations=PBKDF2_ROUNDS)
  ```
  **`PBKDF2_ROUNDS = 2048`, HMAC-SHA512**, salt `"mnemonic" + passphrase`
  (BIP-39 standard). Matches the Trezor reference.

### 3.2 Mnemonic storage at rest

Two layered encryption schemes — both are well-documented in `faq.rst`:

#### Layer A — keystore encryption (the seed / private keys themselves)

From `electrum/keystore.py::Deterministic_KeyStore` and PR #4838
(`keystore: stronger pbkdf for encryption`, SomberNight):

- **Version 1 (legacy):** symmetric key = `sha256d(password)` (no salt).
  Quoting the PR description: "Currently, when keystore encryption is enabled,
  the symmetric key used to encrypt the seed/private keys is `sha256d(password)`."
- **Version 2 (current, post-PR #4838):**
  ```
  PBKDF2-HMAC-SHA256, 50 000 iterations, 16-byte random per-wallet salt.
  ```
  "Newly created wallets will use version 2 automatically; and for existing
  wallets, if they change the password, they will get upgraded to version 2."

  The seed / xprv / WIF is then encrypted with **AES-256-CBC** under that key
  (`pw_encode` / `pw_decode` in `bitcoin.py`).

#### Layer B — wallet file (`electrum.dat`) storage encryption

From `electrum/storage.py`:

- Three states: **plaintext**, **BIE1** (ECIES, user-password), **BIE2**
  (ECIES, xpub-derived password — used with hardware wallets).
- Magic header: `b'BIE1'` or `b'BIE2'`.
- The password is mapped to an EC private key via:
  ```python
  secret = hashlib.pbkdf2_hmac('sha512', password.encode('utf-8'),
                               b'', iterations=1024)
  ec_key = ecc.ECPrivkey.from_arbitrary_size_secret(secret)
  ```
  (`PBKDF2-HMAC-SHA512, 1024 iterations, **no salt**` — historical weakness
  noted in `electrum/issues/3147`: "the (kind of outdated) PBKDF2 key derivation
  with no salt and a quite low iteration count").
- The serialized wallet JSON is then **zlib-compressed** and sealed with
  **ECIES** (`ecc.ECPubkey.encrypt_message` / `ECPrivkey.decrypt_message`).

**OS keystore:** none on the desktop side (filesystem permissions + the
encryption above). On Android (Kotlin/QML build) the file is in app-private
storage.

### 3.3 Mnemonic display

Qt GUI: shows all words in a grid, asks the user to retype a subset (3–4
random indices) to verify. Same pattern in the Android QML build.

### 3.4 Passphrase support

**BIP-39 passphrase supported** (`passphrase` field stored in the keystore
dict). From `electrum/keystore.py`:
```python
if self.passphrase:
    return pw_decode(self.passphrase, password,
                     version=self.pw_hash_version)
```
The passphrase is encrypted **inside the keystore** (which is itself
encrypted by layer A or layer B). It is **re-prompted** each time the wallet
is opened (unlike BlueWallet's one-time-at-import model).

### 3.5 In-memory handling

The wallet file is **decrypted once at load time** and the entire JSON
(including the still-encrypted seed, xprv, etc.) lives in Python memory as
strings for the session. Per the FAQ: "the wallet information will remain
unencrypted in the memory of your computer for the duration of your session".
Private keys are **decrypted only briefly, when you need to sign a
transaction** (txn signing path holds the key in a short-lived variable and
drops it). No `mlock`, no explicit zeroize — Python doesn't give reliable
zeroing for `str`/`bytes` either.

### 3.6 Backup & recovery

- The **wallet file itself is the backup** (`.dat` or named wallet file).
- No encrypted-backup-export format — copying the wallet file is the backup.
- Seed can be **exported as plain text** via the GUI ("Show seed" with
  password prompt).

### 3.7 Sign-in flow

Password prompt on wallet open (each session). For hardware wallets, the
BIP-39 passphrase is requested separately. No biometric option on desktop.

### 3.8 Multi-wallet

Multiple wallet **files** per `~/.electrum/wallets/` directory; each is
independent. One seed per wallet file in the typical case, but a single
BIP-39 seed can back multiple wallets via different derivation paths (this
is supported but not the default UI).

### 3.9 Star count + last commit

Very active. PR #4838 (stronger PBKDF2 for keystore) merged, and discussion in
`#3147` / `#5999` / `#4909` continues about stronger storage encryption
(versioned KDF header is a future direction).

---

## 4. Sparrow Wallet (Java / JavaFX desktop)

**Repo:** https://github.com/sparrowwallet/sparrow · ⭐ ~2k+ · actively maintained
**Stack:** Java 25, JavaFX, H2 embedded DB (`*.mv.db`).

### 4.1 Mnemonic generation

BIP-39 via the **`bitcoinj` `MnemonicCode`** class (12 or 24 words).
Entropy from `SecureRandom` (Java's default). The Sparrow UI default is
**12 words**; the Quick Start guide explicitly walks the user through
"Click Enter 12 Words. You will now see 12 text fields … Click the Generate
New Button to get Sparrow to randomly choose 12 words".

### 4.2 Mnemonic storage at rest

Sparrow's wallet store is an **H2 database** file (`<wallet>.mv.db`). When
encrypted, the entire DB is wrapped. From the academic security analysis
(`doi.org/10.3390/electronics13132433`, Table 5 — directly verified against
Sparrow source):

| Parameter     | Value          |
| ------------- | -------------- |
| Hash length   | 32             |
| Salt length   | 16             |
| Iterations    | 10             |
| Memory (KiB)  | 256 × 1024     |
| Parallelism   | 4              |
| Cipher        | AES-128-CBC    |

That is **`Argon2id`** (the 2015 PHC winner), hardcoded. Sparrow's docs claim
it is tuned to "take at least 500ms on modern hardware" (`sparrowwallet.com/features`).

Encryption flow (`src/main/java/com/sparrowwallet/sparrow/io/JsonPersistence.java`,
via the Stacker.news thread pointing at line 167 of commit `4feb4a3a`):
**ECIES** — derive EC key from Argon2-derived bytes, then encrypt with ECIES
(per `rijndael`'s comment: "it uses ECIES:
https://github.com/sparrowwallet/sparrow/blob/4feb4a3a79a3bbe69178fbefa38cd530fe963240/src/main/java/com/sparrowwallet/sparrow/io/JsonPersistence.java#L167").

A wallet `Storage` holds:
- `encryptionPubKey` — used to verify password (re-derive → compare pubkey).
- `keyDeriver` — `AsymmetricKeyDeriver` (Argon2id params).
- Master seed / xprv are encrypted **per-keystore** under `key` derived from
  the password + salt (`SettingsController.java`:
  `Key key = new Key(encryptionFullKey.getPrivKeyBytes(), storage.getKeyDeriver().getSalt(), EncryptionType.Deriver.ARGON2);`).

### 4.3 Mnemonic display

12 words shown in a 4-column grid; user must **re-enter the words**
("Re-enter Words…"). UI validates with "Valid checksum" message before
proceeding. Verbatim from the Quick Start docs:
"Sparrow checks that you have done this process correctly by asking you to
re-enter the words. … If your words are correct, Sparrow will indicate this
by displaying message with 'Valid checksum'."

### 4.4 Passphrase support (BIP-39 25th word)

**Yes.** Critical design property from `sparrowwallet/sparrow#603` (craigraw's
comment, May 2022):
> "A Sparrow Wallet `*.mv.db` file stores nothing derived from the passphrase.
> This means that if the wallet file is decrypted, a brute force attack to
> locate any funds must not only calculate the seed from the BIP39 words +
> passphrase, but also calculate addresses and search for any funds on the
> blockchain. … this is … significantly more expensive … and also scales as
> the blockchain grows."

This is a deliberate design choice that **brute-force resistance grows with
chain size**. Post-fix: passphrase is requested **once at import**, not at
every open. From the same thread, fix `90224383`:
1. Password field cannot be empty on encrypted wallet load
2. The passphrase will not be re-requested on importing a BIP39 wallet
3. Empty passphrases shown as "No Passphrase" in the seed view

### 4.5 In-memory handling

- During unlock: `ECKey encryptionFullKey = keyDerivationService.getValue()`
  — lives briefly.
- `Key` (the AEAD key) is used to decrypt keystores, then a `finally` block
  calls `key.clear()` and `encryptionFullKey.clear()` (see
  `SettingsController.java` "finally { encryptionFullKey.clear(); if(key != null)
  { key.clear(); } }"). `ECKey.clear()` zeros the private bytes.
- **No `mlock`** (pure-Java; limited options anyway).

### 4.6 Backup & recovery

- **Wallet export** is the encrypted `.mv.db` file itself (or an
  `.json` text export for hardware wallets).
- **`*.mv.db` is portable** — copy to a new install, enter password, open.
- Sparrow signs release binaries with `craigraw`'s GPG key (fingerprint
  `D4D0D3202FC06849A257B38DE94618334C674B40`) for authenticity.

### 4.7 Sign-in flow

Password dialog (modal) on wallet open. Per
`AppController.java::openWallet`, `WalletPasswordDialog` collects password,
a background `Service<ECKey>` derives the Argon2id key, then
`storage.restorePublicKeysFromSeed(wallet, key)` brings the wallet into a
usable state. **No biometric option** (desktop only).

### 4.8 Multi-wallet

**One seed per wallet file** is the typical case. Sparrow supports multi-wallet
configurations via **nested wallets** (e.g. a multisig "parent" wallet
referencing N cosigner wallets, each potentially with its own seed).
Mixed seed + passphrase is supported, with the explicit "stores nothing
derived from the passphrase" property.

### 4.9 Star count + last commit

Very active. Recent commits (last 30 days) include work on miniscript
policies, BSMS export, and the `Config.keyDerivationPeriod` toggle for
Argon2 auto-calibration timing.

---

## 5. Bitcoin Core (C++, reference implementation)

**Repo:** https://github.com/bitcoin/bitcoin · ⭐ ~80k+ · actively maintained
**Stack:** C++20, Berkeley DB (`wallet.dat`), LevelDB (chainstate).

### 5.1 Mnemonic generation

Bitcoin Core **does not generate BIP-39 mnemonics itself**. HD wallets (since
v0.13) generate a **512-bit internal HD seed** via
`CKey::MakeNewKey` / `GenerateNewHDMasterKey()` and store it as an encrypted
private key in `wallet.dat`. The mnemonic concept is **not used at the
protocol level** — there is no BIP-39 wordlist in the codebase. (Some forks
like BTCPay Server or wallet front-ends sit on top and translate the HD
seed into a BIP-39 mnemonic for human backup; Core itself does not.)

### 5.2 Mnemonic storage at rest

`wallet.dat` is a **Berkeley DB** B-tree. The encryption envelope (from
`src/wallet/crypter.h`):

```cpp
/**
 * Private key encryption is done based on a CMasterKey,
 * which holds a salt and random encryption key.
 * CMasterKeys are encrypted using AES-256-CBC using a key
 * derived using derivation method nDerivationMethod
 * (0 == EVP_sha512()) and derivation iterations nDeriveIterations.
 */
class CMasterKey {
public:
    std::vector<unsigned char> vchCryptedKey;
    std::vector<unsigned char> vchSalt;
    unsigned int nDerivationMethod;     // 0 = EVP_sha512()
    unsigned int nDeriveIterations;     // default 25000
    std::vector<unsigned char> vchOtherDerivationParameters;
    static constexpr unsigned int DEFAULT_DERIVE_ITERATIONS = 25000;
};
```

Concrete flow (`src/wallet/wallet.cpp::CWallet::EncryptWallet`):

1. Generate random 32-byte `_vMasterKey` (the actual wallet-encryption key).
2. Generate random 16-byte `vchSalt`.
3. **Calibrate KDF:** run `crypter.SetKeyFromPassphrase(passphrase, salt, 25000, ...)`,
   measure wall time, target 2 500 000 / measured-time iterations, clamp to ≥ 25 000.
   Comment in source: `"25000 rounds is just under 0.1 seconds on a 1.86 GHz
   Pentium M"` — the original floor — but the calibration aims for roughly
   half a second on the host that did the encryption.
4. **Encrypt** the master key with AES-256-CBC under the KDF-derived key →
   `vchCryptedKey`; write `CMasterKey` to DB.
5. **Encrypt every private key** in the wallet with AES-256-CBC under the
   master key, IV = `SHA256d(pubkey)`. (From crypter.h: "Wallet Private Keys
   are then encrypted using AES-256-CBC with the double-sha256 of the public
   key as the IV, and the master key's key as the encryption key".)
6. `Lock()` then `Unlock(passphrase)` to verify the user typed it correctly.
7. If `IsHDEnabled()`, **regenerate the HD seed** and `SetHDMasterKey(
   GenerateNewHDMasterKey())`. The `doc/managing-wallets.md` warning is loud:
   "**IMPORTANT** For security reasons, the encryption process will generate
   a new HD seed, resulting in the creation of a fresh set of active
   descriptors. Therefore, it is crucial to securely back up the newly
   generated wallet file using the `backupwallet` RPC."

OS-level protection: none on Linux/macOS beyond filesystem permissions.
The `wallet.dat` file is fully self-contained.

### 5.3 Mnemonic display

N/A — Bitcoin Core has **no mnemonic UI**. For descriptor wallets, the user
can back up via `listdescriptors` and `dumpwallet` RPC, which export the
**descriptors** (the public template) plus the **encrypted** private key
material. There is **no on-screen "write down 12 words"** flow.

### 5.4 Passphrase support

**No BIP-39 passphrase in Core itself.** There is the **wallet passphrase**
(used to encrypt the wallet — §5.2) which is *separate* from any mnemonic
passphrase. `doc/managing-wallets.md` is explicit:
> "The wallet passphrase and the seed are two separate components in wallet
> security. The seed, or HD seed, functions as a master key for deriving
> private and public keys in a hierarchically deterministic (HD) wallet.
> In contrast, the passphrase serves as an additional layer of security
> specifically designed to secure the private keys within the wallet."

### 5.5 In-memory handling — this is the gold standard

From `src/wallet/wallet.h` and `src/wallet/crypter.h`:
- Master key type: `typedef std::vector<unsigned char> CKeyingMaterial;`
- All sensitive material is held as `CKeyingMaterial` (a `std::vector<uint8_t>`).
- Wipe helper: `memory_cleanse(vchKey.data(), vchKey.size())` — a
  volatile-write + memory-fence wrapper to defeat compiler dead-store
  elimination.
- `CCrypter::~CCrypter()` calls `CleanKey()` which `memory_cleanse()`s both
  `vchKey` and `vchIV` and sets `fKeySet = false`.
- The wallet passphrase is typed as **`SecureString`** (`typedef
  std::basic_string<char, std::char_traits<char>, secure_allocator<char>>
  SecureString;`) — backed by a custom `secure_allocator` that wipes the
  buffer on deallocation.
- Unlock lifecycle: `CWallet::Unlock(passphrase)` decrypts the master key,
  calls `CCryptoKeyStore::Unlock(_vMasterKey)`; the master key lives in
  `vMasterKey` for the session. `CWallet::Lock()` calls
  `CCryptoKeyStore::Lock()` which `memory_cleanse`s.
- `Unlock` is **time-bounded** via `WalletPassphrase` RPC: caller specifies
  timeout in seconds; a timer auto-locks.

**No `mlock`** — `memory_cleanse` only protects against dead-store
optimization, not paging. Same threat model as the other wallets.

### 5.6 Backup & recovery

- **`backupwallet <destination>` RPC** — copies `wallet.dat` to the target.
  Failure case: for descriptor wallets the destination must be a filename
  ("Error: Wallet backup failed!" otherwise) — historical footgun called out
  in `doc/managing-wallets.md`.
- **`dumpwallet <filename.json>`** — plaintext JSON dump of all descriptors
  + keys (intended to be re-encrypted at rest by the operator).
- **`createwallet`** with `passphrase=…` argument bakes the encryption in at
  creation time.
- **`migratewallet` RPC** converts legacy non-descriptor wallets to
  descriptor wallets (changes the address derivation paths to BIP-44/49/84/86
  even though the same BIP-32 seed is used).

### 5.7 Sign-in flow

- `walletpassphrase <passphrase> <timeout>` RPC unlocks for N seconds.
- `walletlock` RPC force-locks immediately.
- No biometric (CLI/server). GUI (Bitcoin Core Qt) shows a password dialog.

### 5.8 Multi-wallet

`createwallet <name> [passphrase] [avoid_reuse]` — **one wallet = one
`wallet.dat`**. Each has its own encryption envelope, its own HD seed.
Descriptor wallets can have multiple `DescriptorScriptPubKeyMan`s, but they
all derive from a single master seed per wallet.

### 5.9 Star count + last commit

Very active. The `wallet.dat` Berkeley DB format is being **gradually
migrated** to a SQLite-based wallet in v30 (in progress at the time of
writing). For descriptor wallets the format is already different (see
`wallet/sqlite.cpp` and `migratewallet`).

---

## Comparison Matrix

| Feature                          | Trust Wallet Core | BlueWallet | Electrum           | Sparrow         | Bitcoin Core       |
| -------------------------------- | ----------------- | ---------- | ------------------ | --------------- | ------------------ |
| BIP-39 mnemonics                 | Yes (Rust)        | Yes (JS)   | Yes (legacy+BIP39) | Yes             | **No**             |
| Word count default               | 12 (128 bits)     | 12         | 13 (legacy) / 12   | 12              | n/a                |
| BIP-39 passphrase (25th word)    | Yes               | Yes        | Yes                | Yes             | n/a                |
| Encrypted-at-rest on disk        | Yes (StoredKey)   | Yes (Realm)| Yes (BIE1/BIE2)    | Yes (H2 + ECIES)| Yes (BDB + AES)    |
| KDF                              | scrypt / PBKDF2   | EVP_BytesToKey-MD5 (legacy)| PBKDF2-SHA256 50k (v2 keystore); PBKDF2-SHA512 1k (storage)| **Argon2id** (256 MiB, 10 iter, p=4)| EVP-SHA512, ~25k–2.5M iter, auto-calibrated |
| Cipher                           | AES-128-CTR / AES-256-CBC | AES-256-CBC | AES-256-CBC | AES-128-CBC | AES-256-CBC |
| Per-wallet random salt           | Yes (PR #4755)    | Yes (8B)   | Yes (v2 keystore) / **No** (storage) | Yes (16B)       | Yes (16B)          |
| Atomic file writes               | Yes (PR #4756)    | Yes        | Yes                | Yes             | `dbw->Rewrite()` after encryption |
| Plausible deniability            | No (host app may) | **Yes** (multi-bucket)| No                 | **Yes** (deliberate: stores nothing derived from passphrase)| No       |
| Biometric unlock                 | Host app          | Yes (gate, not crypto)| No           | No (desktop)    | No (CLI)           |
| In-memory zeroize on drop        | Yes (`TW::memzero`) | **No** (JS string immutability)| Partial (txn sign only)| Yes (`ECKey.clear()`)| **Yes** (`memory_cleanse` + `SecureString`) |
| `mlock` (no-swap)                | No                | No         | No                 | No              | No                 |
| Multi-wallet                     | One seed → many coins | N wallets per install | N files | N wallets (incl. nested multisig) | N `wallet.dat` per node |
| Open-source license              | MIT               | MIT        | MIT                | Apache-2.0      | MIT                |
| Star count (~)                   | ~3.0k             | ~2.5k      | ~3k                | ~2k             | ~80k               |

**Where each is strongest:**
- Trust Wallet Core: cleanest Rust API, atomic writes, fixes for weak
  encryption parameters, broadest chain coverage.
- BlueWallet: best UX, plausible deniability via multi-bucket.
- Electrum: most audited (decade of public scrutiny), simple file format.
- Sparrow: strongest KDF (Argon2id, 256 MiB), thoughtful passphrase threat
  model.
- Bitcoin Core: best in-memory hygiene (`memory_cleanse`, `SecureString`,
  `CKeyingMaterial`), time-bounded unlock, but **no BIP-39** — its own
  encrypted wallet.dat format.

**Where each is weakest:**
- Trust Wallet Core: no `mlock`, mnemonic lives as a normal `std::string`
  in `HDWallet::mnemonic` until destructor.
- BlueWallet: JS string immutability, MD5 KDF (legacy crypto-js compat),
  Realm DB is the encrypted container but is itself a fairly heavy target.
- Electrum: v1 keystore uses `sha256d(password)` (no salt, no KDF);
  storage encryption uses **PBKDF2-SHA512 with 1024 iterations and no salt**
  (issue #3147).
- Sparrow: no `mlock`, KDF params are hardcoded (no per-device calibration
  like Core).
- Bitcoin Core: no BIP-39, no plausible deniability, no biometric.

---

## Recommendations for `bitcoin-wallet-core`

Mapping to ADR 0001 milestones.

### v0.1 — minimal CLI (single-wallet, plaintext at rest)

Goal: get a usable Bitcoin wallet CLI shipped. Plaintext mnemonic on disk
behind filesystem permissions is acceptable for v0.1 **only** if the user
opts in (e.g. a `--plaintext` flag), and the default path should require
explicit consent. Even v0.1 should:

- **Generate 12-word BIP-39** via `bip39` crate (well-audited) + `rand`'s
  OS RNG (`getrandom` on Unix, `BCryptGenRandom` on Windows).
- **Use `zeroize`** for any in-memory `SecretVec` / `Zeroizing<Vec<u8>>`.
  Even though v0.1 has no encryption, the buffer discipline pays off later.
- **Validate input** against the English wordlist with checksum
  (`Mnemonic::validate`).
- **Derive BIP-84 (native segwit) by default** at `m/84'/0'/0'/0/0` —
  matches Sparrow/BlueWallet defaults; explicit `--legacy / --nested-segwit
  / --taproot` flags for the other standards.
- **CLI prompt for BIP-39 passphrase** as a separate `--passphrase` /
  `BIP39_PASSPHRASE` env / hidden TTY read, not concatenated with the
  mnemonic.
- **Warn loudly** if the wallet file is created with mode 0644 or world-
  readable on Unix; refuse to create in a world-writable directory.

### v0.2 — encrypted-at-rest, single-wallet

Adopt the **Sparrow / Bitcoin Core hybrid** as the model:

- **KDF:** **Argon2id** as the default; **scrypt** as a documented
  alternative for users who want to round-trip with Trust Wallet Core
  `StoredKey` JSON files. Reject PBKDF2 < 600k iterations.
- **Argon2id parameters:** calibrated to 500 ms on the user's machine,
  clamped to a minimum of `m=64 MiB, t=3, p=1`. Store the parameters
  alongside the salt in the wallet file header so we can change them later
  (key rotation path).
- **Cipher:** **AES-256-GCM** (authenticated, avoids the PKCS7 padding-
  oracle trap of CBC). 12-byte random nonce per encryption operation;
  authentication tag stored in the file.
- **Wallet file format:**
  ```
  MAGIC (4B = "BWLT") ‖ VERSION (1B) ‖
  KDF_ID (1B) ‖ KDF_PARAMS (variable) ‖
  SALT (16B) ‖ NONCE (12B) ‖ CIPHERTEXT (variable) ‖ TAG (16B)
  ```
  ASCII-armored base64 for terminal-friendly export. Versioned from day 1
  (v0.2 uses version=1; reserve version=2 for future Argon2id-only).
- **Atomic writes** (write to `wallet.bak`, `fsync`, `rename(2)` on the
  same filesystem) — this is what Trust Wallet Core added in PR #4756 and
  it is the right pattern.
- **In-memory:** use `mlock` + `zeroize` + `subtle::ConstantTimeEq`
  (modeled on the `seedlock` crate). Wrap the seed in a `Secret<T>` newtype
  that drops with `ZeroizeOnDrop`.
- **BIP-39 passphrase:** stored **only** inside the encrypted envelope;
  prompted each session (Sparrow pre-fix behavior), or single-time
  at-import (BlueWallet / Sparrow post-fix) — the user chooses, default is
  **re-prompt per session** for higher security.
- **Plausible deniability:** *not in v0.2.* Note it as a v1.x feature.

### v1.0 — multi-wallet, plausible deniability, hardware-wallet interop

- **Multi-wallet:** a single `wallet.dat`-style file holding N wallets,
  each with its own seed/passphrase/derivation path. The container
  encrypts the whole directory tree under one Argon2id-derived KEK; each
  wallet's payload has its own random nonce + AEAD seal.
- **Plausible deniability:** adopt Sparrow's deliberate design — **store
  nothing derived from the passphrase**. A wrong passphrase decrypts to
  garbage addresses, indistinguishable from a real wallet; brute-force
  requires blockchain scan (matching Sparrow's threat model). For extra
  paranoia, support **decoy wallets** (BlueWallet-style: a wrong password
  opens a different wallet tree).
- **Hardware wallet interop:** keep mnemonic import/export bit-compatible
  with the **Electrum keystore JSON** and the **Trust Wallet Core `StoredKey`
  JSON** as documented interop targets. This lets users round-trip with
  Sparrow, BlueWallet, Electrum, and the Trust Wallet mobile app.
- **Constant-time comparisons everywhere** (`subtle::ConstantTimeEq`) —
  signing paths, mnemonic comparisons, password checks.
- **`mlock` the seed bytes** at construction; refuse to operate on systems
  where `mlock` fails (warn loudly, do not silently degrade to
  non-pinned-memory mode). Provide a `--no-mlock` escape hatch for
  sandboxes.
- **BIP-39 passphrase brute-force resistance:** explicitly support
  **empty passphrase = ""** and treat it as a first-class case (don't
  special-case absent vs empty). This matches `seedlock`'s BIP-39 vectors
  and the BIP-39 spec ("An empty string is a valid passphrase").
- **Self-test suite:** include the **Trezor BIP-39 test vectors** (the same
  set Trust Wallet Core added in PR #4384) as the v1.0 acceptance gate.

### Cross-cutting non-goals

- **Browser/JS bindings** in v1.0. The whole point of going Rust is to
  avoid BlueWallet's `you cannot zero a mnemonic in javascript` failure
  mode (per `seedlock` README). If we ever ship a JS/wasm binding, it must
  call into a Rust subprocess that holds the secret, never hold it in
  wasm linear memory that the JS GC can see.
- **Cloud sync, social recovery, MPC** — explicitly out of scope. This
  design is for **self-custody single-signer** only.

---

## Appendix A — Source-of-truth citations

| Wallet            | File / URL                                                                                  | Lines / snippet                                 |
| ----------------- | ------------------------------------------------------------------------------------------- | ----------------------------------------------- |
| Trust Wallet Core | `rust/tw_mnemonic` (post-#4384) + `trezor-crypto/crypto/bip39.c`                           | `mnemonic_to_seed` (PBKDF2-SHA512 2048)         |
| Trust Wallet Core | `src/HDWallet.cpp`                                                                          | `TW::memzero(buf, MnemonicBufLength)`           |
| Trust Wallet Core | `swift/Sources/KeyStore.swift`                                                              | `StoredKey.importHDWithEncryption` + `StoredKeyEncryption.aes128Ctr` |
| Trust Wallet Core | PR #4756 atomic writes                                                                      | `storeWithTemporaryFile`                        |
| Trust Wallet Core | PR #4755 weak-encryption-param fix                                                          | `ScryptParameters::shouldFix` + regenerate      |
| Trust Wallet Core | `developer.trustwallet.com/.../key-management.md` (TWAK, not Core itself)                   | `AES-256-GCM` + PBKDF2                          |
| BlueWallet        | `blue_modules/encryption.ts`                                                                | full file (`evpBytesToKeyMd5`, AES-256-CBC)     |
| BlueWallet        | `bluewallet.io/docs/plausible-deniability/`                                                 | "Plausible Deniability ... multiple encrypted containers (called buckets)" |
| BlueWallet        | `bluewallet.io/features/`                                                                   | "Biometric security (touch ID, Face ID) is not safe, so you will have an additional password" |
| BlueWallet        | Issue #201 (encryption disable request)                                                     | "the storage is really encrypted ... see only encrypted gibberish" |
| Electrum          | `electrum/keystore.py::bip39_to_seed`                                                       | `PBKDF2_ROUNDS = 2048`                          |
| Electrum          | `electrum/storage.py::get_eckey_from_password`                                              | `pbkdf2_hmac('sha512', ..., b'', iterations=1024)` |
| Electrum          | PR #4838 (SomberNight)                                                                      | `PBKDF2-HMAC-SHA256, 50k iterations, 16-byte salt` (v2 keystore) |
| Electrum          | Issue #3147                                                                                 | "PBKDF2 key derivation with no salt and a quite low iteration count" |
| Electrum          | `faq.rst`                                                                                   | "your seed and private keys are encrypted using AES-256-CBC" |
| Sparrow           | `sparrowwallet.com/features`                                                                | "Argon2 ... take at least 500ms on modern hardware" |
| Sparrow           | `sparrowwallet.com/docs/faq.html`                                                           | "Sparrow stores nothing in your wallet file that is derived from your passphrase" |
| Sparrow           | `src/main/java/com/sparrowwallet/sparrow/io/Storage.java` + `JsonPersistence.java:167`      | `EncryptionType.Deriver.ARGON2` + ECIES seal     |
| Sparrow           | Issue #603 (craigraw's passphrase fix)                                                      | `90224383` — passphrase not re-requested at import |
| Sparrow           | doi.org/10.3390/electronics13132433 (academic)                                              | Table 5 — Argon2id params, AES-128-CBC           |
| Bitcoin Core      | `src/wallet/crypter.h`                                                                      | `CMasterKey` (vchSalt, nDeriveIterations=25000)  |
| Bitcoin Core      | `src/wallet/wallet.cpp::CWallet::EncryptWallet`                                            | dynamic iteration calibration                   |
| Bitcoin Core      | `src/wallet/wallet.h`                                                                       | `CKeyingMaterial vMasterKey GUARDED_BY(cs_wallet)` |
| Bitcoin Core      | `doc/managing-wallets.md`                                                                   | "the encryption process will generate a new HD seed" |

## Appendix B — Rust crates worth knowing about

| Crate        | Use                                                  | Why we care                                                |
| ------------ | ---------------------------------------------------- | ---------------------------------------------------------- |
| `bip39`      | BIP-39 wordlist + seed derivation                    | Battle-tested; Trezor reference vector compatibility      |
| `zeroize`    | `Zeroize` / `Zeroizing` / `ZeroizeOnDrop`            | Foundation for in-memory hygiene                           |
| `subtle`     | `ConstantTimeEq`                                     | Avoid timing leaks on password/seed comparison            |
| `argon2`     | Argon2id KDF                                         | OWASP/NIST-recommended password KDF                        |
| `scrypt`     | scrypt KDF                                           | Interop with Trust Wallet Core `StoredKey`                 |
| `aes-gcm`    | AEAD cipher                                          | AES-256-GCM with 96-bit nonce, 128-bit tag                 |
| `chacha20poly1305` | AEAD cipher (alt)                               | ARM-friendly; useful if we ever ship mobile                |
| `getrandom`  | OS RNG                                               | `getrandom(2)` on Unix, `BCryptGenRandom` on Windows        |
| `mlock` (or `libc::mlock`) | Pin memory                      | `seedlock` uses this; unix-only                            |
| `secret-service` (or `keyring`) | OS keystore integration | Optional v1.0 — desktop Linux Secret Service / macOS Keychain |
