import 'package:flutter/material.dart';

/// Backup prompt shown after a successful `btc wallet create` invocation.
///
/// **Threat-model surface:**
/// - `mnemonic` is held as a `String` (Dart immutable + interned — F47
///   caveat applies; v0.2 should route through FFI Uint8List +
///   `Finalizable`).
/// - Default-rendered state masks the mnemonic with U+2022 bullets
///   (`•`) so a screen-recording / over-the-shoulder observer without
///   explicit user action never sees the cleartext on screen. v0.2:
///   replace with a fixed-width placeholder so the bullet count does
///   not leak the mnemonic character length.
/// - The Reveal checkbox is required to flip the state to cleartext.
/// - The Done button is gated on the ack checkbox — the user must
///   explicitly affirm they wrote it down before the dialog closes.
///   `_everRevealed` is the cheap invariant guard so `_acked` is
///   unreachable without first seeing the cleartext (closes "user
///   acks without reading" loss path). v0.2: lift `Masked/Revealed/
///   Acked` into a sealed `BackupStage` enum.
/// - `barrierDismissible: false` (caller) + `PopScope(canPop: _acked)`
///   (this widget) close the tap-outside / Esc / Android-back paths
///   so the dialog cannot dismiss without the user explicitly
///   affirming.
/// - The mnemonic widget is wrapped in
///   `SelectionContainer.disabled(...)` (Flutter 3.3+) so the long-
///   press → Copy path is closed — the revealed mnemonic never
///   reaches the system clipboard. v0.1 UX is paper-and-pen.
/// - `ExcludeSemantics` strips the mnemonic `Text` subtree from the
///   semantics tree so TalkBack / VoiceOver / NVDA do NOT announce
///   the cleartext once revealed (the masked bullets still announce
///   as `[redacted]` for symmetry). NOTE: `BlockSemantics` would NOT
///   work here — it strips semantics of widgets painted BEFORE it,
///   not its descendants. This was the original implementation;
///   flutter's semantics contract made the screen-reader exfil still
///   open. Caught + fixed at L12 post-PR review.
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

  /// Monotonic flag: must be true before `_acked` is settable. Closes
  /// the "user acks without reading" loss path (L12 type-design
  /// post-PR finding). v0.2: sealed `BackupStage` enum replaces
  /// the bool pair at the type level.
  bool _everRevealed = false;

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final monoBody = theme.textTheme.bodyMedium
        ?.copyWith(fontFamily: 'monospace');
    final metaStyle = theme.textTheme.bodySmall
        ?.copyWith(fontFamily: 'monospace');

    final mnemonicNode = _visible
        ? Text(widget.mnemonic, style: monoBody)
        : Text(
            '•' * widget.mnemonic.length,
            style: monoBody,
            // v0.2 follow-up: fixed-width placeholder so the bullet
            // count does not leak the mnemonic char length (LOW).
            semanticsLabel: '[redacted backup phrase — tap Reveal]',
          );

    return PopScope(
      canPop: _acked,
      child: AlertDialog(
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
              // `SelectionContainer.disabled` closes the OS-clipboard
              // exfil path. `ExcludeSemantics` closes the screen-
              // reader exfil path. The two checkboxes below are
              // siblings in the same Column — they remain accessible
              // because they sit OUTSIDE the ExcludeSemantics node.
              SelectionContainer.disabled(
                child: ExcludeSemantics(
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
                onChanged: (v) => setState(() {
                  _visible = v ?? false;
                  if (_visible) _everRevealed = true;
                }),
                title: const Text('Reveal words'),
                controlAffinity: ListTileControlAffinity.leading,
              ),
              CheckboxListTile(
                value: _acked,
                // Disabled until the user has actually revealed the
                // words — `_everRevealed` is the cheap invariant
                // guard. (Enum elevation is the v0.2 type-level fix.)
                onChanged: _everRevealed
                    ? (v) => setState(() => _acked = v ?? false)
                    : null,
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
      ),
    );
  }
}
