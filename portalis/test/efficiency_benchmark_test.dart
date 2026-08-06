import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/features/settings/application/efficiency_benchmark.dart';

void main() {
  test('local efficiency benchmark returns a useful result', () async {
    final result = await const EfficiencyBenchmark().run();

    expect(result.iterations, greaterThan(0));
    expect(result.elapsed, greaterThanOrEqualTo(Duration.zero));
    expect(result.operationsPerSecond, greaterThan(0));
    expect(result.rateLabel, contains('ops/s'));
    expect(result.durationLabel, endsWith('ms'));
  });
}
