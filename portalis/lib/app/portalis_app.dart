import 'package:flutter/material.dart';

import '../design/design.dart';
import '../screens/root_shell.dart';
import '../services/navigation.dart';
import '../theme.dart';

/// Flutter composition. Native startup lives in the bootstrap module.
class MyApp extends StatelessWidget {
  const MyApp({super.key});

  @override
  Widget build(BuildContext context) => MaterialApp(
        title: 'Portalis',
        debugShowCheckedModeBanner: false,
        theme: AppTheme.dark,
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
      );
}
