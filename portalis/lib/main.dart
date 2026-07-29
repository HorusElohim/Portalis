import 'package:flutter/material.dart';
import 'bridge_generated/frb_generated.dart';
import 'screens/root_shell.dart';
import 'theme.dart';

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
      title: 'SmartShare',
      debugShowCheckedModeBanner: false,
      theme: AppTheme.dark,
      home: const RootShell(),
    );
  }
}
