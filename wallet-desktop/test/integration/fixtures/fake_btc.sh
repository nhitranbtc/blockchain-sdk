#!/usr/bin/env bash
# Fake `btc` binary for integration tests (Task 24, issue #172).
#
# Inspects argv, emits canned JSON to stdout (success) or stderr (failure),
# exits with codes per spec §4.3 mapping:
#   0 = success
#   1 = unknown command / unparseable args
#   2 = wrong password (used by `wallet delete` smoke test)
#
# SECURITY (L12 CRITICAL #2):
#   - The fixture NEVER prints the value of `--mnemonic` (or anything else
#     secret-bearing: `--password`, `--password-file` contents,
#     `BTC_WALLET_MNEMONIC` env). It is consumed only for argv-shape
#     dispatch — every case branch emits a canned JSON literal.
#   - Inherited env is written to a sibling `.last_env` file with
#     secret-bearing keys (BTC_WALLET_MNEMONIC, BTC_ENCRYPT_PASSWORD,
#     BTC_DECRYPT_PASSWORD) FILTERED OUT, so the L7 env-strip test can
#     verify `BtcInvoker` is not re-leaking the parent's secrets.
#   - stdout is JSON only; no `set -x` / `echo $@` debugging.
#
# OPERATOR SCOPE (L29):
#   This is a HERMETIC fixture for Dart-side BtcInvoker integration
#   tests. It does NOT touch real Bitcoin / Esplora / testnet. Live
#   testnet smoke is the L29 operator-run gate, not this script.
set -u

# Resolve the script dir so `.last_env` lives next to the script
# regardless of where the test is invoked from.
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"

# 1) Env-write: filtered env, secret keys stripped. `|| true` swallows
#    grep's "no matches" exit. Enable `set -e` ONLY for this block so
#    a redirect failure (read-only fs, permission denied) surfaces,
#    then restore `set +e` for the rest of the script (defense in
#    depth per L12 silent-failure-hunter).
set -e
{
  env \
    | grep -v -E '^(BTC_WALLET_MNEMONIC|BTC_ENCRYPT_PASSWORD|BTC_DECRYPT_PASSWORD)=' \
    || true
} > "$SCRIPT_DIR/.last_env"
set +e

# 2) Dispatch: positional $1 is the subcommand family, $2 the action
#    (where applicable). Flag values (--mnemonic, --password-file, ...)
#    are NEVER echoed; we only check their PRESENCE via $@ iteration.
case "${1:-}" in
  --version)
    echo "btc 0.1.0 (fake)"
    exit 0
    ;;
  config)
    # config show --json: emitted shape is informational only; the
    # existing env-strip test calls parse: (_) => null on it.
    echo '{"data_dir":"/tmp/btc","network":"testnet","esplora_url":"https://blockstream.info/testnet/api","wallets":["fake-uuid-1","fake-uuid-2"]}'
    exit 0
    ;;
  fee-estimates)
    # Esplora fee-estimates JSON shape: bucket-by-block-target keys
    # ('1', '3', '6', '144', '1008'). FeeEstimate.fromJson expects ints.
    echo '{"1":5,"3":4,"6":3,"144":2,"1008":1}'
    exit 0
    ;;
  tx-list)
    # Single canned tx; TxRecord.fromJson parses
    # {txid, direction, amount_sat, fee_sat, confirmations, timestamp}.
    echo '[{"txid":"faketxid","direction":"outgoing","amount_sat":1000,"fee_sat":250,"confirmations":3,"timestamp":1700000000}]'
    exit 0
    ;;
  wallet)
    case "${2:-}" in
      list)
        # WalletInfo shape: {id, network, address_type}.
        echo '[{"id":"fake-uuid-1","network":"testnet","address_type":"native-segwit"}]'
        exit 0
        ;;
      show)
        # WalletDetail shape: {id, network, address_type, first_address,
        # balance:{confirmed_sat, trusted_pending_sat, untrusted_pending_sat,
        # immature_sat}, utxos:[]}. No mnemonic / password echoes.
        echo '{"id":"fake-uuid-1","network":"testnet","address_type":"native-segwit","first_address":"tb1qfake","balance":{"confirmed_sat":12345,"trusted_pending_sat":0,"untrusted_pending_sat":0,"immature_sat":0},"utxos":[]}'
        exit 0
        ;;
      create)
        # WalletCreated shape: {id, mnemonic, first_address, network,
        # address_type}. The mnemonic field is a fixed well-known
        # 12-word phrase for shape coverage; the UI's OpaqueMnemonic
        # wrapper (Task 14) is the real isolation. Real fixtures MUST
        # NOT inject user material here.
        echo '{"id":"fake-uuid-1","mnemonic":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about","first_address":"tb1qfake","network":"testnet","address_type":"native-segwit"}'
        exit 0
        ;;
      import)
        # Same shape as create (import returns a new wallet_id + the
        # same canonical mnemonic phrase). Real callers already have
        # the mnemonic they imported; this fixture just confirms the
        # DTO parse contract.
        echo '{"id":"fake-uuid-2","mnemonic":"abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about","first_address":"tb1qfake","network":"testnet","address_type":"native-segwit"}'
        exit 0
        ;;
      delete)
        # Exit 2 — used by btc_invoker_test.dart to assert BtcError
        # surfaces on non-zero exit. The error message is benign.
        echo "error: wrong password" >&2
        exit 2
        ;;
      sync)
        # Stateless sync — `btc` prints a JSON object with utxo/total;
        # the UI calls WalletShow instead. Return a shape compatible
        # with both `{utxo_count, total_sat}` (Story 11) and the
        # legacy `balance_sat` field.
        echo '{"utxo_count":0,"total_sat":0}'
        exit 0
        ;;
      balance)
        # Stateless balance: integer sats on a single line (real btc
        # prints raw sats; UI calls WalletShow for full detail).
        echo "0"
        exit 0
        ;;
      send)
        # SendResult shape: {txid, fee_sat, vbytes}. Real btc
        # broadcasts and returns txid; fake returns a faketxid prefix.
        echo '{"txid":"faketxid0000000000000000000000000000000000000000000000000000","fee_sat":250,"vbytes":140}'
        exit 0
        ;;
      bump-fee)
        # RBF bump-fee returns the new txid (same shape as send).
        echo '{"txid":"faketxid0000000000000000000000000000000000000000000000000000","fee_sat":500,"vbytes":140}'
        exit 0
        ;;
      *)
        # Defense-in-depth default (MUST be last — bash case picks first
        # match; a misplaced `*)` catches everything before specific
        # branches run). Unknown `wallet` sub-action exits non-zero
        # instead of falling through silently. Mirrors the outer-case
        # fallthrough at line ~163.
        echo "error: unknown wallet subcommand: ${2:-<empty>}" >&2
        exit 1
        ;;
    esac
    ;;
  encrypt)
    # encrypt --in X --out Y — emits binary blob; the UI does not
    # currently exercise this path. Exit 0 / empty stdout.
    exit 0
    ;;
  decrypt)
    # decrypt --in X --out Y — emits UTF-8 plaintext to file; UI does
    # not exercise this. Exit 0 / empty stdout.
    exit 0
    ;;
  message)
    # message sign|verify — UI does not exercise. Sign returns a
    # canned base64 stub; verify returns exit 0 ("valid").
    case "${2:-}" in
      sign)
        echo "fakeb64sig=="
        exit 0
        ;;
      verify)
        exit 0
        ;;
      *)
        echo "error: unknown message subcommand: ${2:-<empty>}" >&2
        exit 1
        ;;
    esac
    ;;
esac

# 3) Fallthrough — every other argv shape is unknown. Exit 1 so the
#    BtcError surface in BtcInvoker surfaces the raw stderr verbatim
#    (BtcLogFilter is the chokepoint at the UI/log layer).
echo "error: unknown command: ${1:-<empty>} ${2:-}" >&2
exit 1