import 'package:flutter/widgets.dart';

/// Where the app currently is, at a level above any one screen.
///
/// Two facts, both needed by the persistent Home button: which tab the shell
/// is showing, and how deep the navigator has been pushed. Neither is
/// reachable from a widget sitting *above* the navigator, which is exactly
/// where that button lives.
class AppNavigation {
  AppNavigation._();

  /// The shell's selected destination. Owned here rather than in `RootShell`'s
  /// State so anything can send the user home — including a control rendered
  /// outside the shell entirely.
  static final tab = ValueNotifier<int>(0);

  /// How many routes are stacked above the root. Zero means the shell itself
  /// is on screen.
  static final depth = ValueNotifier<int>(0);

  /// Registered on `MaterialApp.navigatorObservers`.
  static final observer = _DepthObserver();

  /// The app's navigator.
  ///
  /// Held as a key because the Home button is rendered by
  /// `MaterialApp.builder` — *above* the Navigator — so it has no Navigator
  /// ancestor and `Navigator.of(context)` from there finds nothing.
  static final navigatorKey = GlobalKey<NavigatorState>();

  /// True when the user is anywhere other than the Collections tab of the
  /// shell — i.e. when "go home" would actually do something.
  static bool get isAwayFromHome => depth.value > 0 || tab.value != 0;

  /// Pops back to the shell and selects Collections.
  static void goHome() {
    if (depth.value > 0) {
      navigatorKey.currentState?.popUntil((route) => route.isFirst);
    }
    tab.value = 0;
  }
}

/// Keeps [AppNavigation.depth] in step with the real navigator stack.
class _DepthObserver extends NavigatorObserver {
  // Counted rather than read from the navigator on demand: the button lives
  // above the Navigator, so it has no context that could ask.
  void _set(int delta) {
    final next = AppNavigation.depth.value + delta;
    AppNavigation.depth.value = next < 0 ? 0 : next;
  }

  @override
  void didPush(Route<dynamic> route, Route<dynamic>? previousRoute) {
    // The root route is the shell itself, not a push away from it.
    if (previousRoute != null) _set(1);
  }

  @override
  void didPop(Route<dynamic> route, Route<dynamic>? previousRoute) => _set(-1);

  @override
  void didRemove(Route<dynamic> route, Route<dynamic>? previousRoute) =>
      _set(-1);

  @override
  void didReplace({Route<dynamic>? newRoute, Route<dynamic>? oldRoute}) {
    // Depth is unchanged by a replacement.
  }
}
