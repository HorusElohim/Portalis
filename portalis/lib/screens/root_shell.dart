import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/navigation.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'desktop_shell.dart';
import 'collections_screen.dart';
import 'home_screen.dart';
import 'transfers_screen.dart';
import 'user_screen.dart';

/// Width at or above which the app switches to the three-pane desktop layout.
///
/// Chosen on available width, never on `Platform`: a narrow window on a Mac
/// should get the phone layout, and a wide tablet should get the desktop one.
///
/// The desktop runners open above this and refuse to be dragged below it, so
/// the desktop app is never in the phone layout by accident — see the sizes in
/// `macos/Runner/MainFlutterWindow.swift`, `linux/my_application.cc` and
/// `windows/runner/`. Change one and change the others.
const kDesktopBreakpoint = 1000.0;

/// App root. Three destinations on mobile — Collections, Transfers, You —
/// and a three-pane layout on a wide window.
class RootShell extends StatefulWidget {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends State<RootShell> with WidgetsBindingObserver {
  /// The selected tab lives in [AppNavigation] rather than here, so the
  /// persistent Home button — which sits *above* this widget — can change it.
  int get _tab => AppNavigation.tab.value;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    AppNavigation.tab.addListener(_onTabChanged);
    // One start() covers everything: collections created or joined in a
    // previous session load from disk and appear immediately, alongside any
    // plain torrents in the session.
    Collections.instance.start();
    SettingsService.instance.load();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    // Stop polling whenever the app isn't in front of the user. Seeding
    // continues in Rust regardless — there is just no reason to wake the Dart
    // isolate to redraw a list nobody is looking at.
    Collections.instance.setPaused(state != AppLifecycleState.resumed);
  }

  void _onTabChanged() {
    if (mounted) setState(() {});
  }

  @override
  void dispose() {
    AppNavigation.tab.removeListener(_onTabChanged);
    WidgetsBinding.instance.removeObserver(this);
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
        return ListenableBuilder(
          listenable: Collections.instance,
          builder: (context, _) {
            final rate = Collections.instance.liveRate;
            return Scaffold(
              backgroundColor: AppColors.surfaceDeep,
              body: AmbientBackground(
                intensity: AmbientBackground.intensityForRate(rate),
                child: SafeArea(
                  bottom: false,
                  child: IndexedStack(
                    index: _tab,
                    children: [
                      // IndexedStack keeps every tab alive so switching is
                      // instant — but that also keeps their animations
                      // ticking off-screen. TickerMode freezes the ones the
                      // user can't see, which matters more now that Home
                      // carries a permanently-animating motif.
                      for (var i = 0; i < AppBottomNav.items.length; i++)
                        TickerMode(
                          enabled: i == _tab,
                          child: const [
                            HomeScreen(),
                            CollectionsScreen(),
                            TransfersScreen(),
                            UserScreen(),
                          ][i],
                        ),
                    ],
                  ),
                ),
              ),
              bottomNavigationBar: AppBottomNav(
                index: _tab,
                onSelected: (i) => AppNavigation.tab.value = i,
              ),
            );
          },
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

  /// The leftmost destination is Home, and it carries the app's mark rather
  /// than a generic glyph — it is both "where you are" and "how you get
  /// back". Collections is its own peer beside it: Home answers "what can I
  /// do", Collections answers "what do I have".
  static const items = [
    (icon: null, label: 'Home'),
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
          padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
          child: Row(
            children: [
              for (var i = 0; i < items.length; i++)
                Expanded(
                  child: InkWell(
                    key: Key('navTab$i'),
                    borderRadius: BorderRadius.circular(12),
                    // Home doesn't just select a tab: it unwinds anything
                    // pushed on top as well, so one tap always lands you at
                    // the start rather than one screen shallower.
                    onTap: () => i == 0 ? AppNavigation.goHome() : onSelected(i),
                    child: Padding(
                      padding: const EdgeInsets.symmetric(vertical: 6),
                      child: Column(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          if (items[i].icon == null)
                            Opacity(
                              // The mark is full colour; dimming it is what
                              // makes an unselected tab read as unselected,
                              // the same as the glyphs beside it.
                              opacity: i == index ? 1 : 0.45,
                              child: ClipRRect(
                                borderRadius: BorderRadius.circular(7),
                                child: Image.asset(
                                  'assets/PortalisNature.png',
                                  width: 21,
                                  height: 21,
                                  // Decoded at 3× its slot, not the source's
                                  // 1254² — permanent chrome must not park a
                                  // full-resolution bitmap in the cache.
                                  cacheWidth: 63,
                                  cacheHeight: 63,
                                  filterQuality: FilterQuality.medium,
                                ),
                              ),
                            )
                          else
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
                              fontSize: 9.5,
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
