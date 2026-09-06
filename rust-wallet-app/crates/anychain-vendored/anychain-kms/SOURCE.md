# SOURCE — anychain-kms (vendored)

**Source repo:** https://github.com/0xcregis/anychain
**Pinned commit:** `cf3aa2d59afb2c50dc961919fca011c401238ed6`
**Commit subject:** `build: rustup toolchain update to 1.98.0 (#465)`
**Commit date:** 2026-09-04 15:53:11 +0800
**Vendored on:** 2026-09-06
**Vendored by:** tron-wallet-core v0.1 Phase 0 (plan Task 0.2)

## Pin deviation from plan

Plan reference (`docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`)
cites upstream tag `v0.1.23` for `anychain-kms`. The tag **does not exist** on
the upstream remote (`git ls-remote --tags https://github.com/0xcregis/anychain.git`
returned only `0.0.2` and `0.1.5` as of 2026-09-06). Upstream was sourced at
HEAD of `main` instead — commit SHA recorded above.

Plan amendment deferred (post-Phase 0).

## Source path

Files copied from `.local/anychain/crates/anychain-kms/src/**` at commit `cf3aa2d`
(repo at repo root, kept as read-only reference for inspection; not used by
consumer builds).

## License

Per-file `SPDX-License-Identifier: MIT OR Apache-2.0` preserved on every
vendored source file.

## Local patches

Layered on top of vendored copy per plan Task 0.7:

1. **Zeroizing gap fix** — `src/sign.rs::secp256k1_sign` wraps `sk` byte slice
   in `Zeroizing` for function body scope and calls
   `zeroize::Zeroize::zeroize(&mut sk_buf)` before return. Risk Register #3.

See `CHANGELOG.md` (Task 0.7) for full patch log.

## Upstream tracking

Upstream is not pinned in git; the `.local/anychain` working copy under `.local/` serves
as the inspection mirror. Quarterly sync cadence per plan Q3 (revised 2026-09-06).