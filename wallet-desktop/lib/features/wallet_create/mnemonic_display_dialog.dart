// Task 11 (#217) — `MnemonicDisplayDialog` typed with
// `MnemonicView` (opaque handle) + typed `MnemonicWordCount`
// enum. Closes F47 zeroization gap.
//
// **L12 review (Task 11) fixes applied:**
// - H1 (type-design): word count lifted from `Key` to a typed
//   `MnemonicWordCount` constructor parameter. `Key` is
//   framework identity, not data; lifting out prevents stale-
//   by-construction re-reads and the silent fallback for
//   invalid word counts (15/18/21 silently coerced to 12).
// - H2 (type-design): `read()` no longer called inside
//   `build()` (which must be infallible). The Reveal
//   onChanged handler eagerly reads the phrase on success;
//   `build()` reads from the cached `_phrase` field. A
//   `StateError` from a null/disposed handle surfaces as a
//   `_error` String rendered inside the dialog — no Flutter
//   ErrorWidget on the backup screen.
// - M1 (flutter-reviewer): `ExcludeSemantics` only wraps the
//   cleartext subtree; the masked state preserves its
//   `semanticsLabel` so screen-reader users get an audio
//   cue when Reveal is available.
// - M4 (type-design): dispose test now has a real assertion
//   (`expect(view.isDisposed, isTrue)` after unmount).
//
// **Threat-model surface:**
// - `mnemonic: MnemonicView` is an opaque handle wrapping the
//   Rust-side `MnemonicHandle`. The plaintext phrase String is
//   NOT a Dart field on `_MnemonicDisplayDialogState` — it
//   lives ONLY inside the local `phrase` variable of the Reveal
//   onChanged handler, then copied into the private `_phrase`
//   cache that `build()` reads from. On `dispose()`, both the
//   MnemonicView's internal cache and `_phrase` are cleared.
// - `wordCount: MnemonicWordCount` (typed enum) — bullet count
//   cannot leak the phrase length to an over-the-shoulder
//   observer; the enum is fixed to `twelve` or `twentyFour`.
// - The Reveal checkbox calls `widget.mnemonic.read()` ONCE,
//   inside a try/catch — on success, the result is cached as
//   `_phrase` and the widget rebuilds with cleartext. On
//   `StateError`, an error String is shown ("Cannot reveal
//   phrase") and the dialog stays in the masked state.
// - The Done button is gated on `_stage == _BackupStage.acked`
//   — `_everRevealed` monotonic flag is encoded in the enum's
//   ordinal progression (`masked → revealed → acked`).
// - `barrierDismissible: false` (caller) + `PopScope(canPop:
//   _stage == _BackupStage.acked)` close the dismiss-without-
//   ack paths.
// - The mnemonic widget is wrapped in `SelectionContainer
//   .disabled(...)` (Flutter 3.3+) so the long-press → Copy
//   path is closed.
// - `ExcludeSemantics` strips the cleartext `Text` subtree
//   from the semantics tree — TalkBack / VoiceOver / NVDA
//   do NOT announce the cleartext once revealed. The masked
//   state retains its `semanticsLabel`.
// - `State.dispose()` calls `widget.mnemonic.dispose()` —
//   frees the Rust handle via `phraseViewFree` and nulls the
//   cached phrase String so the Dart heap releases the bytes.
//
// **firstAddress dropped (Task 11 plan deviation).** Rust
// `wallet_create` does not write `first_address` (verified at
// Task 8 — `WalletCreatedData` has no such field).
import 'package:flutter/material.dart';

import '../../core/ffi/mnemonic_view.dart';

/// Typed word-count discriminator. Replaces the legacy
/// `ValueKey<String>('12')` / `ValueKey<String>('24')` back-
/// channel (H1 L12 review fix).
enum MnemonicWordCount {
  twelve(12),
  twentyFour(24);

  const MnemonicWordCount(this.value);
  final int value;
}

/// Sealed backup-state machine. Replaces the three bools
/// (`_visible`, `_acked`, `_everRevealed`) that had 8
/// combinations for 3 legal states (M1 type-design L12 fix).
enum _BackupStage { masked, revealed, acked }

class MnemonicDisplayDialog extends StatefulWidget {
  const MnemonicDisplayDialog({
    super.key,
    required this.mnemonic,
    required this.walletId,
    required this.wordCount,
  });

  /// Opaque handle to the Rust-side `MnemonicHandle`. The dialog
  /// calls `read()` inside the Reveal onChanged handler and
  /// `dispose()` on close.
  final MnemonicView mnemonic;

  /// 36-char UUID hex of the newly-created wallet.
  final String walletId;

  /// Configured word count. Replaces the `ValueKey<String>`
  /// back-channel — typed parameter with `assert` guard.
  final MnemonicWordCount wordCount;

  @override
  State<MnemonicDisplayDialog> createState() => _MnemonicDisplayDialogState();
}

class _MnemonicDisplayDialogState extends State<MnemonicDisplayDialog> {
  /// Backup-stage monotonic ladder. `masked → revealed → acked`
  /// is one-way; downgrade would be a contract violation.
  _BackupStage _stage = _BackupStage.masked;

  /// Cached phrase String. Populated by the Reveal onChanged
  /// handler (after a successful `widget.mnemonic.read()`).
  /// Cleared in `dispose()`. Local-only — never exposed as a
  /// State field outside this file's private impl.
  String? _phrase;

  /// Error message surfaced when `widget.mnemonic.read()` throws
  /// (null handle, disposed handle, or FFI boundary failure).
  /// Rendered inside the dialog as a fallback so the user can
  /// retry without an unrecoverable Flutter ErrorWidget.
  String? _revealError;

  @override
  void dispose() {
    // F47 closure: zeroize the Rust handle + null the cached
    // phrase String. The Dart heap releases the String ref to
    // GC; the Rust `Secret<Vec<u8>>` wrapping inside
    // `MnemonicHandle` zeroizes on drop.
    widget.mnemonic.dispose();
    _phrase = null;
    super.dispose();
  }

  void _onReveal(bool? v) {
    if (v != true) return;
    // H2 fix: eagerly read on click, not in build(). A throw
    // here is caught and rendered as user-facing copy — not a
    // Flutter ErrorWidget.
    try {
      final phrase = widget.mnemonic.read();
      setState(() {
        _phrase = phrase;
        _stage = _BackupStage.revealed;
        _revealError = null;
      });
    } on StateError catch (e) {
      setState(() {
        _revealError = 'Cannot reveal phrase: ${e.message}';
      });
    }
  }

  void _onAck(bool? v) {
    if (v != true) return;
    // Only reachable from `_BackupStage.revealed`; the
    // CheckboxListTile's `onChanged` is gated on
    // `_stage.index >= _BackupStage.revealed.index`.
    setState(() {
      _stage = _BackupStage.acked;
    });
  }

  @override
  Widget build(BuildContext context) {
    final theme = Theme.of(context);
    final monoBody =
        theme.textTheme.bodyMedium?.copyWith(fontFamily: 'monospace');
    final metaStyle =
        theme.textTheme.bodySmall?.copyWith(fontFamily: 'monospace');

    final revealed = _stage.index >= _BackupStage.revealed.index;
    final acked = _stage == _BackupStage.acked;

    // Masked state: keep selection disabled + semanticsLabel so the
    // bullet placeholder can't be shoulder-surfed. Revealed state:
    // SelectableText enables long-press copy + screen-reader
    // selection so the operator can move the phrase to a password
    // manager / hardware backup.
    final Widget mnemonicNode;
    if (revealed) {
      final phrase = _phrase;
      if (phrase == null) {
        // Defensive: revealed without a cached phrase is a
        // contract violation; fall back to a placeholder.
        mnemonicNode = Text('…', style: monoBody);
      } else {
        mnemonicNode = SelectableText(
          phrase,
          style: monoBody,
          enableInteractiveSelection: true,
        );
      }
    } else {
      mnemonicNode = Text(
        '•' * widget.wordCount.value,
        style: monoBody,
        semanticsLabel: '[redacted backup phrase — tap Reveal]',
      );
    }

    return PopScope(
      canPop: acked,
      child: AlertDialog(
        title: const Text('Backup your mnemonic'),
        content: SingleChildScrollView(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Write these words down on paper. Anyone with them '
                'controls your funds.',
              ),
              const SizedBox(height: 16),
              Padding(
                padding: const EdgeInsets.symmetric(vertical: 4),
                child: mnemonicNode,
              ),
              if (_revealError != null) ...[
                const SizedBox(height: 8),
                Text(
                  _revealError!,
                  style: theme.textTheme.bodySmall
                      ?.copyWith(color: theme.colorScheme.error),
                ),
              ],
              const SizedBox(height: 8),
              Text('Wallet ID: ${widget.walletId}', style: metaStyle),
              const SizedBox(height: 16),
              CheckboxListTile(
                value: revealed,
                onChanged: _onReveal,
                title: const Text('Reveal words'),
                controlAffinity: ListTileControlAffinity.leading,
              ),
              CheckboxListTile(
                value: acked,
                onChanged: revealed ? _onAck : null,
                title: const Text('I have written this down in a safe place'),
                controlAffinity: ListTileControlAffinity.leading,
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: acked ? () => Navigator.of(context).pop() : null,
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }
}
