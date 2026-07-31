// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../theme.dart';

/// Pulsing "live copies" indicator dot, used next to a copies label.
class LiveDot extends StatefulWidget {
  const LiveDot({super.key, required this.color, this.size = 8});

  final Color color;
  final double size;

  @override
  State<LiveDot> createState() => _LiveDotState();
}

class _LiveDotState extends State<LiveDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.size,
      height: widget.size,
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) {
          final t = _controller.value;
          return Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.center,
            children: [
              Opacity(
                opacity: (0.55 * (1 - t)).clamp(0.0, 1.0),
                child: Transform.scale(
                  scale: 1 + t * 1.1,
                  child: _dot(widget.color),
                ),
              ),
              _dot(widget.color),
            ],
          );
        },
      ),
    );
  }

  Widget _dot(Color color) => Container(
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      );
}

/// Live copies indicator: pulsing dot + colored label.
class CopiesIndicator extends StatelessWidget {
  const CopiesIndicator({
    super.key,
    required this.color,
    required this.label,
    this.fontSize = 11,
  });

  final Color color;
  final String label;
  final double fontSize;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        LiveDot(color: color),
        const SizedBox(width: 7),
        Flexible(
          child: Text(
            label,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: color,
              fontSize: fontSize,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
      ],
    );
  }
}

/// Concentric outward-radiating rings — the first-run "listening" motif.
/// Purely decorative, so it states nothing about the network; it sits behind
/// copy that makes the actual claim.
class PulseRings extends StatefulWidget {
  const PulseRings({super.key, required this.child, this.size = 150});

  final Widget child;
  final double size;

  @override
  State<PulseRings> createState() => _PulseRingsState();
}

class _PulseRingsState extends State<PulseRings>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 3400),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.size,
      height: widget.size,
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) {
          return Stack(
            alignment: Alignment.center,
            children: [
              // Three rings a third of a cycle apart, so one is always
              // mid-flight and the motif never reads as stalled.
              for (var i = 0; i < 3; i++)
                _ring((_controller.value + i / 3) % 1.0),
              widget.child,
            ],
          );
        },
      ),
    );
  }

  Widget _ring(double t) {
    return Opacity(
      opacity: (0.55 * (1 - t)).clamp(0.0, 1.0),
      child: Transform.scale(
        scale: 0.7 + t * 1.2,
        child: Container(
          width: widget.size * 0.5,
          height: widget.size * 0.5,
          decoration: BoxDecoration(
            shape: BoxShape.circle,
            border: Border.all(color: AppColors.signal.withValues(alpha: 0.5)),
          ),
        ),
      ),
    );
  }
}

/// Traces a colored ring around [child]'s rounded-rect perimeter, clockwise
/// from top-left, proportional to [progress]. Shared by Home's collection
/// cards and the collection detail screen's media tiles — same download
/// indicator wherever something has partial progress. A finished item
/// ([progress] >= 1.0) skips painting entirely so it just keeps its normal
/// static border underneath.
class PerimeterProgress extends StatelessWidget {
  const PerimeterProgress({
    super.key,
    required this.progress,
    required this.color,
    required this.borderRadius,
    required this.child,
  });

  final double progress;
  final Color color;
  final BorderRadius borderRadius;
  final Widget child;

  @override
  Widget build(BuildContext context) {
    if (progress >= 1.0) return child;
    return CustomPaint(
      foregroundPainter: _PerimeterProgressPainter(
        progress: progress,
        color: color,
        borderRadius: borderRadius,
      ),
      child: child,
    );
  }
}

class _PerimeterProgressPainter extends CustomPainter {
  _PerimeterProgressPainter({
    required this.progress,
    required this.color,
    required this.borderRadius,
  });

  final double progress;
  final Color color;
  final BorderRadius borderRadius;

  static const _strokeWidth = 2.5;

  @override
  void paint(Canvas canvas, Size size) {
    // Stroking centers the line on the path, so a path drawn flush with the
    // widget's own bounds would render half of it outside those bounds —
    // inset by half the stroke width so the whole ring lands on-screen
    // regardless of how an ancestor clips.
    final rect = Rect.fromLTWH(
      _strokeWidth / 2,
      _strokeWidth / 2,
      size.width - _strokeWidth,
      size.height - _strokeWidth,
    );
    if (rect.width <= 0 || rect.height <= 0) return;
    final rrect = borderRadius.toRRect(rect);
    // PathMetrics is single-use (backed by a native iterator) — checking
    // `.isEmpty` and then reading `.first` are two separate traversals, and
    // the first one silently consumes the only metric, leaving `.first` to
    // find nothing and throw. Pull the iterator once instead.
    final iterator = (Path()..addRRect(rrect)).computeMetrics().iterator;
    if (!iterator.moveNext()) return;
    final metric = iterator.current;
    final extracted = metric.extractPath(0, metric.length * progress.clamp(0.0, 1.0));
    canvas.drawPath(
      extracted,
      Paint()
        ..color = color
        ..style = PaintingStyle.stroke
        ..strokeWidth = _strokeWidth
        ..strokeCap = StrokeCap.round,
    );
  }

  @override
  bool shouldRepaint(covariant _PerimeterProgressPainter oldDelegate) =>
      oldDelegate.progress != progress || oldDelegate.color != color;
}
