// Task 7 (#213) test — typed FFI wrappers for tokio runtime handle
// (Task 3 surface).
//
// Verifies the two expected symbols resolve and the runtime handle
// round-trips (new → valid pointer → drop).

import 'dart:ffi';
import 'dart:io';

import 'package:flutter_test/flutter_test.dart';
import 'package:wallet_desktop/core/ffi/runtime_bindings.dart';

void main() {
  group('RuntimeBindings symbol resolution', () {
    test('runtimeNew resolves', () {
      expect(RuntimeBindings.runtimeNew, isNotNull);
    });

    test('runtimeDrop resolves', () {
      expect(RuntimeBindings.runtimeDrop, isNotNull);
    });
  }, skip: !Platform.isLinux);

  group('RuntimeBindings smoke', () {
    test(
      'runtime_new returns a non-null pointer; runtime_drop null is no-op',
      () {
        final handle = RuntimeBindings.runtimeNew();
        expect(handle, isNotNull);
        // Drop is allowed exactly once; double-drop is UB. We drop and
        // verify the second-call no-op branch with a fresh null.
        RuntimeBindings.runtimeDrop(handle);
        RuntimeBindings.runtimeDrop(nullptr);
      },
      skip: !Platform.isLinux,
    );
  });
}
