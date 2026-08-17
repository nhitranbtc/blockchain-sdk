import 'package:flutter/material.dart';

/// Modal-style progress overlay. Use inside a [Stack] above the
/// screen content. Renders nothing when [isRunning] is false.
class ProcessProgressOverlay extends StatelessWidget {
  const ProcessProgressOverlay({
    super.key,
    required this.isRunning,
    this.label,
  });
  final bool isRunning;
  final String? label;

  @override
  Widget build(BuildContext context) {
    if (!isRunning) return const SizedBox.shrink();
    final scheme = Theme.of(context).colorScheme;
    return Positioned.fill(
      child: ColoredBox(
        color: scheme.scrim,
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const CircularProgressIndicator(),
              if (label != null)
                Padding(
                  padding: const EdgeInsets.only(top: 16),
                  child: Text(
                    label!,
                    style: TextStyle(color: scheme.onInverseSurface),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
