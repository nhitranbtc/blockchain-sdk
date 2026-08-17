import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

/// Compact display of a Bitcoin address. Tap to copy full address to
/// clipboard + show snackbar confirmation.
///
/// Threat-model: address is not secret (it's a public key on the
/// blockchain), so copy-to-clipboard is safe. The mnemonic / private
/// key never appears in this widget.
class AddressChip extends StatelessWidget {
  const AddressChip({super.key, required this.address, this.network});
  final String address;
  final String? network;

  @override
  Widget build(BuildContext context) {
    final short = address.length <= 12
        ? address
        : '${address.substring(0, 8)}…${address.substring(address.length - 4)}';
    return InkWell(
      onTap: () async {
        await Clipboard.setData(ClipboardData(text: address));
        if (context.mounted) {
          ScaffoldMessenger.of(context).showSnackBar(
            const SnackBar(content: Text('Copied')),
          );
        }
      },
      child: Chip(
        avatar: network == null
            ? null
            : CircleAvatar(child: Text(network!.substring(0, 1))),
        label: Text(short, style: const TextStyle(fontFamily: 'monospace')),
      ),
    );
  }
}
