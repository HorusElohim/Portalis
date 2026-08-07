/// Native-owned local publication state for one collection.
class CollectionImport {
  const CollectionImport({
    required this.stage,
    required this.progress,
    required this.processedBytes,
    required this.totalBytes,
    this.completedPieces = 0,
    this.totalPieces = 0,
    this.error,
  });

  final String stage;
  final double progress;
  final int processedBytes;
  final int totalBytes;
  final int completedPieces;
  final int totalPieces;
  final String? error;

  bool get failed => error != null;
}
