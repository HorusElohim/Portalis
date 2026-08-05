import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/navigation.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'desktop_pane.dart';
import 'desktop_sidebar.dart';
import 'home.dart';
import 'home/collection/join.dart';
import 'home/collection/share.dart';
import 'people.dart';
import 'settings.dart';

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
  DesktopPane _pane = DesktopPane.home;

  /// The collection whose card is showing its contents, if any.
  String? _openId;

  /// The two shells share a destination wherever they have one in common, so
  /// that resizing across the breakpoint leaves you where you were rather than
  /// somewhere arbitrary — which matters now that the window can be dragged
  /// freely between the desktop and phone layouts. It is also what makes
  /// Home's "go to People" link work here: it sets the tab, and nothing
  /// was listening.
  static DesktopPane? _paneForTab(int tab) => switch (tab) {
        0 => DesktopPane.home,
        1 => DesktopPane.people,
        2 => DesktopPane.settings,
        _ => null,
      };

  static int? _tabForPane(DesktopPane pane) => switch (pane) {
        DesktopPane.home => 0,
        DesktopPane.people => 1,
        DesktopPane.settings => 2,
        // No mobile peer: Share/Join are their own pushed screens on mobile
        // rather than a pane of anything.
        DesktopPane.share || DesktopPane.join => null,
      };

  @override
  void initState() {
    super.initState();
    _pane = _paneForTab(AppNavigation.tab.value) ?? DesktopPane.home;
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
  /// Toggling matters now that Home has no control of its own: the
  /// collections are what the window shows when nothing is layered over them,
  /// so tapping the open pane's button again is how you get back to them.
  void _select(DesktopPane pane) {
    final next = pane == _pane ? DesktopPane.home : pane;
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

  /// Files a drop already picked, on their way to the share pane — see
  /// [_pendingInvite] for why this is held here rather than in the pane.
  /// Null when Home's own "New share" button is what triggered this instead.
  List<PickedFile>? _pendingShareFiles;

  void _openShare([List<PickedFile>? initialFiles]) {
    setState(() => _pendingShareFiles = initialFiles);
    _select(DesktopPane.share);
  }

  /// Clicking an open collection closes it again — the card is the view, so
  /// there is nothing else for a second click to mean.
  void _open(String id) {
    setState(() => _openId = _openId == id ? null : id);
    _select(DesktopPane.home);
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
                    child: _pane == DesktopPane.home
                        ? Home(
                            embedded: true,
                            openId: _openId,
                            onOpen: _open,
                            onShare: _openShare,
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
        return ShareScreen(
          // Keyed so a second drop rebuilds the screen around its own files
          // rather than leaving the first batch on screen.
          key: ValueKey(_pendingShareFiles),
          initialFiles: _pendingShareFiles,
          onClose: () {
            _pendingShareFiles = null;
            _select(DesktopPane.home);
          },
        );
      case DesktopPane.join:
        return JoinCollectionScreen(
          // Keyed by the code so pasting a second one rebuilds the screen
          // around it rather than leaving the first one in the field.
          key: ValueKey(_pendingInvite),
          initialCode: _pendingInvite,
          onClose: () {
            _pendingInvite = null;
            _select(DesktopPane.home);
          },
        );
      case DesktopPane.home:
        // Rendered directly in build(), since it is always on screen.
        return const SizedBox.shrink();
    }
  }
}
