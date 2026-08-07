import 'package:flutter/material.dart';

import '../design/design.dart';
import '../features/appearance/application/theme_controller.dart';
import '../screens/root_shell.dart';
import '../services/navigation.dart';
import '../theme.dart';

/// Flutter composition. Native startup lives in the bootstrap module.
class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: ThemeController.instance,
        builder: (context, _) => MaterialApp(
          // Keyed by the active theme so a switch remounts the whole tree
          // instead of rebuilding in place: many widgets below embed
          // AppColors.* inside `const` constructors, and Flutter skips
          // rebuilding an unchanged const instance on an in-place update —
          // only a fresh mount is guaranteed to repaint every one of them.
          key: ValueKey(ThemeController.instance.id),
          title: 'Portalis',
          debugShowCheckedModeBanner: false,
          theme: AppTheme.current,
          home: const RootShell(),
          navigatorKey: AppNavigation.navigatorKey,
          navigatorObservers: [AppNavigation.observer],
          builder: (context, child) => ToastScope(
            child: GestureDetector(
              behavior: HitTestBehavior.opaque,
              onTap: () => FocusManager.instance.primaryFocus?.unfocus(),
              child: child ?? const SizedBox.shrink(),
            ),
          ),
        ),
      );
}
