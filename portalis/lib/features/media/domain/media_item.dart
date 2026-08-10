/// One file projected by the backend, independent of the collection that
/// carries it. Collection entries add grouping and collaboration context.
class MediaItem {
  const MediaItem({
    required this.label,
    required this.infoHash,
    String? entryLabel,
    this.localPath,
    this.progress = 0.0,
    this.sizeBytes = 0,
    this.downloadedBytes = 0,
    this.pieceRuns = const [],
    this.fetched = true,
    this.addedBy,
  }) : _entryLabel = entryLabel;

  final String label;
  final String? _entryLabel;
  final String infoHash;
  final String? localPath;
  final double progress;
  final int sizeBytes;
  final int downloadedBytes;
  final List<MediaPieceRun> pieceRuns;
  final bool fetched;
  final String? addedBy;

  /// The signed name of the collection entry that introduced this file.
  String get entryLabel => _entryLabel ?? label;

  /// The backend exposes a path only after it has verified completion, so the
  /// path is the readiness signal. This avoids a stale progress value hiding
  /// a valid local file.
  bool get isReady => localPath != null;
}

/// One exact file-relative intersection with torrent piece state. Missing
/// byte ranges are intentionally absent.
class MediaPieceRun {
  const MediaPieceRun({
    required this.offsetBytes,
    required this.lengthBytes,
    required this.verified,
    this.peers = const [],
  });

  final int offsetBytes;
  final int lengthBytes;
  final bool verified;
  final List<String> peers;

  bool get isDownloading => !verified && peers.isNotEmpty;
}
