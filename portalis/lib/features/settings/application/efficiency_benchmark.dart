/// A small, repeatable local benchmark for the Settings health surface.
///
/// It intentionally measures only local computation. It does not upload,
/// download, read user files, or change engine settings.
class EfficiencyBenchmark {
  const EfficiencyBenchmark();

  static const _iterations = 180000;

  Future<EfficiencyBenchmarkResult> run() async {
    final stopwatch = Stopwatch()..start();
    var value = 0x12345678;
    for (var index = 0; index < _iterations; index++) {
      value = (value * 1664525 + 1013904223) & 0x7fffffff;
      if (index % 12000 == 0) {
        await Future<void>.delayed(Duration.zero);
      }
    }
    stopwatch.stop();
    return EfficiencyBenchmarkResult(
      iterations: _iterations,
      elapsed: stopwatch.elapsed,
      checksum: value,
    );
  }
}

class EfficiencyBenchmarkResult {
  const EfficiencyBenchmarkResult({
    required this.iterations,
    required this.elapsed,
    required this.checksum,
  });

  final int iterations;
  final Duration elapsed;
  final int checksum;

  double get operationsPerSecond =>
      iterations / (elapsed.inMicroseconds / Duration.microsecondsPerSecond);

  String get rateLabel {
    final rate = operationsPerSecond;
    if (rate >= 1000000) return '${(rate / 1000000).toStringAsFixed(1)}M ops/s';
    return '${(rate / 1000).toStringAsFixed(0)}K ops/s';
  }

  String get durationLabel => '${elapsed.inMilliseconds} ms';
}
