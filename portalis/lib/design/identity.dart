// Cross-feature identity design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';
import 'indicators.dart';

/// Portalis mark used in persistent chrome and transient event feedback.
class PortalisLogo extends StatelessWidget {
  const PortalisLogo({
    super.key,
    this.size = 32,
    this.energized = false,
  });

  final double size;
  final bool energized;

  @override
  Widget build(BuildContext context) {
    final image = ClipRRect(
      borderRadius: BorderRadius.circular(size * 0.28),
      child: Image.asset(
        'assets/PortalisNature.png',
        width: size,
        height: size,
        cacheWidth: (size * 3).round(),
        cacheHeight: (size * 3).round(),
        filterQuality: FilterQuality.medium,
      ),
    );
    return energized ? PulseRings(size: size * 1.65, child: image) : image;
  }
}

/// Initials avatar. [primary] gives the mint-gradient treatment the design
/// reserves for *this device's* identity; collaborators get the flat
/// dark-mint chip, so the two are never confused inside an avatar stack.
///
/// Squircle rather than circle, matching the rest of the Signal geometry.
class Avatar extends StatelessWidget {
  const Avatar({
    super.key,
    required this.initials,
    this.size = 30,
    this.primary = false,
  });

  final String initials;
  final double size;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: primary ? null : AppColors.signalDeep,
        gradient: primary ? signalFill : null,
        borderRadius: BorderRadius.circular(size * 0.34),
      ),
      child: Text(
        initials,
        style: TextStyle(
          fontFamily: AppFonts.display,
          color: primary ? AppColors.onSignal : AppColors.signalSoft,
          fontSize: size * 0.42,
          fontWeight: FontWeight.w700,
        ),
      ),
    );
  }
}
