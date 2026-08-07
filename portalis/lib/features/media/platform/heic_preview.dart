import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

/// Decodes HEIC/HEIF files with the operating system's image framework.
///
/// The returned JPEG is a bounded preview only. The source file remains at
/// its original path and is never rewritten or copied into app storage.
class HeicPreview {
  HeicPreview._();

  static const _channel = MethodChannel('app.portalis/heic-preview');

  static bool get isSupported => switch (defaultTargetPlatform) {
        TargetPlatform.android ||
        TargetPlatform.iOS ||
        TargetPlatform.macOS =>
          true,
        _ => false,
      };

  static Future<Uint8List?> decode(String path, int maxPixelSize) async {
    if (!isSupported) return null;
    try {
      return await _channel.invokeMethod<Uint8List>('decode', {
        'path': path,
        'maxPixelSize': maxPixelSize.clamp(64, 2048),
      });
    } on PlatformException {
      return null;
    } on MissingPluginException {
      return null;
    }
  }
}
