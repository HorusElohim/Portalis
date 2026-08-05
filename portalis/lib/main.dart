import 'package:flutter/material.dart';
import 'bridge_generated/bridge.dart';
import 'bridge_generated/frb_generated.dart';
import 'screens/root_shell.dart';
import 'services/navigation.dart';
import 'theme.dart';
import 'ui/ui.dart';
import 'version.dart';

Future<void> main() async {
  try {
    // Rely on flutter_rust_bridge's default external library loader for all
    // platforms, then reject a stale native library before any DTO is decoded.
    await RustLib.init();
    final backendVersion = getVersion();
    if (backendVersion != expectedBackendVersion) {
      throw StateError(
        'Native backend $backendVersion is incompatible with this frontend '
        '(expected backend $expectedBackendVersion). '
        'Regenerate the bridge and rebuild the native backend together.',
      );
    }
    runApp(const MyApp());
  } catch (error) {
    runApp(_StartupErrorApp(error: error));
  }
}

class _StartupErrorApp extends StatelessWidget {
  const _StartupErrorApp({required this.error});

  final Object error;

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Portalis startup error',
      home: Scaffold(
        body: Center(
          child: Padding(
            padding: const EdgeInsets.all(24),
            child: SelectableText(
              'Portalis could not start.\n\n$error\n\n'
              'From the portalis folder, run:\n'
              'cargo build --release --manifest-path rust/backend/Cargo.toml\n\n'
              'Then restart Flutter. Regenerate the bindings too when a Rust '
              'API or DTO changes.',
            ),
          ),
        ),
      ),
    );
  }
}

class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) {
    return MaterialApp(
      title: 'Portalis',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark,
      home: const RootShell(),
      navigatorKey: AppNavigation.navigatorKey,
      // Tracks how deep the stack is, so the Home button knows when it
      // would actually do something.
      navigatorObservers: [AppNavigation.observer],
      builder: (context, child) => ToastScope(
        // Above the navigator, so a toast outlives the screen that raised it:
        // creating a collection pops back to Home *and then* reports success.
        child: GestureDetector(
          // Without this, the keyboard stays up after navigating away from a
          // screen with a focused TextField (e.g. Add Torrent's name/magnet
          // fields) — nothing else ever tells it to dismiss. Tapping anywhere
          // outside an input, on any screen, now closes it.
          behavior: HitTestBehavior.opaque,
          onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
          child: child ?? const SizedBox.shrink(),
        ),
      ),
    );
  }
}
