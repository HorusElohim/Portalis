// Part of the Portalis UI kit — see ui.dart.

import 'dart:io';

import 'package:flutter/material.dart';

import '../media_kind.dart';
import '../models.dart';
import '../theme.dart';

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
              Icon(iconFor(kind), size: 26, color: AppColors.textDim),
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
                      color: AppColors.textGhost,
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
/// viewer — so a tile looks the same wherever it appears.
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
    final bg = Paint()..color = const Color(0xFF16211F);
    canvas.drawRect(Offset.zero & size, bg);
    final stripe = Paint()..color = const Color(0xFF1D2926);
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
