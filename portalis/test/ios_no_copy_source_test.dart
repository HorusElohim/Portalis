import 'package:flutter/foundation.dart';
import 'package:flutter/services.dart';
import 'package:portalis/features/collections/platform/no_copy_source_picker.dart';
import 'package:portalis/features/collections/platform/source_access.dart';

import 'test_support.dart';

void main() {
  TestWidgetsFlutterBinding.ensureInitialized();
  const channel = MethodChannel('app.portalis/no-copy-source-picker');

  setUp(() {
    debugDefaultTargetPlatformOverride = TargetPlatform.iOS;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, (call) async {
      if (call.method == 'pickFiles') {
        return [
          {
            'name': 'clip.mov',
            'path': '/private/var/mobile/Media/clip.mov',
            'lengthBytes': 6000000000,
          },
        ];
      }
      return null;
    });
  });

  tearDown(() {
    debugDefaultTargetPlatformOverride = null;
    TestDefaultBinaryMessengerBinding.instance.defaultBinaryMessenger
        .setMockMethodCallHandler(channel, null);
  });

  test('iOS accepts only the native Files no-copy source path', () async {
    expect(supportsDirectPathSources, isFalse);
    expect(supportsNativeFilesSources, isTrue);
    expect(supportsNoCopySources, isTrue);

    final files = await NoCopySourcePicker.pickFiles();

    expect(files, hasLength(1));
    expect(files.single.name, 'clip.mov');
    expect(files.single.path, '/private/var/mobile/Media/clip.mov');
    expect(files.single.lengthBytes, 6000000000);
  });
}
