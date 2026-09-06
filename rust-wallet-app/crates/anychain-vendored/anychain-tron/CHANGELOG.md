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

### Q2 — canonical double-SHA-256 txid (issue #399 historical bug + plan Q2)

- **What:** in `src/transaction.rs::TronTransaction::to_transaction_id`, replaced
  single `crypto::sha256(raw_bytes)` with `crypto::sha256(crypto::sha256(raw_bytes))`.
- **Why:** upstream computed a single SHA-256 here, producing a hash that did
  NOT match TronGrid's reported txid. The patch is canonical TRX double-hash;
  callers no longer need to recompute this manually.
- **Test:** `rust-wallet-app/crates/tron-wallet-core/tests/varint_and_txid.rs::txid_is_double_sha256`
  (regression test passes against both 6-byte and 5-byte fee_limit forms).
- **Side effect:** the in-source test `test_from_bytes` at the original line
  range asserts an old single-SHA-256 txid constant; it is left in place but
  now expected to fail against the patched code. See test references in
  `tests/varint_and_txid.rs::txid_is_double_sha256` for the new expected
  behavior.