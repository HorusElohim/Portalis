/// Download and upload samples collected while a collection is visible to the
/// running transfer engine.
class TransferHistory {
  TransferHistory({required this.startedAt});

  static const _sampleSpacing = Duration(seconds: 1);
  static const _maxSamples = 1800;

  final DateTime startedAt;
  final List<TransferSample> _samples = [];
  DateTime? completedAt;

  List<TransferSample> get samples => List.unmodifiable(_samples);

  bool record({
    required DateTime at,
    required double downloadMbps,
    required double uploadMbps,
    required double progress,
  }) {
    final previous = _samples.isEmpty ? null : _samples.last;
    if (previous != null && at.difference(previous.at) < _sampleSpacing) {
      return false;
    }
    _samples.add(
      TransferSample(
        at: at,
        downloadMbps: downloadMbps,
        uploadMbps: uploadMbps,
        progress: progress,
      ),
    );
    if (_samples.length > _maxSamples) {
      _samples.removeRange(0, _samples.length - _maxSamples);
    }
    return true;
  }
}

class TransferSample {
  const TransferSample({
    required this.at,
    required this.downloadMbps,
    required this.uploadMbps,
    required this.progress,
  });

  final DateTime at;
  final double downloadMbps;
  final double uploadMbps;
  final double progress;
}
