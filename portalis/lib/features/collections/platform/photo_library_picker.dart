import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';

import '../domain/picked_file.dart';

/// Selects persistent PhotoKit references. No picker cache path crosses into
/// Dart, so Rust reads the original Photos-library asset when seeding.
class PhotoLibraryPicker {
  PhotoLibraryPicker._();

  static const _channel = MethodChannel('app.portalis/photo-library-picker');

  static Future<List<PickedFile>> pickMedia() async {
    if (defaultTargetPlatform != TargetPlatform.iOS) {
      throw UnsupportedError('The native Photos picker is only available on iOS');
    }
    final values = await _channel.invokeListMethod<dynamic>('pickMedia') ?? [];
    return values.map(_fromNative).toList(growable: false);
  }

  static Future<void> previewMedia(String sourcePath) async {
    if (defaultTargetPlatform != TargetPlatform.iOS ||
        !sourcePath.startsWith('phasset://')) {
      throw UnsupportedError('The selected media is not a Photos-library asset');
    }
    await _channel.invokeMethod<void>('previewMedia', {'path': sourcePath});
  }

  static PickedFile _fromNative(dynamic value) {
    if (value is! Map) throw const FormatException('Photos picker returned an invalid item');
    final item = Map<Object?, Object?>.from(value);
    final name = item['name'];
    final path = item['path'];
    final length = item['lengthBytes'];
    if (name is! String || path is! String || length is! int || length <= 0) {
      throw const FormatException('Photos picker returned invalid media metadata');
    }
    return PickedFile(name: name, path: path, lengthBytes: length);
  }
}
