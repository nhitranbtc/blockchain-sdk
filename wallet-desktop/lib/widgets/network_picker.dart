import 'package:flutter/material.dart';

/// supportedNetwork for `btc` wallet operations. Listed in spec §2.
const _supportedNetworks = [
  'bitcoin',
  'testnet',
  'testnet4',
  'signet',
  'regtest',
];

/// Segmented network selector. Defaults to testnet (per L29: live
/// testnet is operator-driven, not CI; mainnet opt-in only via
/// Settings, not the default).
class NetworkPicker extends StatelessWidget {
  const NetworkPicker({
    super.key,
    required this.onChanged,
    this.initial = 'testnet',
  });
  final ValueChanged<String> onChanged;
  final String initial;

  @override
  Widget build(BuildContext context) {
    return SegmentedButton<String>(
      segments: _supportedNetworks
          .map((n) => ButtonSegment<String>(value: n, label: Text(n)))
          .toList(growable: false),
      selected: {initial},
      onSelectionChanged: (s) => onChanged(s.first),
    );
  }
}
