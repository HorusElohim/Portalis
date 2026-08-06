import 'package:flutter/material.dart';
import 'package:video_player_media_kit/video_player_media_kit.dart';

import '../bridge_generated/bridge.dart';
import '../bridge_generated/frb_generated.dart';
import '../version.dart';
import 'portalis_app.dart';

/// Starts the native backend before Flutter renders any screen.
Future<void> runPortalisApp() async {
  WidgetsFlutterBinding.ensureInitialized();
  try {
    VideoPlayerMediaKit.ensureInitialized(
      android: true,
      iOS: true,
      macOS: true,
      windows: true,
      linux: true,
    );
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
  Widget build(BuildContext context) => MaterialApp(
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
