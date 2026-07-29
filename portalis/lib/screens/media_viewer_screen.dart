import 'dart:io';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';

import '../media_kind.dart';
import '../models.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'media_details_screen.dart';

class MediaViewerScreen extends StatefulWidget {
  const MediaViewerScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  final Collection collection;
  final MediaItem media;

  @override
  State<MediaViewerScreen> createState() => _MediaViewerScreenState();
}

class _MediaViewerScreenState extends State<MediaViewerScreen> {
  VideoPlayerController? _videoController;
  bool _videoFailed = false;

  @override
  void initState() {
    super.initState();
    _maybeInitVideo();
  }

  @override
  void didUpdateWidget(covariant MediaViewerScreen oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.media.localPath != widget.media.localPath) {
      _disposeVideo();
      _maybeInitVideo();
    }
  }

  @override
  void dispose() {
    _disposeVideo();
    super.dispose();
  }

  bool get _isPlayableVideo => widget.media.isReady && isVideo(widget.media.label);

  void _maybeInitVideo() {
    if (!_isPlayableVideo) return;
    final controller = VideoPlayerController.file(File(widget.media.localPath!));
    _videoController = controller;
    controller.initialize().then((_) {
      if (mounted) setState(() {});
    }).catchError((_) {
      if (mounted) setState(() => _videoFailed = true);
    });
  }

  void _disposeVideo() {
    _videoController?.dispose();
    _videoController = null;
    _videoFailed = false;
  }

  Future<void> _openExternally() async {
    final path = widget.media.localPath;
    if (path == null) return;
    final ok = await launchUrl(Uri.file(path));
    if (!ok && mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Couldn\'t open ${widget.media.label}')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final collection = widget.collection;
    final media = widget.media;
    final peerLabel = collection.collaboratorCount == 1
        ? '1 peer'
        : '${collection.collaboratorCount} peers';

    return Scaffold(
      backgroundColor: AppColors.viewerBg,
      body: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  _CircleButton(
                    icon: Icons.close,
                    onTap: () => Navigator.of(context).pop(),
                  ),
                  Row(
                    children: [
                      if (media.isReady) ...[
                        _CircleButton(
                          icon: Icons.open_in_new_rounded,
                          onTap: _openExternally,
                        ),
                        const SizedBox(width: 8),
                      ],
                      PillButton(
                        label: 'Details',
                        onTap: () => Navigator.of(context).push(
                          MaterialPageRoute(
                            builder: (_) => MediaDetailsScreen(
                              collection: collection,
                              media: media,
                            ),
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Column(
                  children: [
                    Expanded(
                      child: Center(
                        child: _buildPreview(),
                      ),
                    ),
                    const SizedBox(height: 14),
                    Text(
                      media.label,
                      style: const TextStyle(
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      media.isReady
                          ? '${collection.name} · streams from $peerLabel'
                          : '${collection.name} · downloading, $peerLabel connected',
                      style: const TextStyle(
                        fontSize: 11.5,
                        color: AppColors.neutral400,
                      ),
                    ),
                    const SizedBox(height: 14),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }

  Widget _buildPreview() {
    final media = widget.media;
    final controller = _videoController;

    if (_isPlayableVideo && !_videoFailed && controller != null && controller.value.isInitialized) {
      return AspectRatio(
        aspectRatio: controller.value.aspectRatio,
        child: ClipRRect(
          borderRadius: BorderRadius.circular(8),
          child: _VideoPlayerView(controller: controller),
        ),
      );
    }

    return AspectRatio(
      aspectRatio: 16 / 10,
      child: Stack(
        alignment: Alignment.center,
        children: [
          MediaThumbnail(media: media, borderRadius: 8),
          if (_isPlayableVideo && !_videoFailed)
            // Video still initializing.
            const SizedBox(
              width: 32,
              height: 32,
              child: CircularProgressIndicator(
                strokeWidth: 2.5,
                valueColor: AlwaysStoppedAnimation(AppColors.accent300),
              ),
            )
          else if (!media.isReady)
            Container(
              width: 62,
              height: 62,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: AppColors.bg.withValues(alpha: 0.85),
                border: Border.all(color: AppColors.accent600),
              ),
              child: Center(
                child: Text(
                  '${(media.progress * 100).toStringAsFixed(0)}%',
                  style: const TextStyle(
                    color: AppColors.accent300,
                    fontSize: 13,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// Tap-to-play/pause with a lightweight scrub bar — enough to actually use
/// the player, not a full custom-controls redesign.
class _VideoPlayerView extends StatefulWidget {
  const _VideoPlayerView({required this.controller});

  final VideoPlayerController controller;

  @override
  State<_VideoPlayerView> createState() => _VideoPlayerViewState();
}

class _VideoPlayerViewState extends State<_VideoPlayerView> {
  @override
  void initState() {
    super.initState();
    widget.controller.addListener(_onTick);
  }

  @override
  void dispose() {
    widget.controller.removeListener(_onTick);
    super.dispose();
  }

  void _onTick() {
    if (mounted) setState(() {});
  }

  @override
  Widget build(BuildContext context) {
    final controller = widget.controller;
    return GestureDetector(
      onTap: () {
        controller.value.isPlaying ? controller.pause() : controller.play();
      },
      child: Stack(
        alignment: Alignment.center,
        children: [
          VideoPlayer(controller),
          if (!controller.value.isPlaying)
            Container(
              width: 62,
              height: 62,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: AppColors.bg.withValues(alpha: 0.75),
                border: Border.all(color: AppColors.accent600),
              ),
              child: const Icon(
                Icons.play_arrow_rounded,
                color: AppColors.accent300,
                size: 32,
              ),
            ),
          Positioned(
            left: 0,
            right: 0,
            bottom: 0,
            child: VideoProgressIndicator(
              controller,
              allowScrubbing: true,
              padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
              colors: const VideoProgressColors(
                playedColor: AppColors.accent,
                bufferedColor: AppColors.borderStrong,
                backgroundColor: Colors.black26,
              ),
            ),
          ),
        ],
      ),
    );
  }
}

class _CircleButton extends StatelessWidget {
  const _CircleButton({required this.icon, required this.onTap});

  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.white.withValues(alpha: 0.06),
      shape: CircleBorder(side: BorderSide(color: AppColors.borderStrong)),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 34,
          height: 34,
          child: Icon(icon, size: 18, color: AppColors.text),
        ),
      ),
    );
  }
}
