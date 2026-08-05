import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/settings/domain/engine_settings.dart';

const _settings = EngineSettings(
  uploadLimitBps: 100,
  downloadDir: 'C:/Downloads',
  listenPortStart: 6881,
  listenPortEnd: 6999,
  enableUpnpPortForwarding: true,
  disableDht: false,
  disableDhtPersistence: false,
  persistSession: true,
  fastresume: true,
  trackers: ['udp://tracker.example:1337'],
);

void main() {
  test('copyWith preserves fields that are omitted', () {
    final updated = _settings.copyWith(listenPortEnd: 7000);

    expect(updated.listenPortStart, 6881);
    expect(updated.listenPortEnd, 7000);
    expect(updated.downloadDir, 'C:/Downloads');
  });

  test('copyWith can intentionally clear optional fields', () {
    final cleared = _settings.copyWith(
      uploadLimitBps: null,
      downloadDir: null,
    );

    expect(cleared.uploadLimitBps, isNull);
    expect(cleared.downloadDir, isNull);
    expect(cleared.trackers, _settings.trackers);
  });
}
