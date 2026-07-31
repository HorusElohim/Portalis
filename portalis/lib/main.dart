import 'package:flutter/material.dart';
import 'bridge_generated/frb_generated.dart';
import 'screens/root_shell.dart';
import 'services/navigation.dart';
import 'theme.dart';
import 'ui/ui.dart';

Future<void> main() async {
  // Rely on flutter_rust_bridge's default external library loader for all platforms.
  await RustLib.init();
  runApp(const MyApp());
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
          // Stacked above every route: the Home button has to outlive the
          // screens it returns you from.
          child: Stack(
            children: [
              child ?? const SizedBox.shrink(),
              const AppHomeButton(),
            ],
          ),
        ),
      ),
    );
  }
}
