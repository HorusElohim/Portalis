import 'dart:io';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';
import 'package:video_player/video_player.dart';

import '../../../app/app_controllers.dart';
import '../../../design/design.dart';
import '../../collections/domain/collection.dart';
import '../application/media_formats.dart';
import '../domain/media_item.dart';
import 'media_viewer.dart';

/// Coordinates live collection state and the lifetime of an inline player.
class MediaViewerScreen extends StatefulWidget {
  const MediaViewerScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  /// Seeds rather than sources of truth. Current values are looked up while
  /// the view is open so download progress remains live.
  final Collection collection;
  final MediaItem media;

  @override
  State<MediaViewerScreen> createState() => _MediaViewerScreenState();
}

class _MediaViewerScreenState extends State<MediaViewerScreen> {
  VideoPlayerController? _videoController;
  String? _playingPath;
  bool _videoFailed = false;

  Collection get _collection =>
      AppControllers.collections.byId(widget.collection.id) ?? widget.collection;

  MediaItem get _media {
    for (final media in _collection.media) {
      if (media.infoHash == widget.media.infoHash &&
          media.label == widget.media.label) {
        return media;
      }
    }
    return widget.media;
  }

  bool get _isPlayableVideo =>
      _media.isReady &&
      MediaFormats.resolve(_media.label).preview == PreviewSupport.player;

  @override
  void initState() {
    super.initState();
    _syncVideo();
    AppControllers.collections.addListener(_syncVideo);
  }

  @override
  void dispose() {
    AppControllers.collections.removeListener(_syncVideo);
    _disposeVideo();
    super.dispose();
  }

  void _syncVideo() {
    final path = _isPlayableVideo ? _media.localPath : null;
    if (path == _playingPath) return;

    _disposeVideo();
    _playingPath = path;
    if (path != null) _initializeVideo(path);
  }

  void _initializeVideo(String path) {
    final controller = VideoPlayerController.file(File(path));
    _videoController = controller;
    controller.initialize().then((_) {
      if (!mounted || _videoController != controller) return;
      if (!controller.value.isInitialized) {
        setState(() => _videoFailed = true);
        return;
      }
      setState(() {});
    }).catchError((_) {
      if (!mounted || _videoController != controller) return;
      setState(() => _videoFailed = true);
    });
  }

  void _disposeVideo() {
    _videoController?.dispose();
    _videoController = null;
    _playingPath = null;
    _videoFailed = false;
  }

  Future<void> _openExternally() async {
    final path = _media.localPath;
    if (path == null) return;

    final opened = await launchUrl(Uri.file(path));
    if (!opened && mounted) {
      showToast(
        context,
        'Couldn\'t open ${_media.label}',
        severity: ToastSeverity.error,
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) => CollectionMediaViewer(
        collection: _collection,
        media: _media,
        isPlayableVideo: _isPlayableVideo,
        videoFailed: _videoFailed,
        videoController: _videoController,
        onClose: () => Navigator.of(context).pop(),
        onOpenExternally: _openExternally,
      ),
    );
  }
}
