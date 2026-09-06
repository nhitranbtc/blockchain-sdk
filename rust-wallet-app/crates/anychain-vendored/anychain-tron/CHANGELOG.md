# CHANGELOG — anychain-tron (vendored)

Local patches layered on top of vendored copy. Upstream tag `v0.2.14` does not
exist on the upstream remote; vendored copy is sourced from
`0xcregis/anychain @ cf3aa2d` (see `SOURCE.md`).

## 2026-09-06 local patches

### Q13 — fee_limit varint encoding (REVERTED 2026-09-06)

- **What:** left unchanged — `Raw::write_to_with_cached_sizes` emits fee_limit
  via the standard `os.write_int64(18, self.fee_limit)?` call (6-byte wire
  format `90 01 80 c9 fe 3d` for value 130_000_000).
- **Why:** the plan's hypothesis that fee_limit's tag-18 varint emits a
  spurious `0x01` byte that TronGrid mis-parses (issue #540) was incorrect.
  Live-broadcast investigation on 2026-09-06 showed:
  - `/wallet/broadcasttransaction` + 6-byte fee_limit → NPE
  - `/wallet/broadcasttransaction` + 5-byte fee_limit (Q13 patch attempt) → NPE
  - `/wallet/broadcasthex` + 6-byte fee_limit → InvalidProtocolBufferException
  - `/wallet/broadcasthex` + 5-byte fee_limit → InvalidProtocolBufferException
  None of the fee_limit varint variations fix the wire-format issue.
- **Actual fix:** switched broadcast endpoint to `/wallet/broadcasthex` with
  full Transaction envelope (Tangem's approach). Fee_limit encoding here is
  left as standard protobuf varint (matches upstream) and round-trips
  cleanly through `TronTransaction::from_bytes` (round-trip guard in
  `tx::sign::sign_tx` is back in place).
- **Tracking:** real root cause of issue #540 was the wrong endpoint choice,
  not fee_limit varint encoding. CHANGELOG records the Q13 investigation
  result; issue #540 stays open for plan-amendment documentation but is
  effectively resolved by the endpoint switch.

### Q2 — REVERTED 2026-09-06

The Q2 "double-SHA-256" framing of issue #399 was wrong — same shape as
Q13 (varint), also a red herring. Live broadcast verification on
2026-09-06 confirmed `sha256(raw_bytes) == network_reported_txid` (single
SHA-256, matching upstream's `TronTransaction::to_transaction_id`
behaviour). The vendored method is dead code in our consumer (sign.rs
computes txid directly via `tx::sign::txid`); vendored reverted to
upstream single-SHA-256 to keep quarterly sync diffs (Risk #11) clean.

### Risk #3 — Zeroizing sk buffer (plan Risk Register)

Cross-crate patch; documented in `rust-wallet-app/crates/anychain-vendored/anychain-kms/CHANGELOG.md`.
Not duplicated here to keep the quarterly sync audit (Risk #11) clean — a
future re-vendor of `anychain-tron` against upstream `main` would diff this
entry as "local change to be reviewed" against an upstream that never had it.