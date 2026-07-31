// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../services/navigation.dart';
import '../theme.dart';

/// A persistent way back to Collections, pinned to the top centre.
///
/// Sits above the navigator so it survives every push — from a collection,
/// three screens into settings, or the media viewer — which is the whole point:
/// there was previously no single gesture that returned you to the start.
///
/// Hidden when it would do nothing. On the Collections tab with nothing pushed
/// it isn't rendered at all, so it never covers content it can't help with.
/// Top *centre* specifically: back buttons live top-left and actions top-right,
/// and the middle is the one place that is free on every screen.
class AppHomeButton extends StatelessWidget {
  const AppHomeButton({super.key});

  @override
  Widget build(BuildContext context) {
    return ValueListenableBuilder<int>(
      valueListenable: AppNavigation.depth,
      builder: (context, _, __) => ValueListenableBuilder<int>(
        valueListenable: AppNavigation.tab,
        builder: (context, _, __) {
          if (!AppNavigation.isAwayFromHome) return const SizedBox.shrink();
          return Align(
            alignment: Alignment.topCenter,
            child: SafeArea(
              child: Padding(
                padding: const EdgeInsets.only(top: 6),
                child: _Pill(onTap: AppNavigation.goHome),
              ),
            ),
          );
        },
      ),
    );
  }
}

class _Pill extends StatelessWidget {
  const _Pill({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface.withValues(alpha: 0.92),
      shape: StadiumBorder(
        side: BorderSide(color: AppColors.signal.withValues(alpha: 0.28)),
      ),
      // A real shadow, because this floats over arbitrary screen content and
      // has to stay legible against a photo as well as a dark list.
      elevation: 6,
      shadowColor: AppColors.signal.withValues(alpha: 0.18),
      child: InkWell(
        key: const Key('appHomeButton'),
        customBorder: const StadiumBorder(),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.fromLTRB(10, 7, 14, 7),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              ClipRRect(
                borderRadius: BorderRadius.circular(7),
                child: Image.asset(
                  'assets/PortalisNature.png',
                  width: 20,
                  height: 20,
                  // Decoded at 3× its 20pt slot rather than the source's
                  // 1254², so a persistent chrome element doesn't park a
                  // full-resolution bitmap in the image cache.
                  cacheWidth: 60,
                  cacheHeight: 60,
                  filterQuality: FilterQuality.medium,
                ),
              ),
              const SizedBox(width: 8),
              Text('Home', style: displayText(size: 13.5)),
            ],
          ),
        ),
      ),
    );
  }
}
