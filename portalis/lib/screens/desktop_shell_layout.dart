import 'package:flutter/material.dart';

import '../design/design.dart';
import '../theme.dart';
import 'desktop_pane.dart';
import 'desktop_top_bar.dart';

/// Wide arrangement of the shared adaptive shell state.
class DesktopShellLayout extends StatelessWidget {
  const DesktopShellLayout({
    super.key,
    required this.pane,
    required this.onPane,
    required this.home,
    required this.content,
    required this.liveRate,
  });

  final DesktopPane pane;
  final ValueChanged<DesktopPane> onPane;
  final Widget home;
  final Widget content;
  final double liveRate;

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: AppColors.surfaceDeep,
        body: AmbientBackground(
          intensity: Glow.intensityForRate(liveRate),
          child: Column(
            children: [
              DesktopTopBar(pane: pane, onPane: onPane),
              Expanded(
                child: SafeArea(
                  top: false,
                  child: pane == DesktopPane.home ? home : content,
                ),
              ),
            ],
          ),
        ),
      );
}
