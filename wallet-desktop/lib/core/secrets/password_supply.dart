import 'temp_secret_file.dart';

/// Run [body] with a temp file containing [password].
///
/// Bridges the `btc` `--password-file` flag (btc-wallet Issue #84, mirrored
/// to wallet-desktop per design §6) — caller passes the resulting [path]
/// as the flag value. `btc` reads the file content as the password,
/// then unlinks it in its own handler layer.
///
/// This is a thin delegate to [withTempSecretFile] (Task 5) — the
/// separate function name documents intent (`password`, not generic
/// `secret`) and gives callers a stable API surface that can later grow
/// (e.g., wrap `--password-stdin` or environment-variable fallbacks)
/// without churning the generic `withTempSecretFile` signature.
///
/// **L12 CRITICAL #2**: never log [password]. Test fixtures use `'hunter2'`.
Future<void> withPasswordFile(
  String password,
  Future<void> Function(String path) body,
) =>
    withTempSecretFile(password, body);
