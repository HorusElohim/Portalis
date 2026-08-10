import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/media_item.dart';

/// Paints only piece state supplied by the backend. When older/unavailable
/// telemetry has no ranges, the existing aggregate perimeter remains as the
/// honest fallback.
class MediaPieceFrame extends StatefulWidget {
  const MediaPieceFrame({
    super.key,
    required this.media,
    required this.color,
    required this.borderRadius,
    required this.child,
  });

  final MediaItem media;
  final Color color;
  final BorderRadius borderRadius;
  final Widget child;

  @override
  State<MediaPieceFrame> createState() => _MediaPieceFrameState();
}

class _MediaPieceFrameState extends State<MediaPieceFrame>
    with SingleTickerProviderStateMixin {
  late final AnimationController _pulse = AnimationController(
    vsync: this,
    duration: const Duration(milliseconds: 900),
    value: 1,
  );

  bool get _hasWorkers =>
      widget.media.pieceRuns.any((run) => run.isDownloading);

  @override
  void didChangeDependencies() {
    super.didChangeDependencies();
    _syncPulse();
  }

  @override
  void didUpdateWidget(covariant MediaPieceFrame oldWidget) {
    super.didUpdateWidget(oldWidget);
    _syncPulse();
  }

  void _syncPulse() {
    final shouldAnimate =
        _hasWorkers && !MediaQuery.disableAnimationsOf(context);
    if (shouldAnimate && !_pulse.isAnimating) {
      _pulse.repeat(reverse: true);
    } else if (!shouldAnimate && _pulse.isAnimating) {
      _pulse
        ..stop()
        ..value = 1;
    }
  }

  @override
  void dispose() {
    _pulse.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    final size = widget.media.sizeBytes;
    final segments = size <= 0
        ? const <PerimeterSegment>[]
        : [
            for (final run in widget.media.pieceRuns)
              PerimeterSegment(
                start: run.offsetBytes / size,
                extent: run.lengthBytes / size,
                active: run.isDownloading,
                workerCount: run.peers.length,
              ),
          ];
    return AnimatedBuilder(
      animation: _pulse,
      builder: (context, child) => PerimeterProgress(
        progress: widget.media.progress,
        color: widget.color,
        activeColor: Color.lerp(
          AppColors.signalDim,
          AppColors.signalSoft,
          _pulse.value,
        ),
        borderRadius: widget.borderRadius,
        segments: segments,
        child: child!,
      ),
      child: widget.child,
    );
  }
}
