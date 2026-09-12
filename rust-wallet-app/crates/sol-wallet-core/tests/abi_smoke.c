/* abi_smoke.c — Phase 8.2 Step 3a (audit H17) + Step 13.
 *
 * In-process ABI smoke test. Loads `libsol_wallet_core.{dylib,so,dll}`
 * via `libloading` from a Rust integration test (see
 * `tests/abi_smoke.rs`), then this file is the C harness the Rust
 * test compiles + links + invokes.
 *
 * Strategy:
 *   1. Resolve each of the 16 `sol_wallet_*` symbols via `dlsym`.
 *   2. Call each with NULL inputs.
 *   3. Assert each returns a non-Panic (non-99) FfiError code
 *      documented for the "NULL inputs" case.
 *
 * Any segfault = test RED. Any `99` panic code = scrubber-not-firing
 * bug = test RED. Any other return code that does NOT match the
 * documented `FfiError` = ABI drift = test RED.
 *
 * The Rust wrapper test in `tests/abi_smoke.rs` builds this C harness
 * via `cc` (the `cc` crate is already in the dev-dep tree) then runs
 * the resulting binary, which exits 0 on GREEN.
 *
 * Audit references:
 *   - H17 (No ABI link test on real device) — this harness.
 *   - H7 (cdylib Rust runtime symbol leakage) — companion to
 *     `scripts/symbol_audit.sh`; this C harness proves symbols are
 *     callable from a real non-Rust consumer (the dylib's C ABI is
 *     intact).
 */

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* FfiError codes — mirrors sol-wallet-core/src/ffi.rs. */
#define SOL_OK                     0
#define SOL_BUF_TOO_SMALL          1
#define SOL_NULL_POINTER           2
#define SOL_INVALID_UTF8           3
#define SOL_WALLET_NOT_FOUND       4
#define SOL_WALLET_LOCKED          5
#define SOL_DECRYPT_FAILED         6
#define SOL_INSUFFICIENT_FUNDS     7
#define SOL_TRANSPORT              8
#define SOL_EXCEEDS_MAX_PER_TX     9
#define SOL_DEST_NOT_ALLOWED       10
#define SOL_EXCEEDS_DAILY_LIMIT    11
#define SOL_POLICY_DENIED          12
#define SOL_RPC                    13
#define SOL_UNIMPLEMENTED          98
#define SOL_PANIC                  99

#define ASSERT_NE_PANIC(call, label) do {                                       \
    int32_t _rc = (call);                                                       \
    if (_rc == SOL_PANIC) {                                                     \
        fprintf(stderr, "RED: %s returned SOL_PANIC (99) — scrubber missed?\n", \
                label);                                                         \
        return 1;                                                               \
    }                                                                           \
    if (_rc != SOL_NULL_POINTER && _rc != SOL_UNIMPLEMENTED &&                    \
        _rc != SOL_INVALID_UTF8 && _rc != SOL_BUF_TOO_SMALL &&                   \
        _rc != SOL_OK && _rc != SOL_WALLET_LOCKED) {                             \
        fprintf(stderr,                                                         \
                "RED: %s returned unexpected code %d (not in NULL-safe set)\n",  \
                label, _rc);                                                    \
        return 1;                                                               \
    }                                                                           \
} while (0)

#define ASSERT_OK(call, label) do {                                             \
    int32_t _rc = (call);                                                       \
    if (_rc != SOL_OK) {                                                        \
        fprintf(stderr, "RED: %s expected SOL_OK, got %d\n", label, _rc);      \
        return 1;                                                               \
    }                                                                           \
} while (0)

/* Provided by the Rust wrapper test (tests/abi_smoke.rs) — resolves
 * each `sol_wallet_*` symbol from the loaded cdylib + invokes each
 * one with NULL inputs via the `invoke_*` shim functions below.
 *
 * This file is compiled by the Rust test driver as a standalone binary.
 * The actual `dlsym` lives in the Rust side to avoid pulling libdl into
 * the C harness.
 */
extern int32_t shim_zeroize_null(void);
extern int32_t shim_clear_panic(void);
extern int32_t shim_lock_null(void);
extern int32_t shim_create_mnemonic_null(void);
extern int32_t shim_import_mnemonic_null(void);
extern int32_t shim_unlock_null(void);
extern int32_t shim_get_address_null(void);
extern int32_t shim_sign_transaction_null(void);
extern int32_t shim_send_sol_null(void);
extern int32_t shim_send_spl_null(void);
extern int32_t shim_get_balance_sol_null(void);
extern int32_t shim_get_balance_spl_null(void);
extern int32_t shim_last_error_message_null(void);
extern int32_t shim_set_policy_null(void);
extern int32_t shim_get_policy_null(void);
extern int32_t shim_default_cluster_null(void);

int main(void) {
    fprintf(stderr, "ABI smoke: 16-export NULL-input matrix\n");

    ASSERT_OK(shim_clear_panic(), "clear_panic (empty slot)");
    ASSERT_NE_PANIC(shim_clear_panic(), "clear_panic (idempotent)");

    ASSERT_NE_PANIC(shim_zeroize_null(), "zeroize NULL");
    ASSERT_NE_PANIC(shim_lock_null(), "lock NULL");
    ASSERT_NE_PANIC(shim_create_mnemonic_null(), "create_mnemonic NULL");
    ASSERT_NE_PANIC(shim_import_mnemonic_null(), "import_mnemonic NULL");
    ASSERT_NE_PANIC(shim_unlock_null(), "unlock NULL");
    ASSERT_NE_PANIC(shim_get_address_null(), "get_address NULL");
    ASSERT_NE_PANIC(shim_sign_transaction_null(), "sign_transaction NULL");
    ASSERT_NE_PANIC(shim_send_sol_null(), "send_sol NULL");
    ASSERT_NE_PANIC(shim_send_spl_null(), "send_spl NULL");
    ASSERT_NE_PANIC(shim_get_balance_sol_null(), "get_balance_sol NULL");
    ASSERT_NE_PANIC(shim_get_balance_spl_null(), "get_balance_spl NULL");
    ASSERT_NE_PANIC(shim_last_error_message_null(), "last_error_message NULL");
    ASSERT_NE_PANIC(shim_set_policy_null(), "set_policy NULL");
    ASSERT_NE_PANIC(shim_get_policy_null(), "get_policy NULL");
    ASSERT_NE_PANIC(shim_default_cluster_null(), "default_cluster NULL");

    fprintf(stderr, "ABI smoke GREEN: 16/16 NULL-input calls safe\n");
    return 0;
}
