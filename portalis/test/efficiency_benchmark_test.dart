import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/settings/application/efficiency_benchmark.dart';

EfficiencyBenchmarkResult resultOf(int elapsedNanoseconds) =>
    EfficiencyBenchmarkResult(
      iterations: 180000,
      elapsedNanoseconds: elapsedNanoseconds,
      checksum: 1,
    );

void main() {
  test('local efficiency benchmark returns a useful result', () async {
    final result = await const EfficiencyBenchmark().run();

    expect(result.iterations, greaterThan(0));
    expect(result.elapsedNanoseconds, greaterThanOrEqualTo(0));
    expect(result.operationsPerSecond, greaterThan(0));
    expect(result.rateLabel, contains('ops/s'));
    expect(result.durationLabel, isNotEmpty);
  });

  test('the stopwatch is read at nanosecond resolution, not truncated', () {
    final stopwatch = Stopwatch()..start();
    var value = 1;
    for (var index = 0; index < 180000; index++) {
      value = (value * 1664525 + 1013904223) & 0x7fffffff;
    }
    stopwatch.stop();
    expect(value, isNonZero);

    final nanoseconds = EfficiencyBenchmark.elapsedNanosecondsOf(stopwatch);

    // Duration bottoms out at microseconds, so this is the precision that
    // reading stopwatch.elapsed would have discarded.
    expect(
      nanoseconds,
      greaterThanOrEqualTo(stopwatch.elapsed.inMicroseconds * 1000),
    );
    expect(nanoseconds, lessThan((stopwatch.elapsed.inMicroseconds + 1) * 1000));
  });

  group('durationLabel scales to the run', () {
    test('a sub-microsecond run reads as nanoseconds, not as zero', () {
      expect(resultOf(834).durationLabel, '834 ns');
      expect(resultOf(0).durationLabel, '0 ns');
    });

    test('a microsecond run keeps a decimal', () {
      expect(resultOf(1000).durationLabel, '1.0 µs');
      expect(resultOf(578875).durationLabel, '578.9 µs');
    });

    test('a millisecond run reads as milliseconds', () {
      expect(resultOf(12345000).durationLabel, '12.3 ms');
    });

    test('a run past a second reads as seconds', () {
      expect(resultOf(1500000000).durationLabel, '1.50 s');
    });
  });

  test('reports what one iteration cost', () {
    // 180,000 iterations in 578,875 ns is a little over 3 ns each.
    expect(resultOf(578875).nanosecondsPerOperation, closeTo(3.216, 0.001));
    expect(resultOf(578875).perOperationLabel, '3.2 ns/op');
  });

  test('an immeasurably fast run reports a bounded rate, not infinity', () {
    final result = resultOf(0);

    expect(result.operationsPerSecond.isFinite, isTrue);
    expect(result.operationsPerSecond, greaterThan(0));
    expect(result.rateLabel, isNot(contains('Infinity')));
  });
}
