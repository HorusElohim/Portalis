// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../theme.dart';

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
        gradient: primary
            ? const LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [AppColors.signal, AppColors.signalDim],
              )
            : null,
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
