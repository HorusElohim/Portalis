import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/collections/domain/share_payload_limits.dart';

void main() {
  test('accepts a normal share payload', () {
    expect(
      SharePayloadLimits.error(
        fileCount: 2,
        totalBytes: 2048,
        largestFileBytes: 1024,
      ),
      isNull,
    );
  });

  test('rejects a payload that can overflow the bridge length', () {
    expect(
      SharePayloadLimits.error(
        fileCount: 1,
        totalBytes: SharePayloadLimits.maxBytes + 1,
        largestFileBytes: SharePayloadLimits.maxBytes + 1,
      ),
      'One selected file is too large to share in one operation',
    );
  });

  test('rejects too many files before sending them to native code', () {
    expect(
      SharePayloadLimits.error(
        fileCount: SharePayloadLimits.maxFiles + 1,
        totalBytes: 1,
        largestFileBytes: 1,
      ),
      'A share can contain at most 10000 files',
    );
  });
}
