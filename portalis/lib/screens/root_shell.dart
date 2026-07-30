import 'package:flutter/material.dart';
import '../services/collab_collections.dart';
import '../services/torrent_collections.dart';
import '../theme.dart';
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
    TorrentCollections.instance.start();
    // So collab collections (created/joined in a previous session) are
    // already loaded and ready to share/sync as soon as the app opens,
    // rather than only appearing after visiting the User screen once.
    CollabCollections.instance.refresh();
  }

  @override
  void dispose() {
    TorrentCollections.instance.stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return const Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(child: HomeScreen()),
    );
  }
}
