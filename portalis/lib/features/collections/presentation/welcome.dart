// Empty-state presentation for the collections feature.

import 'package:flutter/material.dart';

import '../../../theme.dart';
import '../../../design/indicators.dart';
import '../../../design/primitives.dart';

/// The shared "what Portalis is" moment: the pulsing mark, the headline and
/// the one line under it.
///
/// Appears wherever there is nothing more specific on screen yet — mobile's
/// Home, and desktop's Collections pane before anything has been shared or
/// joined. It used to be written out in both places, close enough to have
/// drifted (a comma here, a line-break there) but never identical — this is
/// the one copy, so the two can no longer disagree about what the welcome
/// says. Anything a call site needs beyond this — a footer line, a set of
/// action buttons — is its own content placed around this widget, not a
/// parameter added to it: this stays exactly the fact both screens share.
class Welcome extends StatelessWidget {
  const Welcome({super.key, this.titleSize = 32});

  /// The window decides most type scale in this app (see [ScreenHeader]),
  /// but this headline sits outside that frame on both call sites, so each
  /// states its own.
  final double titleSize;

  @override
  Widget build(BuildContext context) {
    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        PulseRings(
          size: 168,
          child: ClipRRect(
            borderRadius: BorderRadius.circular(AppRadius.card),
            child: Image.asset(
              'assets/PortalisNature.png',
              width: 72,
              height: 72,
              // Decoded near the size it is drawn, not the source's 1254² —
              // so the full-resolution bitmap never enters the image cache
              // for a 72pt slot.
              cacheWidth: 216,
              cacheHeight: 216,
              filterQuality: FilterQuality.medium,
            ),
          ),
        ),
        const SizedBox(height: 22),
        CanvasTitle(
          'Send anything,\nstraight to a friend',
          size: titleSize,
          textAlign: TextAlign.center,
          height: 1.03,
        ),
        const SizedBox(height: 10),
        Text(
          'No uploads, no size limits. Files move device to device — '
          'and stay on yours.',
          textAlign: TextAlign.center,
          style: AppText.body(color: AppColors.textDim, height: 1.5),
        ),
      ],
    );
  }
}
