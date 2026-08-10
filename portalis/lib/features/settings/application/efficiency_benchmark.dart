/// A small, repeatable local benchmark for the Settings health surface.
///
/// It intentionally measures only local computation. It does not upload,
/// download, read user files, or change engine settings.
///
/// Timings are nanoseconds, matching the rest of Portalis. That is not a
/// rounded-up microsecond: `Stopwatch` ticks at nanosecond frequency here, and
/// it is `Duration` that cannot carry it — its finest unit is the microsecond,
/// so reading `stopwatch.elapsed` throws the last three digits away.
class EfficiencyBenchmark {
  const EfficiencyBenchmark();

  static const _iterations = 180000;

  Future<EfficiencyBenchmarkResult> run() {
    final stopwatch = Stopwatch()..start();
    var value = 0x12345678;
    for (var index = 0; index < _iterations; index++) {
      value = (value * 1664525 + 1013904223) & 0x7fffffff;
    }
    stopwatch.stop();
    return Future.value(
      EfficiencyBenchmarkResult(
        iterations: _iterations,
        elapsedNanoseconds: elapsedNanosecondsOf(stopwatch),
        checksum: value,
      ),
    );
  }

  /// Reads a stopwatch at the finest resolution its platform offers.
  ///
  /// Ticks are converted by the stopwatch's own frequency rather than assumed:
  /// native builds tick in nanoseconds, the web ticks in microseconds.
  static int elapsedNanosecondsOf(Stopwatch stopwatch) {
    const nanosecondsPerSecond = 1000000000;
    final frequency = stopwatch.frequency;
    if (frequency <= 0) return 0;
    // Whole nanoseconds per tick on every platform Dart runs on, which keeps
    // the multiplication small enough for the web's 53-bit integers.
    if (nanosecondsPerSecond % frequency == 0) {
      return stopwatch.elapsedTicks * (nanosecondsPerSecond ~/ frequency);
    }
    return stopwatch.elapsedTicks * nanosecondsPerSecond ~/ frequency;
  }
}

class EfficiencyBenchmarkResult {
  const EfficiencyBenchmarkResult({
    required this.iterations,
    required this.elapsedNanoseconds,
    required this.checksum,
  });

  final int iterations;
  final int elapsedNanoseconds;
  final int checksum;

  double get operationsPerSecond {
    // A run the clock reports as zero would divide by zero and claim an
    // infinite rate; one nanosecond makes the figure a lower bound instead.
    final nanoseconds = elapsedNanoseconds < 1 ? 1 : elapsedNanoseconds;
    return iterations / (nanoseconds / 1000000000);
  }

  /// What one iteration cost, which is the figure nanoseconds are really for.
  ///
  /// Averaged over [iterations], so its precision is far finer than the
  /// resolution of any single clock reading.
  double get nanosecondsPerOperation =>
      iterations < 1 ? 0 : elapsedNanoseconds / iterations;

  String get perOperationLabel =>
      '${nanosecondsPerOperation.toStringAsFixed(1)} ns/op';

  String get rateLabel {
    final rate = operationsPerSecond;
    if (rate >= 1000000) return '${(rate / 1000000).toStringAsFixed(1)}M ops/s';
    return '${(rate / 1000).toStringAsFixed(0)}K ops/s';
  }

  /// Scales to the run, the way [rateLabel] already does.
  ///
  /// A fixed unit rounds a fast run away: whole milliseconds turned a
  /// sub-millisecond benchmark into `0 ms`, which reads as broken rather than
  /// fast.
  String get durationLabel {
    final nanoseconds = elapsedNanoseconds;
    if (nanoseconds < 1000) return '$nanoseconds ns';
    if (nanoseconds < 1000000) {
      return '${(nanoseconds / 1000).toStringAsFixed(1)} µs';
    }
    if (nanoseconds < 1000000000) {
      return '${(nanoseconds / 1000000).toStringAsFixed(1)} ms';
    }
    return '${(nanoseconds / 1000000000).toStringAsFixed(2)} s';
  }
}
