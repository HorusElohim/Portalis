// Cross-feature ambient design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';

/// A living gradient that responds to the app's actual state.
///
/// The rule the palette rests on applies here too: this **only moves when
/// data is moving**. With nothing in flight it is a single static gradient
/// with no ticker at all — not a paused animation, no ticker is created — so
/// an idle app costs nothing to display. When bytes are flowing the gradient
/// drifts, and its warmth scales with real throughput.
///
/// That coupling is the point. A background that animates constantly is both
/// a lie (it implies activity) and a battery cost; one that animates only
/// during transfer is ambient feedback you get for free while the radio is
/// already awake.
///
/// Honours [MediaQueryData.disableAnimations], so reduce-motion users get the
/// static form regardless of activity.
class AmbientBackground extends StatefulWidget {
  const AmbientBackground({
    super.key,
    required this.child,
    this.intensity = 0,
    this.accent = AppColors.signal,
  });

  final Widget child;

  /// 0 = nothing moving, 1 = saturated. Callers map real throughput onto
  /// this; see [Glow.intensityForRate].
  final double intensity;

  /// Which colour the wash takes. Ember when the activity is a torrent, so
  /// the background agrees with the row that caused it.
  final Color accent;

  @override
  State<AmbientBackground> createState() => _AmbientBackgroundState();
}

class _AmbientBackgroundState extends State<AmbientBackground>
    with SingleTickerProviderStateMixin {
  AnimationController? _controller;

  bool get _shouldAnimate =>
      widget.intensity > 0 && !MediaQuery.disableAnimationsOf(context);

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncTicker();
  }

  @override
  void didUpdateWidget(covariant AmbientBackground oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.intensity != widget.intensity) _syncTicker();
  }

  /// Creates the controller on first need and disposes it the moment the app
  /// goes quiet, so an idle screen holds no ticker and schedules no frames.
  void _syncTicker() {
    if (_shouldAnimate) {
      _controller ??= AnimationController(
        vsync: this,
        // Very slow: this is ambience, not motion design. A long period also
        // means the compositor has little to do between frames.
        duration: const Duration(seconds: 14),
      )..repeat();
    } else {
      _controller?.dispose();
      _controller = null;
    }
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final controller = _controller;
    return Stack(
      children: [
        Positioned.fill(
          // Isolated from the content above it: the gradient repainting must
          // never mark the widget tree above as dirty.
          child: RepaintBoundary(
            child: controller == null
                ? _Wash(
                    t: 0, intensity: widget.intensity, accent: widget.accent)
                : AnimatedBuilder(
                    animation: controller,
                    builder: (context, _) => _Wash(
                      t: controller.value,
                      intensity: widget.intensity,
                      accent: widget.accent,
                    ),
                  ),
          ),
        ),
        widget.child,
      ],
    );
  }
}

/// The gradient itself. Two soft radial pools that drift in opposition,
/// over the app's base colour.
class _Wash extends StatelessWidget {
  const _Wash({
    required this.t,
    required this.intensity,
    required this.accent,
  });

  final double t;
  final double intensity;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    // A gentle figure-of-eight rather than a circle, so the two pools never
    // settle into an obvious loop.
    final phase = t * 2 * 3.14159265;
    final dx = 0.35 * _sin(phase);
    final dy = 0.22 * _sin(phase * 2);

    // Even at full intensity this stays a wash: the content has to remain
    // the brightest thing on screen.
    final top = (0.10 + 0.10 * intensity).clamp(0.0, 0.22);
    final bottom = (0.04 + 0.07 * intensity).clamp(0.0, 0.14);

    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: RadialGradient(
          center: Alignment(dx, -0.85 + dy),
          radius: 1.15,
          colors: [
            accent.withValues(alpha: top),
            AppColors.surfaceDeep.withValues(alpha: 0),
          ],
          stops: const [0, 0.72],
        ),
      ),
      child: DecoratedBox(
        decoration: BoxDecoration(
          gradient: RadialGradient(
            center: Alignment(-dx * 0.8, 0.9 - dy),
            radius: 1.0,
            colors: [
              // The second pool leans to the deeper green, which reads as
              // canopy rather than glow.
              AppColors.signalDim.withValues(alpha: bottom),
              AppColors.surfaceDeep.withValues(alpha: 0),
            ],
            stops: const [0, 0.68],
          ),
        ),
        child: const SizedBox.expand(),
      ),
    );
  }

  /// Small-angle-free sine without importing dart:math for one call.
  static double _sin(double x) {
    // Normalise into [-pi, pi] for the polynomial approximation below.
    const twoPi = 6.283185307179586;
    const pi = 3.141592653589793;
    var v = x % twoPi;
    if (v > pi) v -= twoPi;
    // Bhaskara I's approximation — accurate to ~0.2%, which is far beyond
    // what a background gradient can show, and avoids a libm call per frame.
    final abs = v < 0 ? -v : v;
    final num = 16 * abs * (pi - abs);
    final den = 5 * pi * pi - 4 * abs * (pi - abs);
    final result = num / den;
    return v < 0 ? -result : result;
  }
}
