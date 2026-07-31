import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import 'desktop_shell.dart';
import 'home_screen.dart';
import 'transfers_screen.dart';
import 'user_screen.dart';

/// Width at or above which the app switches to the three-pane desktop layout.
///
/// Chosen on available width, never on `Platform`: a narrow window on a Mac
/// should get the phone layout, and a wide tablet should get the desktop one.
const kDesktopBreakpoint = 1000.0;

/// App root. Three destinations on mobile — Collections, Transfers, You —
/// and a three-pane layout on a wide window.
class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> {
  int _tab = 0;

  @override
  void initState() {
    super.initState();
    // One start() covers everything: collections created or joined in a
    // previous session load from disk and appear immediately, alongside any
    // plain torrents in the session.
    Collections.instance.start();
    SettingsService.instance.load();
  }

  @override
  void dispose() {
    Collections.instance.stop();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) {
        if (constraints.maxWidth >= kDesktopBreakpoint) {
          return const DesktopShell();
        }
        return Scaffold(
          backgroundColor: AppColors.surfaceDeep,
          body: SafeArea(
            bottom: false,
            child: IndexedStack(
              index: _tab,
              children: const [
                HomeScreen(),
                TransfersScreen(),
                UserScreen(),
              ],
            ),
          ),
          bottomNavigationBar: AppBottomNav(
            index: _tab,
            onSelected: (i) => setState(() => _tab = i),
          ),
        );
      },
    );
  }
}

/// The three-destination bottom bar.
class AppBottomNav extends StatelessWidget {
  const AppBottomNav({
    super.key,
    required this.index,
    required this.onSelected,
  });

  final int index;
  final ValueChanged<int> onSelected;

  static const items = [
    (icon: Icons.dashboard_outlined, label: 'Collections'),
    (icon: Icons.swap_horiz, label: 'Transfers'),
    (icon: Icons.person_outline, label: 'You'),
  ];

  @override
  Widget build(BuildContext context) {
    return Container(
      decoration: const BoxDecoration(
        color: AppColors.surfaceDeep,
        border: Border(top: BorderSide(color: AppColors.border)),
      ),
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(20, 8, 20, 8),
          child: Row(
            children: [
              for (var i = 0; i < items.length; i++)
                Expanded(
                  child: InkWell(
                    key: Key('navTab$i'),
                    borderRadius: BorderRadius.circular(12),
                    onTap: () => onSelected(i),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 6),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          Icon(
                            items[i].icon,
                            size: 21,
                            // Selection is plain white, not mint — mint is
                            // reserved for data actually moving.
                            color: i == index
                                ? AppColors.text
                                : AppColors.textGhost,
                          ),
                          const SizedBox(height: 5),
                          Text(
                            items[i].label,
                            style: TextStyle(
                              fontSize: 10,
                              fontWeight: i == index
                                  ? FontWeight.w600
                                  : FontWeight.w400,
                              color: i == index
                                  ? AppColors.text
                                  : AppColors.textGhost,
                            ),
                          ),
                        ],
                      ),
                    ),
                  ),
                ),
            ],
          ),
        ),
      ),
    );
  }
}
