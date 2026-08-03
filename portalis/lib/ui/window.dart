// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

/// How much horizontal room a screen has been given, and the thresholds the
/// app's layout keys off — the vocabulary a screen reasoning about available
/// width should use, instead of ad hoc `constraints.maxWidth >= <magic
/// number>` checks repeated at each call site.
class WindowSize {
  const WindowSize(this.width);

  final double width;

  /// At or above this, [RootShell] shows [DesktopShell]'s three-pane layout
  /// instead of the phone one.
  ///
  /// Chosen on available width, never on `Platform`: a narrow window on a
  /// Mac should get the phone layout, and a wide tablet should get the
  /// desktop one.
  ///
  /// The desktop runners *open* above this, so the app never launches into
  /// the phone layout by accident, but they can be dragged well below it:
  /// the phone layout is a first-class layout on desktop too, and the
  /// crossing is a real transition rather than a degradation. Their floor is
  /// the smallest phone the design targets — see
  /// `macos/Runner/MainFlutterWindow.swift`, `linux/my_application.cc` and
  /// `windows/runner/`.
  static const desktopBreakpoint = 1000.0;

  /// At or above this, an embedded desktop pane (Settings, People) has
  /// enough room to justify a second column instead of one long one.
  static const spaciousBreakpoint = 860.0;

  bool get isDesktop => width >= desktopBreakpoint;
  bool get isSpacious => width >= spaciousBreakpoint;

  /// How many roughly-[itemWidth]-wide cards fit across, never fewer than
  /// one and never more than four — a fifth-plus column reads as clutter
  /// before it reads as use of the space.
  int columns(double itemWidth) => (width / itemWidth).floor().clamp(1, 4);
}

/// [LayoutBuilder], but handing the builder a [WindowSize] rather than raw
/// [BoxConstraints] — the one place a screen's available width becomes the
/// vocabulary above instead of another ad hoc threshold.
class WindowBuilder extends StatelessWidget {
  const WindowBuilder({super.key, required this.builder});

  final Widget Function(BuildContext context, WindowSize window) builder;

  @override
  Widget build(BuildContext context) {
    return LayoutBuilder(
      builder: (context, constraints) =>
          builder(context, WindowSize(constraints.maxWidth)),
    );
  }
}
