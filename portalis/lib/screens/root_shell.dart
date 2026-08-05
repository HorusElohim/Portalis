import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/navigation.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'desktop_shell.dart';
import 'home.dart';
import 'people.dart';
import 'settings.dart';

/// App root. Three destinations on mobile — Home, People, Settings — and a
/// two-pane layout on a wide window.
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
    return WindowBuilder(
      builder: (context, window) {
        if (window.isDesktop) {
          return const DesktopShell();
        }
        return ListenableBuilder(
          listenable: Collections.instance,
          builder: (context, _) {
            final rate = Collections.instance.liveRate;
            return Scaffold(
              backgroundColor: AppColors.surfaceDeep,
              body: AmbientBackground(
                intensity: Glow.intensityForRate(rate),
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
                          // PeopleScreen and SettingsScreen render through
                          // AppScreen, which wraps its own Scaffold and back
                          // button unless told this parent already supplies
                          // both — true here, same as every other tab.
                          child: const [
                            Home(),
                            PeopleScreen(embedded: true),
                            SettingsScreen(embedded: true),
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
  /// back". Home now answers both "what can I do" and "what do I have": the
  /// omnibar, the New-share/Add-torrent actions and the collection list all
  /// live there together (see `home.dart`), which is what let the old
  /// Collections destination fold away — it was the same list a second
  /// place, reached a second way.
  ///
  /// There is no Transfers destination either, for the same reason: every
  /// collection row carries its own bar, rate and countdown, Home filters to
  /// what is arriving, and its header states the aggregate.
  ///
  /// People *is* its own destination, back after two rounds of being reached
  /// only indirectly (see the regression note on the "you" test group) —
  /// desktop already gives it a one-tap header button, and a phone-width
  /// window burying the same directory three taps deep under Settings is
  /// exactly the asymmetry that made it easy to lose track of who a
  /// collection is even shared with.
  ///
  /// Settings is what You became once its profile content folded in as that
  /// screen's leading section (see `settings.dart`) — identity and engine
  /// behaviour are both "how this device behaves", not two different
  /// questions.
  static const items = [
    (icon: null, label: 'Home'),
    (icon: Icons.people_outline, label: 'People'),
    (icon: Icons.tune, label: 'Settings'),
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
                    borderRadius: BorderRadius.circular(AppRadius.inner),
                    // Home doesn't just select a tab: it unwinds anything
                    // pushed on top as well, so one tap always lands you at
                    // the start rather than one screen shallower.
                    onTap: () =>
                        i == 0 ? AppNavigation.goHome() : onSelected(i),
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
                                borderRadius:
                                    BorderRadius.circular(AppRadius.tight),
                                child: Image.asset(
                                  'assets/PortalisNature.png',
                                  width: 24,
                                  height: 24,
                                  // Decoded at 3× its slot, not the source's
                                  // 1254² — permanent chrome must not park a
                                  // full-resolution bitmap in the cache.
                                  cacheWidth: 72,
                                  cacheHeight: 72,
                                  filterQuality: FilterQuality.medium,
                                ),
                              ),
                            )
                          else
                            Icon(
                              items[i].icon,
                              size: 24,
                              // Selection is plain white, not mint — mint is
                              // reserved for data actually moving.
                              color: i == index
                                  ? AppColors.text
                                  : AppColors.textGhost,
                            ),
                          const SizedBox(height: 6),
                          Text(
                            items[i].label,
                            style: AppText.caption(
                              color: i == index
                                  ? AppColors.text
                                  : AppColors.textGhost,
                              weight: i == index
                                  ? FontWeight.w600
                                  : FontWeight.w400,
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
