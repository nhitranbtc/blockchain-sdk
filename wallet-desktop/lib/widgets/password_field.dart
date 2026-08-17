import 'package:flutter/material.dart';

/// Password input with eye-toggle. Defaults to obscured. Clears its
/// TextEditingController on dispose (defense-in-depth: never let the
/// cleartext linger in widget-tree state after navigation).
///
/// **`TextEditingController.clear()` is NOT real zeroization** — same
/// F47 gap as OpaqueMnemonic. The controller's internal buffer holds
/// the password until GC. v0.2: route through FFI Uint8List +
/// Finalizable.
class PasswordField extends StatefulWidget {
  const PasswordField({super.key, required this.onChanged, this.onSubmitted});
  final ValueChanged<String> onChanged;
  final ValueChanged<String>? onSubmitted;

  @override
  State<PasswordField> createState() => _PasswordFieldState();
}

class _PasswordFieldState extends State<PasswordField> {
  bool _obscure = true;
  late final TextEditingController _controller;

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

  @override
  Widget build(BuildContext context) {
    return TextField(
      controller: _controller,
      obscureText: _obscure,
      autocorrect: false,
      enableSuggestions: false,
      onChanged: widget.onChanged,
      onSubmitted: widget.onSubmitted,
      decoration: InputDecoration(
        labelText: 'Password',
        border: const OutlineInputBorder(),
        suffixIcon: IconButton(
          icon: Icon(_obscure ? Icons.visibility : Icons.visibility_off),
          onPressed: () => setState(() => _obscure = !_obscure),
        ),
      ),
    );
  }
}
