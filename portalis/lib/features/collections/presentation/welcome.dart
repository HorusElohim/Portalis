// Empty-state presentation for the collections feature.

import 'package:flutter/material.dart';

import '../../../theme.dart';
import '../../../design/identity.dart';
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
/// The optional logo action keeps the shared welcome visual reusable while
/// allowing an empty library to make the mark its primary share affordance.
class Welcome extends StatelessWidget {
  const Welcome({super.key, this.titleSize = 32, this.onLogoTap});

  /// The window decides most type scale in this app (see [ScreenHeader]),
  /// but this headline sits outside that frame on both call sites, so each
  /// states its own.
  final double titleSize;
  final VoidCallback? onLogoTap;

  @override
  Widget build(BuildContext context) {
    final logo = const PulseRings(
      size: 168,
      child: PortalisLogo(size: 72),
    );
    final shareLogo = onLogoTap == null
        ? logo
        : Tooltip(
            message: 'Share files',
            child: Semantics(
              button: true,
              label: 'Share files',
              child: GestureDetector(
                key: const Key('shareCollectionAction'),
                onTap: onLogoTap,
                child: logo,
              ),
            ),
          );

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        shareLogo,
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
