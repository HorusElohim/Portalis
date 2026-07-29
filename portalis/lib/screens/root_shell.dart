import 'package:flutter/material.dart';
import '../services/torrent_collections.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'home_screen.dart';
import 'settings_screen.dart';
import 'user_screen.dart';

/// Root shell hosting the three bottom-tab destinations: Collections
/// (home), User, and Settings. Adding a torrent (Home's "＋ Add torrent")
/// is what feeds Collections — there's no separate Torrent tab; see
/// `TorrentCollections` for why a torrent already *is* a collection.
class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> {
  RootTab _tab = RootTab.collections;

  @override
  void initState() {
    super.initState();
    TorrentCollections.instance.start();
  }

  @override
  void dispose() {
    TorrentCollections.instance.stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: IndexedStack(
          index: RootTab.values.indexOf(_tab),
          children: const [
            HomeScreen(),
            UserScreen(),
            SettingsScreen(),
          ],
        ),
      ),
      bottomNavigationBar: RootTabBar(
        current: _tab,
        onSelect: (tab) => setState(() => _tab = tab),
      ),
    );
  }
}
