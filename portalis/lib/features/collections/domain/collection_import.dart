/// Native-owned local publication state for one collection.
class CollectionImport {
  const CollectionImport({
    required this.stage,
    required this.progress,
    required this.processedBytes,
    required this.totalBytes,
    this.error,
  });

  final String stage;
  final double progress;
  final int processedBytes;
  final int totalBytes;
  final String? error;

  bool get failed => error != null;
}
