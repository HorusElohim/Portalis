import 'package:flutter/material.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'home_screen.dart';

/// App root: just Collections (Home). User and Settings are reached via
/// the icons in Home's top bar (avatar / gear) rather than a bottom tab
/// bar — there's only one real destination, so a tab bar had nothing left
/// to switch between.
class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> {
  @override
  void initState() {
    super.initState();
    // One start() now covers everything: collections created or joined in a
    // previous session load from disk and appear here immediately, alongside
    // any plain torrents in the session.
    Collections.instance.start();
  }

  @override
  void dispose() {
    Collections.instance.stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(child: PageBody(child: HomeScreen())),
    );
  }
}
