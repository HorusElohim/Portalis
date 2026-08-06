// Collection media presentation shared by list, grid and viewer surfaces.

import 'dart:io';

import 'package:flutter/material.dart';
import 'package:video_player/video_player.dart';

import '../../../theme.dart';
import '../application/media_formats.dart';
import '../domain/media_item.dart';

/// Placeholder tile standing in for real thumbnails/covers/media — shown
/// for anything not downloaded yet, and for file types without an in-app
/// frame renderer. The icon communicates the file type at a glance instead
/// of every non-image tile looking identical.
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
                    style: monoLabel(
                        size: 10, color: AppColors.textGhost, letterSpacing: 0),
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
/// previewable). Video files render their first frame without starting
/// playback. Used
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
    if (media.isReady && format.preview == PreviewSupport.player) {
      return _VideoThumbnail(media: media, borderRadius: borderRadius);
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

class _VideoThumbnail extends StatefulWidget {
  const _VideoThumbnail({required this.media, required this.borderRadius});

  final MediaItem media;
  final double borderRadius;

  @override
  State<_VideoThumbnail> createState() => _VideoThumbnailState();
}

class _VideoThumbnailState extends State<_VideoThumbnail> {
  VideoPlayerController? _controller;
  bool _failed = false;

  @override
  void initState() {
    super.initState();
    _initialize();
  }

  Future<void> _initialize() async {
    final path = widget.media.localPath;
    if (path == null) return;
    final controller = VideoPlayerController.file(File(path));
    _controller = controller;
    try {
      await controller.initialize();
      if (!controller.value.isInitialized) {
        throw StateError(
          controller.value.errorDescription ?? 'Video failed to initialize',
        );
      }
      try {
        await controller.setVolume(0);
      } catch (_) {
        // Muting a thumbnail is optional; a volume failure must not hide a
        // valid video frame.
      }
      if (mounted) setState(() {});
    } catch (_) {
      if (mounted) setState(() => _failed = true);
      await controller.dispose();
    }
  }

  @override
  void dispose() {
    _controller?.dispose();
    super.dispose();
  }

  @override
  void didUpdateWidget(covariant _VideoThumbnail oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.media.localPath == widget.media.localPath) return;
    _controller?.dispose();
    _controller = null;
    _failed = false;
    _initialize();
  }

  @override
  Widget build(BuildContext context) {
    final controller = _controller;
    if (_failed || controller == null || !controller.value.isInitialized) {
      return PlaceholderTile(
        label: MediaThumbnail._typeLabel(widget.media.label),
        borderRadius: widget.borderRadius,
        kind: kindOf(widget.media.label),
      );
    }

    return ClipRRect(
      borderRadius: BorderRadius.circular(widget.borderRadius),
      child: LayoutBuilder(
        builder: (context, constraints) {
          final aspect = controller.value.aspectRatio <= 0
              ? 16 / 9
              : controller.value.aspectRatio;
          final tileAspect = constraints.maxWidth / constraints.maxHeight;
          final width = aspect > tileAspect
              ? constraints.maxHeight * aspect
              : constraints.maxWidth;
          final height = width / aspect;
          return FittedBox(
            fit: BoxFit.cover,
            clipBehavior: Clip.hardEdge,
            child: SizedBox(
              width: width,
              height: height,
              child: VideoPlayer(controller),
            ),
          );
        },
      ),
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
