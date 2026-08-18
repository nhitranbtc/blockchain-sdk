#!/usr/bin/env bash
# Test wrapper that exports BTC_WALLET_MNEMONIC before exec'ing
# fake_btc.sh — exercises the L7 env-strip boundary at the FIXTURE
# level (Task 24, btc_invoker_test.dart env-strip test).
#
# **Why this exists**: BtcInvoker's own `_secretEnvKeys` filter runs
# in the Dart parent process against `Platform.environment`, which is
# UNMODIFIABLE in Dart 3+ (mutation throws UnsupportedError). So the
# BtcInvoker-level filter can only be exercised by launching `flutter
# test` with `BTC_WALLET_MNEMONIC=probe` in the shell — that's the
# L29 operator-driven gate (separate, documented in lessons.md).
#
# This wrapper instead exercises the FIXTURE's grep filter at the
# `--mnemonic` env-strip layer, which is the parallel defense at the
# fixture boundary. After wrapper exec, the fixture's inherited env
# MUST NOT contain `BTC_WALLET_MNEMONIC` (asserted in
# btc_invoker_test.dart).
set -eu
export BTC_WALLET_MNEMONIC="probe-secret-must-be-stripped-by-L7"
exec "$(cd "$(dirname "$0")" && pwd)/fake_btc.sh" "$@"