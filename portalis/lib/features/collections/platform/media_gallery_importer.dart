import 'package:flutter/foundation.dart';
import 'package:gal/gal.dart';

import '../../../media/formats.dart';
import '../domain/collection.dart';

/// A platform-owned side effect after media becomes complete. The transfer
/// controller asks for an import; it never knows Photos/MediaStore details.
abstract interface class MediaGalleryImporter {
  Future<void> importReadyMedia(Iterable<Collection> collections);
}

MediaGalleryImporter mediaGalleryImporterForCurrentPlatform() {
  final isMobile = defaultTargetPlatform == TargetPlatform.iOS ||
      defaultTargetPlatform == TargetPlatform.android;
  return isMobile ? MobileMediaGalleryImporter() : const NoopMediaGalleryImporter();
}

class NoopMediaGalleryImporter implements MediaGalleryImporter {
  const NoopMediaGalleryImporter();

  @override
  Future<void> importReadyMedia(Iterable<Collection> collections) async {}
}

/// Serialises calls into the native photo library and remembers successful or
/// rejected attempts for the lifetime of the app, preventing an OS permission
/// prompt from being retried every polling tick.
class MobileMediaGalleryImporter implements MediaGalleryImporter {
  final Set<String> _handled = {};
  bool _importing = false;

  @override
  Future<void> importReadyMedia(Iterable<Collection> collections) async {
    if (_importing) return;
    _importing = true;
    try {
      for (final collection in collections) {
        for (final media in collection.media) {
          final path = media.localPath;
          if (path == null || !media.isReady || _handled.contains(_key(media))) {
            continue;
          }
          if (!isImage(media.label) && !isVideo(media.label)) continue;

          _handled.add(_key(media));
          try {
            if (isImage(media.label)) {
              await Gal.putImage(path, album: collection.name);
            } else {
              await Gal.putVideo(path, album: collection.name);
            }
          } catch (_) {
            // The source remains available in Portalis. A retry would spam a
            // denied permission prompt, so this attempt is intentionally final.
          }
        }
      }
    } finally {
      _importing = false;
    }
  }

  String _key(MediaItem media) => '${media.infoHash}:${media.label}';
}
