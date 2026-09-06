# SOURCE — anychain-core (vendored)

**Source repo:** https://github.com/0xcregis/anychain
**Pinned commit:** `cf3aa2d59afb2c50dc961919fca011c401238ed6`
**Commit subject:** `build: rustup toolchain update to 1.98.0 (#465)`
**Commit date:** 2026-09-04 15:53:11 +0800
**Vendored on:** 2026-09-06
**Vendored by:** tron-wallet-core v0.1 Phase 0 (plan Task 0.2)

## Pin deviation from plan

Plan reference (`docs/superpowers/plans/2026-09-05-tron-wallet-core-v0.1-anychain.md`)
cites upstream tag `v0.1.8` for `anychain-core`. The tag **does not exist** on
the upstream remote (`git ls-remote --tags https://github.com/0xcregis/anychain.git`
returned only `0.0.2` and `0.1.5` as of 2026-09-06). Upstream was sourced at
HEAD of `main` instead — commit SHA recorded above.

Plan amendment deferred (post-Phase 0).

## Source path

Files copied from `.local/anychain/crates/anychain-core/src/**` at commit `cf3aa2d`
(repo at repo root, kept as read-only reference for inspection; not used by
consumer builds).

## License

Per-file `SPDX-License-Identifier: MIT OR Apache-2.0` preserved on every
vendored source file.

## Local patches

None at vendoring time. Future patches (Q2 dual-SHA256 txid, Q13 varint,
Zeroizing gap) layered on top per `CHANGELOG.md` (Task 0.7).

## Upstream tracking

Upstream is not pinned in git; the `.local/anychain` working copy under `.local/` serves
as the inspection mirror. Quarterly sync cadence per plan Q3 (revised 2026-09-06):
fetch `.local/anychain` from `origin`, manually diff against `crates/anychain-vendored/`,
gate on (a) no breaking wire-format change, (b) regression tests PASS, (c) manual
review.