import 'dart:developer' as developer;

import 'package:flutter/material.dart';
import 'package:flutter_riverpod/flutter_riverpod.dart';

import '../../providers/esplora_config_provider.dart';

/// Settings screen — Esplora URL / SPKI pin editor + network picker.
///
/// **Reads + writes** `esploraConfigProvider` (Task 12):
/// - On mount: seed the form fields from the persisted `EsploraConfig`.
/// - On `Save`: call `esploraConfigProvider.notifier.update(...)` with
///   the current form values; the notifier writes through to the JSON
///   file on disk.
/// - On network change: re-derive URL/pin from `EsploraConfig.defaults(network)`
///   so the user sees the canonical config for the new network. The
///   user can still edit the URL/pin fields after switching (e.g. for
///   self-hosted Esplora).
///
/// **Lessons applied**:
/// - **L33.1 pure `build()`:** form fields are local (user-typed
///   text); provider state is read once in `initState` postFrame.
///   `build()` does NOT mutate state — only reads controllers.
/// - **L33.2 controller hoist:** `TextEditingController` for URL +
///   pin is `late final`, allocated in `initState`, disposed in
///   `dispose`. Never constructed inline in `build()` (Task 21 L12
///   CRITICAL fix pattern).
/// - **L34.1 defensive defaults:** `EsploraConfig.defaults(network)`
///   returns a typed `EsploraConfig` with safe defaults — no `null`
///   parsing path (no `parse:` callback here, so the L34.1 lesson
///   doesn't apply directly, but the typed-default pattern is the
///   same shape).
///
/// **No secrets:** the SPKI pin is a public TLS-fingerprint value
/// (hex-encoded SHA-256 of the SPKI, per RFC 7469). It's not secret.
/// The screen never logs `_urlController.text` or `_pinController.text`
/// — `developer.log` doesn't appear anywhere in this screen.
class SettingsScreen extends ConsumerStatefulWidget {
  const SettingsScreen({super.key});

  @override
  ConsumerState<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends ConsumerState<SettingsScreen> {
  String _network = 'testnet';
  late final TextEditingController _urlController;
  late final TextEditingController _pinController;

  @override
  void initState() {
    super.initState();
    // Hoisted controllers (L33.2). Allocated once; seeded in the
    // postFrameCallback below.
    _urlController = TextEditingController();
    _pinController = TextEditingController();
    WidgetsBinding.instance.addPostFrameCallback((_) async {
      if (!mounted) return;
      final cfg = await ref.read(esploraConfigProvider.future);
      if (!mounted) return;
      // L12 type-design Task 23 MEDIUM: fall back to 'testnet' if the
      // persisted config has an unrecognized network string (e.g.
      // hand-edited file, schema drift). `EsploraConfig.defaults`
      // has a catch-all `default:` branch that returns the input
      // verbatim with empty URL — the dropdown's `initialValue`
      // would then crash with a Flutter assertion if the value is
      // not in its `items` list.
      const knownNetworks = {
        'bitcoin', 'testnet', 'testnet4', 'signet', 'regtest',
      };
      final network = knownNetworks.contains(cfg.network)
          ? cfg.network
          : 'testnet';
      setState(() {
        _network = network;
        _urlController.text = cfg.url;
        _pinController.text = cfg.spkiPin;
      });
    });
  }

  @override
  void dispose() {
    _urlController.dispose();
    _pinController.dispose();
    super.dispose();
  }

  /// Re-derive URL/pin from the canonical defaults when the network
  /// changes. User can still edit the fields afterward (e.g. for a
  /// self-hosted Esplora on testnet).
  void _onNetworkChanged(String? v) {
    final network = v ?? 'testnet';
    final defaults = EsploraConfig.defaults(network);
    setState(() {
      _network = network;
      _urlController.text = defaults.url;
      _pinController.text = defaults.spkiPin;
    });
  }

  Future<void> _save() async {
    try {
      await ref.read(esploraConfigProvider.notifier).save(
            EsploraConfig(
              network: _network,
              url: _urlController.text.trim(),
              spkiPin: _pinController.text.trim(),
            ),
          );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Saved')),
        );
      }
    } catch (e, st) {
      // L12 type-design Task 23 MEDIUM (CRITICAL per pr-test-analyzer):
      // `save()` writes JSON to disk — FileSystemException on disk
      // full / permission denied / parent locked. Without the try/catch
      // the failure was silent and the user got no feedback. Log via
      // dart:developer (L21 pattern — no BtcError surface here since
      // this is local file IO, not a CLI invocation) and surface an
      // error SnackBar so the user can retry or diagnose.
      developer.log(
        'settings save failed',
        name: 'SettingsScreen',
        error: e,
        stackTrace: st,
      );
      if (mounted) {
        ScaffoldMessenger.of(context).showSnackBar(
          const SnackBar(content: Text('Save failed.')),
        );
      }
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      appBar: AppBar(title: const Text('Settings')),
      body: Padding(
        padding: const EdgeInsets.all(16),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            DropdownButtonFormField<String>(
              key: const Key('settings_network'),
              initialValue: _network,
              decoration: const InputDecoration(
                labelText: 'Network',
                border: OutlineInputBorder(),
              ),
              items: const [
                DropdownMenuItem(value: 'bitcoin', child: Text('bitcoin')),
                DropdownMenuItem(value: 'testnet', child: Text('testnet')),
                DropdownMenuItem(value: 'testnet4', child: Text('testnet4')),
                DropdownMenuItem(value: 'signet', child: Text('signet')),
                DropdownMenuItem(value: 'regtest', child: Text('regtest')),
              ],
              onChanged: _onNetworkChanged,
            ),
            const SizedBox(height: 16),
            TextField(
              key: const Key('settings_url'),
              controller: _urlController,
              decoration: const InputDecoration(
                labelText: 'Esplora URL',
                border: OutlineInputBorder(),
              ),
              autocorrect: false,
              enableSuggestions: false,
            ),
            const SizedBox(height: 16),
            TextField(
              key: const Key('settings_pin'),
              controller: _pinController,
              decoration: const InputDecoration(
                labelText: 'SPKI pin (64-char hex)',
                border: OutlineInputBorder(),
              ),
              autocorrect: false,
              enableSuggestions: false,
            ),
            const SizedBox(height: 16),
            FilledButton(
              key: const Key('settings_save'),
              onPressed: _save,
              child: const Text('Save'),
            ),
          ],
        ),
      ),
    );
  }
}
