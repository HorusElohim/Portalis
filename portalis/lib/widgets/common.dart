import 'dart:io';

import 'package:flutter/material.dart';
import '../media_kind.dart';
import '../models.dart';
import '../theme.dart';

/// Circular avatar with initials, matching the accent-800/600 avatar style.
class Avatar extends StatelessWidget {
  const Avatar({super.key, required this.initials, this.size = 30});

  final String initials;
  final double size;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: AppColors.accent800,
        shape: BoxShape.circle,
        border: Border.all(color: AppColors.accent600),
      ),
      child: Text(
        initials,
        style: TextStyle(
          color: AppColors.accent300,
          fontSize: size * 0.4,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

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

/// Outlined accent pill button, e.g. "＋ Share something".
class PillButton extends StatelessWidget {
  const PillButton({
    super.key,
    required this.label,
    required this.onTap,
    this.icon,
    this.filled = false,
    this.dim = false,
  });

  final String label;
  final VoidCallback? onTap;
  final Widget? icon;
  final bool filled;

  /// Use the dimmer neutral outline instead of the accent outline.
  final bool dim;

  @override
  Widget build(BuildContext context) {
    final color = dim ? AppColors.neutral300 : AppColors.accent300;
    final borderColor = dim ? AppColors.borderStrong : AppColors.accent;
    return Material(
      color: filled ? AppColors.accent : Colors.transparent,
      shape: StadiumBorder(
        side: BorderSide(color: filled ? AppColors.accent : borderColor),
      ),
      child: InkWell(
        customBorder: const StadiumBorder(),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[icon!, const SizedBox(width: 7)],
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: filled ? AppColors.bg : color,
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Placeholder tile standing in for real thumbnails/covers/media — shown
/// for anything not downloaded yet, and for any file type real thumbnails
/// don't apply to (video frames aren't extracted, audio/subtitles/other
/// files have no visual content at all). The icon communicates the file
/// type at a glance instead of every non-image tile looking identical.
class PlaceholderTile extends StatelessWidget {
  const PlaceholderTile({
    super.key,
    this.label,
    this.borderRadius = 0,
    this.kind = MediaKind.other,
  });

  final String? label;
  final double borderRadius;
  final MediaKind kind;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: CustomPaint(
        painter: _DiagonalStripePainter(),
        child: Center(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Icon(iconFor(kind), size: 26, color: AppColors.neutral400),
              if (label != null) ...[
                const SizedBox(height: 6),
                Padding(
                  padding: const EdgeInsets.symmetric(horizontal: 6),
                  child: Text(
                    label!,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    textAlign: TextAlign.center,
                    style: const TextStyle(
                      color: AppColors.neutral500,
                      fontSize: 10,
                      fontFamily: 'monospace',
                    ),
                  ),
                ),
              ],
            ],
          ),
        ),
      ),
    );
  }
}

/// A [MediaItem]'s thumbnail: the real downloaded image when one's ready,
/// falling back to [PlaceholderTile] otherwise (not downloaded yet, or not
/// an image — video/audio/subtitle/other files get a type icon instead,
/// since none of those have a real frame/cover to render here). Used
/// everywhere a media tile is shown — collection cards, grids, the media
/// viewer — so real and mock media render identically.
class MediaThumbnail extends StatelessWidget {
  const MediaThumbnail({super.key, required this.media, this.borderRadius = 0});

  final MediaItem media;
  final double borderRadius;

  @override
  Widget build(BuildContext context) {
    if (media.isReady && isImage(media.label)) {
      return ClipRRect(
        borderRadius: BorderRadius.circular(borderRadius),
        child: Image.file(
          File(media.localPath!),
          fit: BoxFit.cover,
          errorBuilder: (context, error, stack) => PlaceholderTile(
            label: media.label,
            borderRadius: borderRadius,
            kind: kindOf(media.label),
          ),
        ),
      );
    }
    return PlaceholderTile(
      label: media.label,
      borderRadius: borderRadius,
      kind: kindOf(media.label),
    );
  }
}

class _DiagonalStripePainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final bg = Paint()..color = const Color(0xFF1E2130);
    canvas.drawRect(Offset.zero & size, bg);
    final stripe = Paint()..color = const Color(0xFF232637);
    const gap = 16.0;
    for (double x = -size.height; x < size.width; x += gap) {
      final path = Path()
        ..moveTo(x, size.height)
        ..lineTo(x + size.height, 0)
        ..lineTo(x + size.height + 8, 0)
        ..lineTo(x + 8, size.height)
        ..close();
      canvas.drawPath(path, stripe);
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

/// Small "SECTION HEADER" style label.
class SectionLabel extends StatelessWidget {
  const SectionLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(
      text,
      style: const TextStyle(
        color: AppColors.neutral400,
        fontSize: 9.5,
        fontFamily: 'monospace',
        fontWeight: FontWeight.w500,
        letterSpacing: 1.2,
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

/// A back-chevron text button, e.g. "‹ Back".
class NavBackButton extends StatelessWidget {
  const NavBackButton({super.key, this.onTap});

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return TextButton(
      onPressed: onTap ?? () => Navigator.of(context).maybePop(),
      style: TextButton.styleFrom(
        foregroundColor: AppColors.accent300,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      ),
      child: const Text(
        '‹ Back',
        style: TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
      ),
    );
  }
}
