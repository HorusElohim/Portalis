import 'dart:typed_data';

import 'package:flutter_image_compress/flutter_image_compress.dart';

/// Re-encodes HEIC/HEIF to JPEG.
///
/// Flutter's built-in image codecs (Skia) cannot decode HEIC at all — the
/// iPhone camera's default format since iOS 11. Left as-is, a shared HEIC
/// photo would never preview in-app for anyone (the sender included) and may
/// not open for recipients on non-Apple platforms either. Converting at
/// share-time fixes both: the JPEG is what actually gets seeded, so it is
/// what every peer receives.
///
/// Registered against the HEIC format in `media/formats.dart` rather than
/// called directly, so the decision "this type needs converting" lives with
/// the type. Kept in its own file so the registry doesn't pull in a platform
/// codec plugin.
Future<({String name, Uint8List bytes})> heicToJpeg({
  required String name,
  required Uint8List bytes,
}) async {
  final jpeg = await FlutterImageCompress.compressWithList(
    bytes,
    // Large enough that real photos are never needlessly downscaled — this is
    // a format fix, not a size reduction.
    minWidth: 8000,
    minHeight: 8000,
    quality: 95,
  );

  final dot = name.lastIndexOf('.');
  final baseName = dot == -1 ? name : name.substring(0, dot);
  return (name: '$baseName.jpg', bytes: jpeg);
}
