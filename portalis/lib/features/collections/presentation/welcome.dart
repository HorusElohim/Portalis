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
/// says. The optional logo action keeps the shared welcome visual reusable
/// while allowing an empty library to make the mark its primary share
/// affordance.
class Welcome extends StatefulWidget {
  const Welcome({
    super.key,
    this.titleSize = 32,
    this.onLogoTap,
    this.animationCycle = 0,
    this.footer,
  });

  /// The window decides most type scale in this app (see [ScreenHeader]),
  /// but this headline sits outside that frame on both call sites, so each
  /// states its own.
  final double titleSize;
  final VoidCallback? onLogoTap;
  final int animationCycle;
  final String? footer;

  @override
  State<Welcome> createState() => _WelcomeState();
}

class _WelcomeState extends State<Welcome> with SingleTickerProviderStateMixin {
  late final AnimationController _controller;
  late final Animation<double> _visibility;

  @override
  void initState() {
    super.initState();
    _controller = AnimationController(
      vsync: this,
      duration: const Duration(seconds: 7),
    );
    _visibility = TweenSequence<double>([
      TweenSequenceItem(tween: ConstantTween(1.0), weight: 22),
      TweenSequenceItem(
        tween: Tween(begin: 1.0, end: 0.0).chain(
          CurveTween(curve: Curves.easeInOutCubic),
        ),
        weight: 78,
      ),
    ]).animate(_controller);
    _controller.forward();
  }

  @override
  void didUpdateWidget(Welcome oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (widget.animationCycle != oldWidget.animationCycle) {
      _controller.forward(from: 0);
    }
  }

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final logo = const PulseRings(
      size: 168,
      child: PortalisLogo(size: 72),
    );
    final shareLogo = widget.onLogoTap == null
        ? logo
        : Tooltip(
            message: 'Share files',
            child: Semantics(
              button: true,
              label: 'Share files',
              child: GestureDetector(
                key: const Key('shareCollectionAction'),
                onTap: widget.onLogoTap,
                child: logo,
              ),
            ),
          );

    return Column(
      mainAxisSize: MainAxisSize.min,
      children: [
        shareLogo,
        const SizedBox(height: 22),
        SizeTransition(
          sizeFactor: _visibility,
          alignment: Alignment.topCenter,
          child: FadeTransition(
            key: const Key('homeWelcomeMessage'),
            opacity: _visibility,
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                CanvasTitle(
                  'Send anything to anybody',
                  size: widget.titleSize,
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
                if (widget.footer != null) ...[
                  const SizedBox(height: 26),
                  Text(
                    widget.footer!,
                    textAlign: TextAlign.center,
                    style: monoLabel(
                      size: 10.5,
                      color: AppColors.textGhost,
                    ),
                  ),
                ],
              ],
            ),
          ),
        ),
      ],
    );
  }
}
