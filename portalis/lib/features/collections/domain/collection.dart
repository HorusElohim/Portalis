import '../../media/domain/media_item.dart';
import 'collection_import.dart';

/// Pure collection state used by the Flutter application.
///
/// This model deliberately knows nothing about Flutter widgets, colours, text
/// formatting, or Flutter-Rust Bridge. It is the stable shape controllers and
/// screens share; adapters map native DTOs into it and presentation extensions
/// decide how to render it.
enum CollectionKind { shared, torrent }

class Collaborator {
  const Collaborator({
    required this.deviceId,
    required this.name,
    this.isAdmin = false,
  });

  final String deviceId;
  final String name;
  final bool isAdmin;

  String get initials => name.isEmpty ? '?' : name[0].toUpperCase();
}

/// One user-visible collection: either an invite-based shared collection or
/// one plain torrent. The backend owns the state labels and figures; this type
/// exposes only semantic queries derived from those facts.
class Collection {
  const Collection({
    required this.id,
    required this.name,
    required this.kind,
    required this.collaborators,
    required this.media,
    this.inviteCode,
    this.progress = 0.0,
    this.totalBytes = 0,
    this.downloadedBytes = 0,
    this.uploadedBytes = 0,
    this.downloadMbps = 0.0,
    this.uploadMbps = 0.0,
    this.livePeers = 0,
    this.torrentPeers = const [],
    this.pendingMedia = 0,
    this.etaSecs,
    this.state = '',
    this.ingestion,
  });

  final String id;
  final String name;
  final CollectionKind kind;
  final String? inviteCode;
  final List<Collaborator> collaborators;
  final List<MediaItem> media;
  final double progress;
  final int totalBytes;
  final int downloadedBytes;
  final int uploadedBytes;
  final double downloadMbps;
  final double uploadMbps;
  final int livePeers;

  /// `"ip:port"` of this collection's live swarm peers — `torrent` kind
  /// only. Anonymous: BitTorrent carries no identity beyond a network
  /// address, so there is no name to show, only that someone is there.
  final List<String> torrentPeers;
  final int pendingMedia;
  final int? etaSecs;

  /// Backend-defined state, including native `importing` and `import_failed`
  /// publication states.
  final String state;
  final CollectionImport? ingestion;

  bool get isShared => kind == CollectionKind.shared;
  bool get isComplete => progress >= 1.0 && pendingMedia == 0;
  bool get isSharing => state == 'seeding' && media.isNotEmpty;
  bool get isConnecting => state == 'connecting';
  bool get isMoving =>
      downloadMbps > 0 ||
      uploadMbps > 0 ||
      state == 'downloading' ||
      (ingestion != null && !ingestion!.failed) ||
      pendingMedia > 0;

  /// The entries that made this collection grow, regrouped from its flat media
  /// list in the order the backend projected them.
  List<CollectionEntry> get entries {
    final byHash = <String, List<MediaItem>>{};
    for (final mediaItem in media) {
      byHash.putIfAbsent(mediaItem.infoHash, () => []).add(mediaItem);
    }
    return [
      for (final entry in byHash.entries)
        CollectionEntry(
          infoHash: entry.key,
          addedBy: entry.value.first.addedBy,
          media: entry.value,
        ),
    ];
  }

  /// A complete value fingerprint for the polling controller. Keeping it here
  /// avoids making rendering depend on object identity while still including
  /// every value current UI can display.
  int get signature => Object.hash(
        id,
        name,
        kind,
        inviteCode,
        progress,
        totalBytes,
        downloadedBytes,
        uploadedBytes,
        downloadMbps,
        uploadMbps,
        livePeers,
        pendingMedia,
        etaSecs,
        state,
        ingestion == null
            ? null
            : Object.hash(
                ingestion!.stage,
                ingestion!.progress,
                ingestion!.processedBytes,
                ingestion!.totalBytes,
                ingestion!.completedPieces,
                ingestion!.totalPieces,
                ingestion!.error,
              ),
        Object.hashAll(
          collaborators.map((c) => Object.hash(c.deviceId, c.name, c.isAdmin)),
        ),
        Object.hashAll(torrentPeers),
        Object.hashAll(
          media.map(
            (m) => Object.hash(
              m.label,
              m.entryLabel,
              m.infoHash,
              m.localPath,
              m.progress,
              m.sizeBytes,
              m.downloadedBytes,
              Object.hashAll(
                m.pieceRuns.map(
                  (run) => Object.hash(
                    run.offsetBytes,
                    run.lengthBytes,
                    run.verified,
                    Object.hashAll(run.peers),
                  ),
                ),
              ),
              m.fetched,
              m.addedBy,
            ),
          ),
        ),
      );
}

/// One signed manifest entry, represented by the torrent that carries its
/// files. A not-yet-fetched entry is a single placeholder media item.
class CollectionEntry {
  const CollectionEntry({
    required this.infoHash,
    required this.media,
    this.addedBy,
  });

  final String infoHash;
  final List<MediaItem> media;
  final String? addedBy;

  bool get fetched => media.any((item) => item.fetched);
  int get totalBytes => media.fold(0, (sum, item) => sum + item.sizeBytes);
  int get downloadedBytes =>
      media.fold(0, (sum, item) => sum + item.downloadedBytes);
  double get progress =>
      totalBytes == 0 ? 0.0 : (downloadedBytes / totalBytes).clamp(0.0, 1.0);
  String get label => media.isEmpty ? infoHash : media.first.entryLabel;
}
