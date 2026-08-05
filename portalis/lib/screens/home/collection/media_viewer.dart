import 'dart:io';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';

import '../../../media/formats.dart';
import '../../../models.dart';
import '../../../services/collections.dart';
import '../../../theme.dart';
import '../../../ui/ui.dart';

class MediaViewerScreen extends StatefulWidget {
  const MediaViewerScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  /// Seeds, not sources of truth. Both are re-read from [Collections] on every
  /// rebuild so the figures on screen tick while a file downloads — held by
  /// value they froze at whatever they were when the tile was tapped, which is
  /// precisely when they are least interesting.
  final Collection collection;
  final MediaItem media;

  @override
  State<MediaViewerScreen> createState() => _MediaViewerScreenState();
}

class _MediaViewerScreenState extends State<MediaViewerScreen> {
  VideoPlayerController? _videoController;
  bool _videoFailed = false;

  /// The technical rows, shown in place. They used to be a pushed screen —
  /// two taps and a screen transition to read an info hash, and a third to get
  /// back to the picture.
  bool _showDetails = false;

  Collection get _collection =>
      Collections.instance.byId(widget.collection.id) ?? widget.collection;

  /// Matched on info-hash *and* name: one manifest entry can hold several
  /// files, so the hash alone doesn't identify a file.
  MediaItem get _media {
    for (final m in _collection.media) {
      if (m.infoHash == widget.media.infoHash &&
          m.label == widget.media.label) {
        return m;
      }
    }
    return widget.media;
  }

  /// The path the current controller was built for, so [_syncVideo] can tell
  /// a genuine change from another identical poll.
  String? _playingPath;

  @override
  void initState() {
    super.initState();
    _syncVideo();
    // Not `didUpdateWidget`: nothing rebuilds this widget with new arguments
    // any more — the file arrives through the cache, so that is what has to be
    // watched. A video that finishes downloading while it is open now starts
    // playing instead of staying a thumbnail until the screen is reopened.
    Collections.instance.addListener(_syncVideo);
  }

  @override
  void dispose() {
    Collections.instance.removeListener(_syncVideo);
    _disposeVideo();
    super.dispose();
  }

  /// Whether *this* file can play inline, per the registry — not merely
  /// whether it is video. MKV and AVI are video-kind but the platform players
  /// handle them inconsistently, so they open externally instead of showing
  /// a black frame here.
  bool get _isPlayableVideo =>
      _media.isReady &&
      MediaFormats.resolve(_media.label).preview == PreviewSupport.player;

  void _syncVideo() {
    final path = _isPlayableVideo ? _media.localPath : null;
    if (path == _playingPath) return;
    _disposeVideo();
    _playingPath = path;
    if (path != null) _maybeInitVideo();
  }

  void _maybeInitVideo() {
    if (!_isPlayableVideo) return;
    final controller = VideoPlayerController.file(File(_media.localPath!));
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
    _playingPath = null;
  }

  Future<void> _openExternally() async {
    final path = _media.localPath;
    if (path == null) return;
    final ok = await launchUrl(Uri.file(path));
    if (!ok && mounted) {
      showToast(context, 'Couldn\'t open ${_media.label}',
          severity: ToastSeverity.error);
    }
  }

  @override
  Widget build(BuildContext context) {
    // Rebuilt on every poll, so everything below is the current answer rather
    // than the answer at the moment this screen opened.
    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final collection = _collection;
    final media = _media;

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
                        label: _showDetails ? 'Less' : 'Details',
                        onTap: () =>
                            setState(() => _showDetails = !_showDetails),
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
                      style: AppText.cardTitle(),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      // Where the file lives, and nothing else: the peer count
                      // and the rates belong to the live block below, and
                      // saying them twice made neither one authoritative.
                      media.isReady
                          ? collection.name
                          : '${collection.name} · downloading',
                      style: AppText.caption(color: AppColors.textDim),
                    ),
                    const SizedBox(height: 12),
                    // Always on screen, never behind a tap: this is the whole
                    // reason someone opens a file that is still arriving.
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
                            : 'Not fetched — size unknown until it starts',
                      ),
                    ),
                    // AnimatedSize over a conditional child, not a cross-fade:
                    // a cross-fade keeps the hidden half in the tree, laying
                    // out and rebuilding rows nobody is looking at on every
                    // poll.
                    AnimatedSize(
                      duration: const Duration(milliseconds: 180),
                      curve: Curves.easeOutCubic,
                      alignment: Alignment.topCenter,
                      child: _showDetails
                          ? _Details(collection: collection, media: media)
                          : const SizedBox(width: double.infinity),
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
    final media = _media;
    final controller = _videoController;

    if (_isPlayableVideo &&
        !_videoFailed &&
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
          // Shown up to near-fullscreen here, unlike this widget's usual grid
          // tile / row-icon job — decode sharper accordingly.
          MediaThumbnail(media: media, borderRadius: 8, decodeSize: 720),
          if (_isPlayableVideo && !_videoFailed)
            // Video still initializing.
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
                      color: AppColors.signalSoft, weight: FontWeight.w500),
                ),
              ),
            ),
        ],
      ),
    );
  }
}

/// The rows that used to be a screen of their own: identifiers and state,
/// worth having but not worth looking at every time. Live like everything
/// else here.
class _Details extends StatelessWidget {
  const _Details({required this.collection, required this.media});

  final Collection collection;
  final MediaItem media;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.only(top: 14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const SectionLabel('DETAILS'),
          const SizedBox(height: 6),
          InfoRow(label: 'Collection', value: collection.name),
          // The batch this file was added as — several files share one, and
          // it is the only place that grouping is visible from here.
          if (media.entryLabel != media.label)
            InfoRow(label: 'Added as', value: media.entryLabel),
          InfoRow(
            label: 'State',
            value: collection.state.isEmpty ? 'Unknown' : collection.state,
          ),
          if (media.sizeBytes > 0)
            InfoRow(label: 'Size', value: formatBytesPrecise(media.sizeBytes)),
          // The info hash of the *torrent this file came from*: a shared
          // collection has one per manifest entry, so it belongs to the file,
          // not the collection.
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
