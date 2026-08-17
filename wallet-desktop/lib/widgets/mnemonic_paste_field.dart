import 'package:flutter/material.dart';

/// Mnemonic paste area with live word-count validation + required
/// acknowledgment checkbox before the parent can proceed (used in
/// Task 19 WalletImportScreen).
///
/// Security: `TextEditingController.clear()` on dispose (defense-in-depth,
/// not real zeroization — same F47 caveat as PasswordField).
class MnemonicPasteField extends StatefulWidget {
  const MnemonicPasteField({
    super.key,
    required this.onChanged,
    required this.expectedWordCount,
  }) : assert(
          expectedWordCount >= 12 && expectedWordCount % 3 == 0,
          'BIP-39 word counts are 12/15/18/21/24',
        );
  final ValueChanged<String> onChanged;
  final int expectedWordCount;

  @override
  State<MnemonicPasteField> createState() => _MnemonicPasteFieldState();
}

class _MnemonicPasteFieldState extends State<MnemonicPasteField> {
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
