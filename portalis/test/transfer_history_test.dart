import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/collections/domain/transfer_history.dart';

void main() {
  test('keeps timestamped transfer samples at a readable cadence', () {
    final start = DateTime(2026, 8, 5, 12, 0);
    final history = TransferHistory(startedAt: start);

    expect(
      history.record(
        at: start,
        downloadMbps: 1,
        uploadMbps: 0,
        progress: 0.1,
      ),
      isTrue,
    );
    expect(
      history.record(
        at: start.add(const Duration(milliseconds: 500)),
        downloadMbps: 2,
        uploadMbps: 0,
        progress: 0.2,
      ),
      isFalse,
    );
    expect(
      history.record(
        at: start.add(const Duration(seconds: 1)),
        downloadMbps: 3,
        uploadMbps: 0.2,
        progress: 0.3,
      ),
      isTrue,
    );

    expect(history.samples, hasLength(2));
    expect(history.samples.last.downloadMbps, 3);
  });
}
