import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/picked_file.dart';
import '../services/navigation.dart';
import 'desktop_pane.dart';
import '../features/collections/presentation/collection_detail.dart';
import '../features/collections/presentation/collection_join.dart';
import '../features/collections/presentation/collection_share.dart';

/// The one stateful shell for every window size.
///
/// Lifecycle, selected destination, open collection, and one-shot flows live
/// here. Subclasses only choose how that state is arranged on screen, so
/// resizing a Windows window cannot replace one navigation state with another.
abstract class AdaptiveShell extends StatefulWidget {
  const AdaptiveShell({super.key});
}

abstract class AdaptiveShellState<T extends AdaptiveShell> extends State<T>
    with WidgetsBindingObserver {
  int get tab => AppNavigation.tab.value;
  DesktopPane get pane => _pane;
  String? get openId => _openId;
  String? get pendingInvite => _pendingInvite;
  List<PickedFile>? get pendingShareFiles => _pendingShareFiles;

  DesktopPane _pane = DesktopPane.home;
  String? _openId;
  String? _pendingInvite;
  List<PickedFile>? _pendingShareFiles;

  @override
  void initState() {
    super.initState();
    WidgetsBinding.instance.addObserver(this);
    AppNavigation.tab.addListener(_onTabChanged);
    AppControllers.collections.start();
    AppControllers.settings.load();
    AppControllers.identity.load();
  }

  @override
  void didChangeAppLifecycleState(AppLifecycleState state) {
    AppControllers.collections.setPaused(state != AppLifecycleState.resumed);
  }

  void _onTabChanged() {
    final next = _paneForTab(AppNavigation.tab.value);
    if (next != null && next != _pane && mounted) {
      setState(() => _pane = next);
    } else if (mounted) {
      setState(() {});
    }
  }

  void selectTab(int index) {
    AppNavigation.tab.value = index;
  }

  void selectPane(DesktopPane value) {
    final next = value == _pane ? DesktopPane.home : value;
    if (next == _pane) return;
    setState(() => _pane = next);
    final tab = _tabForPane(next);
    if (tab != null) AppNavigation.tab.value = tab;
  }

  void openCollection(String id, {required bool inline}) {
    final collection = AppControllers.collections.byId(id);
    if (collection == null) return;
    if (!inline) {
      Navigator.of(context).push(
        MaterialPageRoute(builder: (_) => CollectionScreen(collection: collection)),
      );
      return;
    }
    setState(() => _openId = _openId == id ? null : id);
    selectPane(DesktopPane.home);
  }

  void openShare([List<PickedFile>? initialFiles, bool inline = false]) {
    if (!inline) {
      Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => ShareScreen(initialFiles: initialFiles),
        ),
      );
      return;
    }
    setState(() => _pendingShareFiles = initialFiles);
    selectPane(DesktopPane.share);
  }

  void openJoin(String code, {bool inline = false}) {
    if (!inline) {
      Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => JoinCollectionScreen(initialCode: code),
        ),
      );
      return;
    }
    setState(() => _pendingInvite = code);
    selectPane(DesktopPane.join);
  }

  void closeShare() {
    setState(() => _pendingShareFiles = null);
    selectPane(DesktopPane.home);
  }

  void closeJoin() {
    setState(() => _pendingInvite = null);
    selectPane(DesktopPane.home);
  }

  @protected
  Widget buildCompactLayout(BuildContext context);

  @protected
  Widget buildWideLayout(BuildContext context);

  @override
  Widget build(BuildContext context) => WindowBuilder(
        builder: (context, window) => ListenableBuilder(
          listenable: AppControllers.collections,
          builder: (context, _) => KeyedSubtree(
            // Desktop and compact layouts have incompatible parent chains.
            // Make a breakpoint crossing an explicit replacement rather than
            // letting Flutter attempt to retain dependencies from one layout
            // under the other while MediaQuery is notifying resize listeners.
            // Navigation itself remains above this boundary in this State.
            key: ValueKey(window.isDesktop),
            child: window.isDesktop
                ? buildWideLayout(context)
                : buildCompactLayout(context),
          ),
        ),
      );

  @override
  void dispose() {
    AppNavigation.tab.removeListener(_onTabChanged);
    WidgetsBinding.instance.removeObserver(this);
    AppControllers.collections.stop();
    super.dispose();
  }

  static DesktopPane? _paneForTab(int value) => switch (value) {
        0 => DesktopPane.home,
        1 => DesktopPane.user,
        2 => DesktopPane.people,
        3 => DesktopPane.settings,
        _ => null,
      };

  static int? _tabForPane(DesktopPane value) => switch (value) {
        DesktopPane.home => 0,
        DesktopPane.user => 1,
        DesktopPane.people => 2,
        DesktopPane.settings => 3,
        DesktopPane.share || DesktopPane.join => null,
      };
}
