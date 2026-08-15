import 'package:meta/meta.dart';

/// Sealed hierarchy of `btc` CLI subcommands with their argv builders.
///
/// Each subclass represents one `btc` subcommand family (e.g. `wallet
/// list`, `wallet show`, `tx-list`). `argv` returns the ready-to-pass
/// list of arguments for `Process.start(btc, argv)` (Task 10 BtcInvoker).
///
/// `sealed` (Dart 3) — callers must exhaustively match on subclasses when
/// switching on a `BtcCommand`. Mirrors `BtcException`/`SecretException`
/// ADT patterns from Tasks 4 + 5.
///
/// **Conventions**:
/// - Optional flags (e.g. `--dry-run`, `--confirm-yes`) emit their flag
///   and value only when the constructor param is non-null / non-false.
/// - `--password-file` is REQUIRED for any wallet command that touches
///   an encrypted wallet — Task 6 `withPasswordFile` produces the path.
/// - `--esplora-url` + `--pin-spki` are required for any command that
///   calls Esplora (F20 SPKI pin, F36 HTTPS-only).
///
/// **Note on `case final` patterns**: Dart 3's collection-if supports
/// `if (field case final x?)` to capture-and-narrow nullable fields,
/// but `x` is only in scope of the immediately-following element —
/// not across the `,` separator that follows in spread-style collections.
/// We use local-variable capture (`final x = nullableField;` then
/// `if (x != null) …, if (x != null) x,`) which works for fields.
@immutable
sealed class BtcCommand {
  const BtcCommand();

  List<String> get argv;

  // Static named factories (Dart 3). Extension static methods
  // (`extension BtcCommandStatic on BtcCommand`) can't be called via the
  // class name; Dart 3 requires explicit extension name. Plan §Task 8
  // used extension static — corrected to class static here.

  static WalletList walletList({required String network}) =>
      WalletList(network: network);

  static WalletShow walletShow({
    required String id,
    required String network,
    required String passwordFilePath,
    String? esploraUrl,
    String? esploraSpkiPin,
  }) =>
      WalletShow(
        id: id,
        network: network,
        passwordFilePath: passwordFilePath,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
      );

  static WalletSend walletSend({
    required String mnemonic,
    required String network,
    String? to,
    String? address,
    int? amountSat,
    required int feeRateSatPerVb,
    required String passwordFilePath,
    required String esploraUrl,
    required String esploraSpkiPin,
    String? confirmYes,
    bool dryRun = false,
  }) =>
      WalletSend(
        mnemonic: mnemonic,
        network: network,
        to: to,
        address: address,
        amountSat: amountSat,
        feeRateSatPerVb: feeRateSatPerVb,
        passwordFilePath: passwordFilePath,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
        confirmYes: confirmYes,
        dryRun: dryRun,
      );

  static WalletCreate walletCreate({
    required int words,
    required String network,
    required String addressType,
    required String passwordFilePath,
    String? confirmYes,
  }) =>
      WalletCreate(
        words: words,
        network: network,
        addressType: addressType,
        passwordFilePath: passwordFilePath,
        confirmYes: confirmYes,
      );

  static WalletImport walletImport({
    required String mnemonic,
    required String network,
    required String passwordFilePath,
  }) =>
      WalletImport(
        mnemonic: mnemonic,
        network: network,
        passwordFilePath: passwordFilePath,
      );

  static TxList txList({
    required String mnemonic,
    required String network,
    required String esploraUrl,
    required String esploraSpkiPin,
    int? limit,
  }) =>
      TxList(
        mnemonic: mnemonic,
        network: network,
        esploraUrl: esploraUrl,
        esploraSpkiPin: esploraSpkiPin,
        limit: limit,
      );
}

class WalletList extends BtcCommand {
  const WalletList({required this.network});
  final String network;
  @override
  List<String> get argv => ['wallet', 'list', '--network', network, '--json'];
}

class WalletShow extends BtcCommand {
  const WalletShow({
    required this.id,
    required this.network,
    required this.passwordFilePath,
    this.esploraUrl,
    this.esploraSpkiPin,
  });
  final String id;
  final String network;
  final String passwordFilePath;
  final String? esploraUrl;
  final String? esploraSpkiPin;

  @override
  List<String> get argv {
    final eu = esploraUrl;
    final ep = esploraSpkiPin;
    return [
      'wallet',
      'show',
      id,
      '--network',
      network,
      '--password-file',
      passwordFilePath,
      if (eu != null) '--esplora-url',
      if (eu != null) eu,
      if (ep != null) '--pin-spki',
      if (ep != null) ep,
    ];
  }
}

class WalletDelete extends BtcCommand {
  const WalletDelete({required this.id, required this.network});
  final String id;
  final String network;
  @override
  List<String> get argv => ['wallet', 'delete', id, '--network', network];
}

class WalletRename extends BtcCommand {
  const WalletRename(
      {required this.id, required this.to, required this.network});
  final String id;
  final String to;
  final String network;
  @override
  List<String> get argv =>
      ['wallet', 'rename', '--id', id, '--to', to, '--network', network];
}

class WalletCreate extends BtcCommand {
  const WalletCreate({
    required this.words,
    required this.network,
    required this.addressType,
    required this.passwordFilePath,
    this.confirmYes,
  });
  final int words;
  final String network;
  final String addressType;
  final String passwordFilePath;
  final String? confirmYes;

  @override
  List<String> get argv {
    final cy = confirmYes;
    return [
      'wallet',
      'create',
      '--words',
      '$words',
      '--network',
      network,
      '--type',
      addressType,
      '--password-file',
      passwordFilePath,
      if (cy != null) '--confirm-yes',
      if (cy != null) cy,
    ];
  }
}

class WalletImport extends BtcCommand {
  const WalletImport({
    required this.mnemonic,
    required this.network,
    required this.passwordFilePath,
  });
  final String mnemonic;
  final String network;
  final String passwordFilePath;
  @override
  List<String> get argv => [
        'wallet',
        'import',
        '--mnemonic',
        mnemonic,
        '--network',
        network,
        '--password-file',
        passwordFilePath,
      ];
}

class WalletSend extends BtcCommand {
  const WalletSend({
    required this.mnemonic,
    required this.network,
    this.to,
    this.address,
    this.amountSat,
    required this.feeRateSatPerVb,
    required this.passwordFilePath,
    required this.esploraUrl,
    required this.esploraSpkiPin,
    this.confirmYes,
    this.dryRun = false,
  });
  final String mnemonic;
  final String network;
  final String? to; // multi-recipient form (single OK too)
  final String? address; // single-recipient form (deprecated, use `to`)
  final int? amountSat;
  final int feeRateSatPerVb;
  final String passwordFilePath;
  final String esploraUrl;
  final String esploraSpkiPin;
  final String? confirmYes;
  final bool dryRun;

  @override
  List<String> get argv {
    final t = to;
    final a = address;
    final amt = amountSat;
    final cy = confirmYes;
    return [
      'wallet',
      'send',
      '--mnemonic',
      mnemonic,
      '--network',
      network,
      if (t != null) '--to',
      if (t != null) t,
      if (a != null) '--address',
      if (a != null) a,
      if (amt != null) '--amount-sat',
      if (amt != null) '$amt',
      '--fee-rate',
      '$feeRateSatPerVb',
      if (dryRun) '--dry-run',
      if (cy != null) '--confirm-yes',
      if (cy != null) cy,
      '--password-file',
      passwordFilePath,
      '--esplora-url',
      esploraUrl,
      '--pin-spki',
      esploraSpkiPin,
    ];
  }
}

class TxList extends BtcCommand {
  const TxList({
    required this.mnemonic,
    required this.network,
    required this.esploraUrl,
    required this.esploraSpkiPin,
    this.limit,
  });
  final String mnemonic;
  final String network;
  final String esploraUrl;
  final String esploraSpkiPin;
  final int? limit;

  @override
  List<String> get argv {
    final l = limit;
    return [
      'tx-list',
      '--mnemonic',
      mnemonic,
      '--network',
      network,
      '--esplora-url',
      esploraUrl,
      '--pin-spki',
      esploraSpkiPin,
      if (l != null) '--limit',
      if (l != null) '$l',
      '--json',
    ];
  }
}

class FeeEstimates extends BtcCommand {
  const FeeEstimates({
    required this.network,
    required this.esploraUrl,
    required this.esploraSpkiPin,
  });
  final String network;
  final String esploraUrl;
  final String esploraSpkiPin;

  @override
  List<String> get argv => [
        'fee-estimates',
        '--network',
        network,
        '--esplora-url',
        esploraUrl,
        '--pin-spki',
        esploraSpkiPin,
        '--json',
      ];
}

class ConfigShow extends BtcCommand {
  const ConfigShow();
  @override
  List<String> get argv => ['config', 'show', '--json'];
}
