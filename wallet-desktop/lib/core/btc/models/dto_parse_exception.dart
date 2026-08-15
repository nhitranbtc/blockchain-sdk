import 'package:meta/meta.dart';

/// Thrown when `btc` CLI `--json` output cannot be parsed into a DTO.
///
/// Carries the offending JSON path (e.g. `wallet_detail.balance.confirmed_sat`)
/// and the raw value for diagnostics. Replaces the uncaught `TypeError`
/// that `as String` / `as num` casts would otherwise propagate.
@immutable
class DtoParseException implements Exception {
  const DtoParseException(this.path, [this.value]);

  final String path;
  final Object? value;

  @override
  String toString() => 'DtoParseException($path: $value)';
}

/// Helper for required `String` fields. Throws [DtoParseException] with
/// field context if the value is missing or not a `String`.
String dtoString(Map<String, dynamic> j, String path) {
  final v = j[path];
  if (v is! String) {
    throw DtoParseException(path, v);
  }
  return v;
}

/// Helper for required `int` fields (accepts `int` or `double`). Throws
/// [DtoParseException] if missing or not a `num`.
int dtoInt(Map<String, dynamic> j, String path) {
  final v = j[path];
  if (v is! num) {
    throw DtoParseException(path, v);
  }
  return v.toInt();
}

/// Helper for optional `int` fields. Returns `null` if missing; throws
/// [DtoParseException] if present but not a `num`.
int? dtoIntOpt(Map<String, dynamic> j, String path) {
  final v = j[path];
  if (v == null) return null;
  if (v is! num) {
    throw DtoParseException(path, v);
  }
  return v.toInt();
}
