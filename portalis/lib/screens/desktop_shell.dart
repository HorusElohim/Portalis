import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/navigation.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'desktop_collections_pane.dart';
import 'desktop_pane.dart';
import 'desktop_sidebar.dart';
import 'join_collection_screen.dart';
import 'people_screen.dart';
import 'settings_screen.dart';
import 'share_screen.dart';
import 'user_screen.dart';

/// The wide-window layout: sidebar, list, and whatever is being looked at.
///
/// Same model and the same widgets as mobile — only the arrangement differs.
/// Reached from [RootShell] on width alone, so resizing the window moves
/// between layouts without any platform check. What each [DesktopPane] means
/// is documented there, next to the type every other file in this trio reads
/// it from.
class DesktopShell extends StatefulWidget {
  const DesktopShell({super.key});

  @override
  State<DesktopShell> createState() => _DesktopShellState();
}

class _DesktopShellState extends State<DesktopShell> {
  DesktopPane _pane = DesktopPane.collections;

  /// The collection whose card is showing its contents, if any.
  String? _openId;

  /// The two shells share a destination wherever they have one in common, so
  /// that resizing across the breakpoint leaves you where you were rather than
  /// somewhere arbitrary — which matters now that the window can be dragged
  /// freely between the desktop and phone layouts. It is also what makes
  /// Home's "go to Collections" link work here: it sets the tab, and nothing
  /// was listening.
  /// Mobile's Home lands on Collections, which is what this layout puts in its
  /// place. The tab itself is left alone in that case, so narrowing the window
  /// again returns you to Home rather than stranding you somewhere you never
  /// chose.
  static DesktopPane? _paneForTab(int tab) => switch (tab) {
        0 || 1 => DesktopPane.collections,
        2 => DesktopPane.people,
        3 => DesktopPane.you,
        _ => null,
      };

  static int? _tabForPane(DesktopPane pane) => switch (pane) {
        DesktopPane.collections => 1,
        DesktopPane.people => 2,
        DesktopPane.you => 3,
        // No mobile peer: Settings is reached through You there, and
        // Share/Join are their own pushed screens on mobile rather than a
        // pane of anything.
        DesktopPane.settings || DesktopPane.share || DesktopPane.join => null,
      };

  @override
  void initState() {
    super.initState();
    _pane = _paneForTab(AppNavigation.tab.value) ?? DesktopPane.collections;
    AppNavigation.tab.addListener(_onTabChanged);
  }

  @override
  void dispose() {
    AppNavigation.tab.removeListener(_onTabChanged);
    super.dispose();
  }

  void _onTabChanged() {
    final pane = _paneForTab(AppNavigation.tab.value);
    if (pane != null && pane != _pane && mounted) {
      setState(() => _pane = pane);
    }
  }

  /// Selects a pane, or closes it if it is already open.
  ///
  /// Toggling matters now that Collections has no control of its own: the
  /// collections are what the window shows when nothing is layered over them,
  /// so tapping the open pane's button again is how you get back to them.
  void _select(DesktopPane pane) {
    final next = pane == _pane ? DesktopPane.collections : pane;
    if (next == _pane) return;
    setState(() => _pane = next);
    final tab = _tabForPane(next);
    if (tab != null) AppNavigation.tab.value = tab;
  }

  /// A code the omnibar recognised, on its way to the join pane. Held here
  /// rather than passed through the pane, so the pane stays stateless about
  /// what is on screen beside it.
  String? _pendingInvite;

  void _joinWithCode(String code) {
    setState(() => _pendingInvite = code);
    _select(DesktopPane.join);
  }

  /// Clicking an open collection closes it again — the card is the view, so
  /// there is nothing else for a second click to mean.
  void _open(String id) {
    setState(() => _openId = _openId == id ? null : id);
    _select(DesktopPane.collections);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: ListenableBuilder(
        listenable: Collections.instance,
        builder: (context, _) {
          // The same wash mobile's shell carries — brightening with real
          // throughput, dark and still otherwise. Desktop drew a flat
          // background regardless of what was moving; this was the one
          // piece of "the two shells share a model" that stopped at the
          // window's edge.
          return AmbientBackground(
            intensity: Glow.intensityForRate(Collections.instance.liveRate),
            // No second panel. A collection opens inside its own card, in
            // the one list — a panel beside it was a second, thinner
            // account of the same collection, and a button to get from
            // one to the other.
            child: Row(
              children: [
                DesktopSidebar(pane: _pane, onPane: _select),
                Expanded(
                  child: SafeArea(
                    child: _pane == DesktopPane.collections
                        ? DesktopCollectionsPane(
                            openId: _openId,
                            onOpen: _open,
                            onShare: () => _select(DesktopPane.share),
                            onJoin: _joinWithCode,
                          )
                        : _centre(),
                  ),
                ),
              ],
            ),
          );
        },
      ),
    );
  }

  Widget _centre() {
    switch (_pane) {
      // `embedded: true` — see AppScreen. Each renders bare (no
      // Scaffold, no SafeArea), relying on the one already established in
      // build() above, so getting that right isn't something this switch or
      // any one screen has to remember on its own.
      case DesktopPane.you:
        return const UserScreen(embedded: true);
      case DesktopPane.people:
        return const PeopleScreen(embedded: true);
      case DesktopPane.settings:
        return const SettingsScreen(embedded: true);
      // Share and Join have their own Scaffold/SafeArea regardless of
      // context — a one-shot action rather than a destination with an
      // embedded/pushed duality, so AppScreen doesn't apply. Closing
      // returns to the list they replaced, same as re-tapping an open header
      // button.
      case DesktopPane.share:
        return ShareScreen(onClose: () => _select(DesktopPane.collections));
      case DesktopPane.join:
        return JoinCollectionScreen(
          // Keyed by the code so pasting a second one rebuilds the screen
          // around it rather than leaving the first one in the field.
          key: ValueKey(_pendingInvite),
          initialCode: _pendingInvite,
          onClose: () {
            _pendingInvite = null;
            _select(DesktopPane.collections);
          },
        );
      case DesktopPane.collections:
        // Rendered directly in build(), since it is always on screen.
        return const SizedBox.shrink();
    }
  }
}

