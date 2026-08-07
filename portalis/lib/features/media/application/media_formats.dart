import 'package:flutter/material.dart';

import '../../../theme.dart';

/// What the app can actually do with a file of a given type, in the app.
///
/// This is a capability, not an aspiration: each value corresponds to a code
/// path that exists. If a renderer isn't written, the honest answer is
/// [externalOnly] with a reason — never a promise the viewer can't keep.
enum PreviewSupport {
  /// Decoded and drawn inline by Flutter's own image pipeline.
  image,

  /// Decoded by the platform image framework and drawn inline by Flutter.
  nativeImage,

  /// Played inline with transport controls (`video_player`).
  player,

  /// Rendered inline as plain text.
  text,

  /// No in-app renderer. Handed to the OS via `url_launcher` instead.
  externalOnly,
}

/// A single file type the app knows how to handle.
///
/// Everything the UI needs to present a file is declared here once —
/// classification, icon, accent, and what preview is possible. Nothing
/// downstream re-derives these from the
/// extension, so a format behaves identically in the grid, the viewer, the
/// details panel and the formats reference.
@immutable
class MediaFormat {
  const MediaFormat({
    required this.extensions,
    required this.label,
    required this.kind,
    required this.preview,
    this.previewNote,
    this.icon,
  });

  /// Lower-case, without the dot. The first is treated as canonical.
  final List<String> extensions;

  /// Human name, e.g. "JPEG image".
  final String label;

  final MediaKind kind;
  final PreviewSupport preview;

  /// Why in-app preview isn't possible. Required in spirit whenever
  /// [preview] is [PreviewSupport.externalOnly] — it's what turns a dead end
  /// into an explanation.
  final String? previewNote;

  /// Overrides the kind's default icon.
  final IconData? icon;

  String get canonicalExtension => extensions.first;

  IconData get effectiveIcon => icon ?? iconFor(kind);

  /// The accent this type reads as. Only torrents and live transfers get the
  /// reserved colours — a format never does.
  Color get accent => switch (kind) {
        MediaKind.image => AppColors.hues[1],
        MediaKind.video => AppColors.hues[3],
        MediaKind.audio => AppColors.hues[4],
        MediaKind.subtitle => AppColors.hues[2],
        MediaKind.document => AppColors.hues[5],
        MediaKind.archive => AppColors.hues[0],
        MediaKind.other => AppColors.textDim,
      };
}

/// The coarse bucket a format belongs to. Drives grouping and the default
/// icon; formats can still override individually.
enum MediaKind { image, video, audio, subtitle, document, archive, other }

/// The open registry.
///
/// "Open" in the sense that matters: adding support for a type is one
/// [register] call, and every surface — thumbnails, the viewer, the details
/// panel, the formats reference screen — picks it up without being touched.
/// Nothing anywhere else is allowed to hard-code an extension list, which is
/// what let the old `imageExtensions`/`videoExtensions` sets drift out of
/// step with what the viewer could really render.
class MediaFormats {
  MediaFormats._();

  static final Map<String, MediaFormat> _byExtension = {};
  static final List<MediaFormat> _registered = [];
  static bool _defaultsInstalled = false;

  /// Registers a format. Later registrations win for a contested extension,
  /// so an app or a test can override a built-in without editing this file.
  static void register(MediaFormat format) {
    _ensureDefaults();
    _add(format);
  }

  static void _add(MediaFormat format) {
    _registered.removeWhere((f) => identical(f, format));
    _registered.add(format);
    for (final ext in format.extensions) {
      _byExtension[ext.toLowerCase()] = format;
    }
  }

  /// The format for a filename, or `null` when nothing claims it.
  static MediaFormat? lookup(String filename) {
    _ensureDefaults();
    return _byExtension[extensionOf(filename)];
  }

  /// The format for a filename, falling back to [unknown] so callers never
  /// have to special-case an unrecognised file.
  static MediaFormat resolve(String filename) => lookup(filename) ?? unknown;

  /// Every registered format, grouped and ordered for display.
  static List<MediaFormat> get all {
    _ensureDefaults();
    final order = MediaKind.values;
    return [..._registered]..sort((a, b) {
        final byKind = order.indexOf(a.kind).compareTo(order.indexOf(b.kind));
        return byKind != 0 ? byKind : a.label.compareTo(b.label);
      });
  }

  static List<MediaFormat> ofKind(MediaKind kind) =>
      all.where((f) => f.kind == kind).toList();

  /// Every extension the app claims to understand.
  static Set<String> get knownExtensions {
    _ensureDefaults();
    return _byExtension.keys.toSet();
  }

  /// The catch-all. Not registered against any extension — it's what
  /// [resolve] returns when nothing matches.
  static const unknown = MediaFormat(
    extensions: ['*'],
    label: 'File',
    kind: MediaKind.other,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no viewer for this type, so it opens in '
        'whatever app your system uses for it.',
  );

  /// Visible for tests that need a clean slate.
  @visibleForTesting
  static void resetToDefaults() {
    _byExtension.clear();
    _registered.clear();
    _defaultsInstalled = false;
    _ensureDefaults();
  }

  static void _ensureDefaults() {
    if (_defaultsInstalled) return;
    // Set before installing, so a format whose construction calls back into
    // the registry can't recurse.
    _defaultsInstalled = true;
    for (final f in _builtIns) {
      _add(f);
    }
  }
}

/// Default icon per kind. A format may override it.
IconData iconFor(MediaKind kind) => switch (kind) {
      MediaKind.image => Icons.image_outlined,
      MediaKind.video => Icons.movie_outlined,
      MediaKind.audio => Icons.audiotrack_outlined,
      MediaKind.subtitle => Icons.subtitles_outlined,
      MediaKind.document => Icons.description_outlined,
      MediaKind.archive => Icons.folder_zip_outlined,
      MediaKind.other => Icons.insert_drive_file_outlined,
    };

/// The lower-case extension of a filename, without the dot. Empty when there
/// isn't one.
String extensionOf(String name) {
  final dot = name.lastIndexOf('.');
  return dot == -1 ? '' : name.substring(dot + 1).toLowerCase();
}

// ---------------------------------------------------------------------------
// Convenience predicates, kept so call sites read naturally. All of them go
// through the registry rather than their own extension list.
// ---------------------------------------------------------------------------

MediaKind kindOf(String name) => MediaFormats.resolve(name).kind;

bool isImage(String name) => MediaFormats.resolve(name).kind == MediaKind.image;
bool isVideo(String name) => MediaFormats.resolve(name).kind == MediaKind.video;
bool isAudio(String name) => MediaFormats.resolve(name).kind == MediaKind.audio;
bool isSubtitle(String name) =>
    MediaFormats.resolve(name).kind == MediaKind.subtitle;

/// Whether the in-app viewer can render this file itself.
bool hasInAppPreview(String name) =>
    MediaFormats.resolve(name).preview != PreviewSupport.externalOnly;

// ---------------------------------------------------------------------------
// Built-ins
// ---------------------------------------------------------------------------

const _kNoDecoder =
    'Flutter\'s image pipeline has no decoder for this format, so it opens '
    'in your system viewer instead.';

final List<MediaFormat> _builtIns = [
  // --- Images -------------------------------------------------------------
  const MediaFormat(
    extensions: ['jpg', 'jpeg'],
    label: 'JPEG image',
    kind: MediaKind.image,
    preview: PreviewSupport.image,
  ),
  const MediaFormat(
    extensions: ['png'],
    label: 'PNG image',
    kind: MediaKind.image,
    preview: PreviewSupport.image,
  ),
  const MediaFormat(
    extensions: ['gif'],
    label: 'GIF image',
    kind: MediaKind.image,
    preview: PreviewSupport.image,
  ),
  const MediaFormat(
    extensions: ['webp'],
    label: 'WebP image',
    kind: MediaKind.image,
    preview: PreviewSupport.image,
  ),
  const MediaFormat(
    extensions: ['bmp'],
    label: 'Bitmap image',
    kind: MediaKind.image,
    preview: PreviewSupport.image,
  ),
  const MediaFormat(
    extensions: ['heic', 'heif'],
    label: 'HEIC photo',
    kind: MediaKind.image,
    preview: PreviewSupport.nativeImage,
  ),

  // --- Video --------------------------------------------------------------
  const MediaFormat(
    extensions: ['mp4', 'm4v'],
    label: 'MP4 video',
    kind: MediaKind.video,
    preview: PreviewSupport.player,
  ),
  const MediaFormat(
    extensions: ['mov'],
    label: 'QuickTime video',
    kind: MediaKind.video,
    preview: PreviewSupport.player,
  ),
  const MediaFormat(
    extensions: ['webm'],
    label: 'WebM video',
    kind: MediaKind.video,
    preview: PreviewSupport.player,
  ),
  const MediaFormat(
    extensions: ['mkv'],
    label: 'Matroska video',
    kind: MediaKind.video,
    // Try the native player first. Unsupported codecs are handled by the
    // viewer's existing failure fallback and can still open externally.
    preview: PreviewSupport.player,
  ),
  const MediaFormat(
    extensions: ['avi'],
    label: 'AVI video',
    kind: MediaKind.video,
    preview: PreviewSupport.player,
  ),

  // --- Audio --------------------------------------------------------------
  const MediaFormat(
    extensions: ['mp3'],
    label: 'MP3 audio',
    kind: MediaKind.audio,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no audio player yet, so this opens in your '
        'system player.',
  ),
  const MediaFormat(
    extensions: ['m4a', 'aac'],
    label: 'AAC audio',
    kind: MediaKind.audio,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no audio player yet, so this opens in your '
        'system player.',
  ),
  const MediaFormat(
    extensions: ['wav'],
    label: 'WAV audio',
    kind: MediaKind.audio,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no audio player yet, so this opens in your '
        'system player.',
  ),
  const MediaFormat(
    extensions: ['flac'],
    label: 'FLAC audio',
    kind: MediaKind.audio,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no audio player yet, so this opens in your '
        'system player.',
  ),
  const MediaFormat(
    extensions: ['ogg', 'opus'],
    label: 'Ogg audio',
    kind: MediaKind.audio,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Portalis has no audio player yet, so this opens in your '
        'system player.',
  ),

  // --- Subtitles ----------------------------------------------------------
  const MediaFormat(
    extensions: ['srt'],
    label: 'SubRip subtitles',
    kind: MediaKind.subtitle,
    preview: PreviewSupport.text,
  ),
  const MediaFormat(
    extensions: ['vtt'],
    label: 'WebVTT subtitles',
    kind: MediaKind.subtitle,
    preview: PreviewSupport.text,
  ),
  const MediaFormat(
    extensions: ['ass', 'ssa'],
    label: 'ASS/SSA subtitles',
    kind: MediaKind.subtitle,
    preview: PreviewSupport.text,
  ),
  const MediaFormat(
    extensions: ['sub'],
    label: 'MicroDVD subtitles',
    kind: MediaKind.subtitle,
    preview: PreviewSupport.text,
  ),

  // --- Documents ----------------------------------------------------------
  const MediaFormat(
    extensions: ['txt', 'md', 'log'],
    label: 'Plain text',
    kind: MediaKind.document,
    preview: PreviewSupport.text,
  ),
  const MediaFormat(
    extensions: ['pdf'],
    label: 'PDF document',
    kind: MediaKind.document,
    preview: PreviewSupport.externalOnly,
    previewNote: _kNoDecoder,
  ),

  // --- Archives -----------------------------------------------------------
  const MediaFormat(
    extensions: ['zip'],
    label: 'ZIP archive',
    kind: MediaKind.archive,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Archives are shared as-is and opened by your system.',
  ),
  const MediaFormat(
    extensions: ['iso'],
    label: 'Disc image',
    kind: MediaKind.archive,
    preview: PreviewSupport.externalOnly,
    previewNote: 'Archives are shared as-is and opened by your system.',
  ),
];
