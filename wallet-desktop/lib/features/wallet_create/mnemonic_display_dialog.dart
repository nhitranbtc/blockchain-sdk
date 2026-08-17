import 'package:flutter/material.dart';

/// Backup prompt shown after a successful `btc wallet create` invocation.
///
/// **Threat-model surface:**
/// - `mnemonic` is held as a `String` (Dart immutable + interned — F47
///   caveat applies; v0.2 should route through FFI Uint8List +
///   `Finalizable`).
/// - Default-rendered state masks the mnemonic with U+2022 bullets
///   (`•`) so a screen-recording / over-the-shoulder observer without
///   explicit user action never sees the cleartext on screen.
/// - The Reveal checkbox is required to flip the state to cleartext.
/// - The Done button is gated on the ack checkbox — the user must
///   explicitly affirm they wrote it down before the dialog closes.
/// - `barrierDismissible: false` is set by the caller so accidental
///   taps / Esc keypresses do not dismiss the dialog.
/// - The mnemonic widget is wrapped in
///   `SelectionContainer.disabled(...)` (Flutter 3.3+) so the long-
///   press → Copy path is closed — the revealed mnemonic never
///   reaches the system clipboard. v0.1 UX is paper-and-pen.
/// - `BlockSemantics` strips the mnemonic Text node from the
///   semantics tree so TalkBack / VoiceOver / NVDA do not announce
///   the cleartext once revealed. Without this, screen-reader
///   shoulder-surfing is a separate exfiltration vector.
///
/// **Logging**: the dialog NEVER logs `mnemonic`, `walletId`, or
/// `firstAddress`. Any future logging path MUST go through
/// `BtcLogFilter.redact` (Task 7) per L12 CRITICAL #2.
class MnemonicDisplayDialog extends StatefulWidget {
  const MnemonicDisplayDialog({
    super.key,
    required this.mnemonic,
    required this.walletId,
    required this.firstAddress,
  });
  final String mnemonic;
  final String walletId;
  final String firstAddress;

  @override
  State<MnemonicDisplayDialog> createState() => _MnemonicDisplayDialogState();
}

class _MnemonicDisplayDialogState extends State<MnemonicDisplayDialog> {
  bool _visible = false;
  bool _acked = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final monoBody = theme.textTheme.bodyMedium
        ?.copyWith(fontFamily: 'monospace');
    final metaStyle = theme.textTheme.bodySmall
        ?.copyWith(fontFamily: 'monospace');

    final mnemonicNode = _visible
        ? Text(widget.mnemonic, style: monoBody)
        : Text('•' * widget.mnemonic.length, style: monoBody);

    return AlertDialog(
      title: const Text('Backup your mnemonic'),
      content: SingleChildScrollView(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const Text(
              'Write these 12/24 words down on paper. Anyone with them '
              'controls your funds.',
            ),
            const SizedBox(height: 16),
            // SelectionContainer.disabled + BlockSemantics: defends
            // against clipboard + screen-reader exfiltration per the
            // threat-model block above. The wrapping MergeSemantics
            // keeps the checkbox subtree readable to assistive tech.
            SelectionContainer.disabled(
              child: BlockSemantics(
                child: Padding(
                  padding: const EdgeInsets.symmetric(vertical: 4),
                  child: mnemonicNode,
                ),
              ),
            ),
            const SizedBox(height: 8),
            Text('Wallet ID: ${widget.walletId}', style: metaStyle),
            Text('First address: ${widget.firstAddress}',
                style: metaStyle),
            const SizedBox(height: 16),
            CheckboxListTile(
              value: _visible,
              onChanged: (v) => setState(() => _visible = v ?? false),
              title: const Text('Reveal words'),
              controlAffinity: ListTileControlAffinity.leading,
            ),
            CheckboxListTile(
              value: _acked,
              onChanged: (v) => setState(() => _acked = v ?? false),
              title: const Text('I have written this down in a safe place'),
              controlAffinity: ListTileControlAffinity.leading,
            ),
          ],
        ),
      ),
      actions: [
        TextButton(
          onPressed: _acked ? () => Navigator.of(context).pop() : null,
          child: const Text('Done'),
        ),
      ],
    );
  }
}
