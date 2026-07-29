import 'dart:typed_data';

import 'package:flutter_image_compress/flutter_image_compress.dart';

import 'media_kind.dart';

/// Flutter's built-in image codecs (Skia) can't decode HEIC/HEIF at all —
/// the iPhone camera's default format since iOS 11. Left as-is, a shared
/// HEIC photo would never preview in-app for anyone (us included) and may
/// not open for recipients on non-Apple platforms either. Converting to
/// JPEG at share-time fixes both: it's what actually gets seeded, so it's
/// what every peer receives.
///
/// Non-HEIC files pass through untouched.
Future<({String name, Uint8List bytes})> normalizeForSharing({
  required String name,
  required Uint8List bytes,
}) async {
  final ext = extensionOf(name);
  if (ext != 'heic' && ext != 'heif') {
    return (name: name, bytes: bytes);
  }

  final jpeg = await FlutterImageCompress.compressWithList(
    bytes,
    // Large enough that real photos never get needlessly downscaled —
    // this is a format fix, not a size reduction.
    minWidth: 8000,
    minHeight: 8000,
    quality: 95,
  );

  final dot = name.lastIndexOf('.');
  final baseName = dot == -1 ? name : name.substring(0, dot);
  return (name: '$baseName.jpg', bytes: jpeg);
}
