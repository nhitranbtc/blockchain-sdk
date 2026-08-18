import 'package:flutter/material.dart';

/// Mnemonic paste area with live word-count validation + required
/// acknowledgment checkbox before the parent can proceed (used in
/// Task 19 WalletImportScreen, Task 21 SendScreen sentinel-clear
/// re-entry path).
///
/// Security: `TextEditingController.clear()` on dispose (defense-in-depth,
/// not real zeroization — same F47 caveat as PasswordField).
///
/// **Submit gating**: the `onSubmit` callback (Task 21) fires only when
/// the field has the expected word count AND the ack checkbox is
/// checked. Parents can wire a single "Continue" button to
/// `fieldKey.currentState?.submit()` and receive the validated mnemonic
/// without holding an intermediate screen-side cache. The parent's
/// own submit button calls `submit()` which validates internally and
/// either invokes `onSubmit(text)` or surfaces no-op (validation fails
/// → user must correct before proceeding).
class MnemonicPasteField extends StatefulWidget {
  const MnemonicPasteField({
    super.key,
    required this.onChanged,
    required this.expectedWordCount,
    this.onSubmit,
  }) : assert(
          expectedWordCount >= 12 && expectedWordCount % 3 == 0,
          'BIP-39 word counts are 12/15/18/21/24',
        );
  final ValueChanged<String> onChanged;
  final int expectedWordCount;
  final ValueChanged<String>? onSubmit;

  @override
  State<MnemonicPasteField> createState() => MnemonicPasteFieldState();
}

class MnemonicPasteFieldState extends State<MnemonicPasteField> {
  late final TextEditingController _controller;
  bool _ackChecked = false;

  @override
  void initState() {
    super.initState();
    _controller = TextEditingController();
  }

  @override
  void dispose() {
    _controller.clear();
    _controller.dispose();
    super.dispose();
  }

  int get _wordCount => _controller.text
      .trim()
      .split(RegExp(r'\s+'))
      .where((w) => w.isNotEmpty)
      .length;

  bool get _isValid => _wordCount == widget.expectedWordCount && _ackChecked;

  /// Public submit hook (Task 21). Returns the validated mnemonic to
  /// `widget.onSubmit` iff the field is valid; otherwise no-op so the
  /// parent's button can re-enable only on valid input. The callback
  /// fires synchronously inside the build cycle; the parent's
  /// notifier.unlock call runs synchronously (Lesson 32.2 identity
  /// capture is the parent's responsibility).
  void submit() {
    if (!_isValid) return;
    final cb = widget.onSubmit;
    if (cb == null) return;
    cb(_controller.text);
  }

  @override
  Widget build(BuildContext context) {
    final valid = _wordCount == widget.expectedWordCount;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        TextField(
          controller: _controller,
          minLines: 3,
          maxLines: 5,
          autocorrect: false,
          enableSuggestions: false,
          onChanged: (v) {
            setState(() {});
            widget.onChanged(v);
          },
          decoration: InputDecoration(
            labelText: 'Mnemonic (paste only — do not type)',
            border: const OutlineInputBorder(),
            errorText: valid || _controller.text.isEmpty
                ? null
                : 'Expected ${widget.expectedWordCount} words; got $_wordCount',
          ),
        ),
        const SizedBox(height: 8),
        CheckboxListTile(
          value: _ackChecked,
          onChanged: (v) => setState(() => _ackChecked = v ?? false),
          title: const Text('I have written this down in a safe place'),
          controlAffinity: ListTileControlAffinity.leading,
        ),
      ],
    );
  }
}
