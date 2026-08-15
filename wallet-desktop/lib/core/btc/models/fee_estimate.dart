import 'package:meta/meta.dart';

import 'dto_parse_exception.dart';

@immutable
class FeeEstimate {
  const FeeEstimate({
    required this.fastestSatPerVb,
    required this.halfHourSatPerVb,
    required this.hourSatPerVb,
    required this.economySatPerVb,
    required this.minimumSatPerVb,
  });
  final int fastestSatPerVb;
  final int halfHourSatPerVb;
  final int hourSatPerVb;
  final int economySatPerVb;
  final int minimumSatPerVb;

  factory FeeEstimate.fromJson(Map<String, dynamic> j) {
    int at(String key) => dtoIntOpt(j, key) ?? 0;
    return FeeEstimate(
      fastestSatPerVb: at('1'),
      halfHourSatPerVb: at('3'),
      hourSatPerVb: at('6'),
      economySatPerVb: at('144'),
      minimumSatPerVb: at('1008'),
    );
  }
}
