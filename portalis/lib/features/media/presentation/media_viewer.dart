import 'package:flutter/material.dart';
import 'package:video_player/video_player.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../collections/domain/collection.dart';
import '../../collections/presentation/collection_presentation.dart';
import '../domain/media_item.dart';
import 'media_thumbnail.dart';

/// Renders a collection item without owning navigation, backend state, or the
/// video controller lifecycle.
class CollectionMediaViewer extends StatelessWidget {
  const CollectionMediaViewer({
    super.key,
    required this.collection,
    required this.media,
    required this.isPlayableVideo,
    required this.videoFailed,
    required this.videoController,
    required this.onClose,
    required this.onOpenExternally,
  });

  final Collection collection;
  final MediaItem media;
  final bool isPlayableVideo;
  final bool videoFailed;
  final VideoPlayerController? videoController;
  final VoidCallback onClose;
  final VoidCallback onOpenExternally;

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: AppColors.viewerBg,
        body: SafeArea(
          child: Column(
            children: [
              Padding(
                padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
                child: Row(
                  mainAxisAlignment: MainAxisAlignment.spaceBetween,
                  children: [
                    _CircleButton(icon: Icons.close, onTap: onClose),
                    Row(
                      children: [
                        if (media.isReady) ...[
                          _CircleButton(
                            icon: Icons.open_in_new_rounded,
                            onTap: onOpenExternally,
                          ),
                          const SizedBox(width: 8),
                        ],
                      ],
                    ),
                  ],
                ),
              ),
              Expanded(
                child: LayoutBuilder(
                  builder: (context, viewport) {
                    final previewHeight =
                        (viewport.maxHeight * 0.72).clamp(300.0, 760.0).toDouble();
                    return Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 16),
                      child: SingleChildScrollView(
                        child: Column(
                          children: [
                            const SizedBox(height: 12),
                            SizedBox(
                              width: double.infinity,
                              height: previewHeight,
                              child: MediaPreview(
                                media: media,
                                isPlayableVideo: isPlayableVideo,
                                videoFailed: videoFailed,
                                videoController: videoController,
                              ),
                            ),
                            const SizedBox(height: 14),
                            Text(media.label, style: AppText.cardTitle()),
                            const SizedBox(height: 3),
                            Text(
                              media.isReady
                                  ? collection.name
                                  : '${collection.name} \u00b7 downloading',
                              style: AppText.caption(color: AppColors.textDim),
                            ),
                            const SizedBox(height: 12),
                            Align(
                              alignment: Alignment.centerLeft,
                              child: TransferFacts(
                                progress: media.progress,
                                downloadedBytes: media.downloadedBytes,
                                totalBytes: media.sizeBytes,
                                downloadMbps: collection.downloadMbps,
                                uploadMbps: collection.uploadMbps,
                                livePeers: collection.livePeers,
                                etaLabel: collection.etaLabel,
                                color: collection.hue,
                                pendingLabel: media.fetched
                                    ? null
                                    : 'Not fetched \u2014 size unknown until it starts',
                              ),
                            ),
                            _MediaDetails(collection: collection, media: media),
                            const SizedBox(height: 14),
                          ],
                        ),
                      ),
                    );
                  },
                ),
              ),
            ],
          ),
        ),
      );
}

/// The in-app preview surface. Additional preview kinds can be added here
/// without changing collection screens or media navigation.
class MediaPreview extends StatelessWidget {
  const MediaPreview({
    super.key,
    required this.media,
    required this.isPlayableVideo,
    required this.videoFailed,
    required this.videoController,
  });

  final MediaItem media;
  final bool isPlayableVideo;
  final bool videoFailed;
  final VideoPlayerController? videoController;

  @override
  Widget build(BuildContext context) {
    final controller = videoController;
    if (isPlayableVideo &&
        !videoFailed &&
        controller != null &&
        controller.value.isInitialized) {
      return AspectRatio(
        aspectRatio: controller.value.aspectRatio,
        child: ClipRRect(
          borderRadius: BorderRadius.circular(AppRadius.tight),
          child: _VideoPlayerView(controller: controller),
        ),
      );
    }

    return AspectRatio(
      aspectRatio: 16 / 10,
      child: Stack(
        alignment: Alignment.center,
        children: [
          MediaThumbnail(media: media, borderRadius: 8, decodeSize: 720),
          if (isPlayableVideo && !videoFailed)
            const SizedBox(
              width: 32,
              height: 32,
              child: CircularProgressIndicator(
                strokeWidth: 2.5,
                valueColor: AlwaysStoppedAnimation(AppColors.signalSoft),
              ),
            )
          else if (!media.isReady)
            Container(
              width: 62,
              height: 62,
              decoration: BoxDecoration(
                shape: BoxShape.circle,
                color: AppColors.surfaceDeep.withValues(alpha: 0.85),
                border: Border.all(color: AppColors.signalDim),
              ),
              child: Center(
                child: Text(
                  '${(media.progress * 100).toStringAsFixed(0)}%',
                  style: AppText.body(
                    color: AppColors.signalSoft,
                    weight: FontWeight.w500,
                  ),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

class _MediaDetails extends StatelessWidget {
  const _MediaDetails({required this.collection, required this.media});

  final Collection collection;
  final MediaItem media;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 14),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            const SectionLabel('DETAILS'),
            const SizedBox(height: 6),
            InfoRow(label: 'Collection', value: collection.name),
            if (media.entryLabel != media.label)
              InfoRow(label: 'Added as', value: media.entryLabel),
            InfoRow(
              label: 'State',
              value: collection.state.isEmpty ? 'Unknown' : collection.state,
            ),
            if (media.sizeBytes > 0)
              InfoRow(label: 'Size', value: formatBytesPrecise(media.sizeBytes)),
            if (media.infoHash.isNotEmpty)
              InfoRow(
                label: 'Info hash',
                value: media.infoHash,
                monospace: true,
                copyable: true,
              ),
          ],
        ),
      );
}

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
  void didUpdateWidget(covariant _VideoPlayerView oldWidget) {
    super.didUpdateWidget(oldWidget);
    if (oldWidget.controller == widget.controller) return;
    oldWidget.controller.removeListener(_onTick);
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
                color: AppColors.surfaceDeep.withValues(alpha: 0.75),
                border: Border.all(color: AppColors.signalDim),
              ),
              child: const Icon(
                Icons.play_arrow_rounded,
                color: AppColors.signalSoft,
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
                playedColor: AppColors.signal,
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
  Widget build(BuildContext context) => Material(
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
