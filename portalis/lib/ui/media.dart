// Part of the Portalis UI kit — see ui.dart.

import 'dart:io';

import 'package:flutter/material.dart';

import '../media/formats.dart';
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
  const MediaThumbnail({
    super.key,
    required this.media,
    this.borderRadius = 0,
    this.decodeSize = 160,
  });

  final MediaItem media;
  final double borderRadius;

  /// Longest side to decode the source image to, in logical pixels.
  ///
  /// Without this, `Image.file` decodes at the file's real resolution — a
  /// 12MP camera photo becomes ~48MB of raw RGBA to paint a tile a few dozen
  /// logical pixels across, and a grid of them can push Flutter's image
  /// cache into the hundreds of MB. The default suits this widget's usual
  /// job (a row icon or a grid tile); callers rendering it larger — the
  /// full-screen viewer — pass a bigger value. Same idea as the nav icon in
  /// `main.dart`, generalised because this widget renders at very different
  /// sizes depending on where it's used.
  final double decodeSize;

  @override
  Widget build(BuildContext context) {
    // Ask the registry what this type can do, rather than assuming every
    // image-kind file is decodable — HEIC is image-kind but only previews
    // because it was converted on the way in.
    final format = MediaFormats.resolve(media.label);
    if (media.isReady && format.preview == PreviewSupport.image) {
      final cacheWidth =
          (decodeSize * MediaQuery.devicePixelRatioOf(context)).round();
      return ClipRRect(
        borderRadius: BorderRadius.circular(borderRadius),
        child: Image.file(
          File(media.localPath!),
          fit: BoxFit.cover,
          cacheWidth: cacheWidth,
          errorBuilder: (context, error, stack) => PlaceholderTile(
            label: _typeLabel(media.label),
            borderRadius: borderRadius,
            kind: kindOf(media.label),
          ),
        ),
      );
    }
    return PlaceholderTile(
      label: _typeLabel(media.label),
      borderRadius: borderRadius,
      kind: kindOf(media.label),
    );
  }

  /// The file's *type*, not its name.
  ///
  /// A tile used to caption itself with the filename, which every surface
  /// showing tiles now prints underneath them anyway — so the name appeared
  /// twice, once truncated to nothing useful inside a 52pt square. The
  /// extension is the one thing a placeholder can add that the caption below
  /// it doesn't already say. Files with no extension get nothing rather than a
  /// repeat of their name.
  static String? _typeLabel(String name) {
    final ext = extensionOf(name);
    return ext.isEmpty ? null : ext.toUpperCase();
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
