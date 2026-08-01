import 'package:flutter/material.dart';

import 'bridge_generated/collections.dart' as bridge;
import 'theme.dart';
export 'theme.dart' show GlowLevel;

export 'bridge_generated/collections.dart' show CollectionKind;

/// The UI-facing mirrors of `collections.rs`'s DTOs.
///
/// Every field here is populated from Rust — there is no mock or sample data
/// path in the app. These types exist rather than using the generated DTOs
/// directly only to add presentation-layer conveniences ([Collection.hue],
/// [Collaborator.initials], [MediaItem.isReady]) and to keep widgets from
/// importing generated code. The mapping is exhaustive and one-way: build
/// them with `fromInfo`, never by hand outside tests.

class MediaItem {
  const MediaItem({
    required this.label,
    required this.infoHash,
    String? entryLabel,
    this.localPath,
    this.progress = 0.0,
    this.sizeBytes = 0,
    this.downloadedBytes = 0,
    this.fetched = true,
    this.addedBy,
  }) : _entryLabel = entryLabel;

  factory MediaItem.fromInfo(bridge.MediaInfo m) => MediaItem(
        label: m.name,
        entryLabel: m.entryName,
        infoHash: m.infoHash,
        localPath: m.absolutePath,
        progress: m.progress,
        sizeBytes: m.lengthBytes.toInt(),
        downloadedBytes: m.downloadedBytes.toInt(),
        fetched: m.fetched,
        addedBy: m.addedBy,
      );

  final String label;

  /// The signed label of the manifest entry this file belongs to — the batch
  /// it was added as, not this file's own name. Defaults to [label] only for
  /// hand-built instances in tests.
  final String? _entryLabel;

  String get entryLabel => _entryLabel ?? label;

  /// The torrent (manifest entry) this file belongs to. Several files in a
  /// collection can share one.
  final String infoHash;

  /// Absolute path, set only once the file is complete — a partially
  /// written file won't open or decode.
  final String? localPath;

  final double progress;
  final int sizeBytes;
  final int downloadedBytes;

  /// `false` when this stands for a whole manifest entry whose torrent isn't
  /// in the session yet: known to exist because a collaborator signed it into
  /// the manifest, but not downloaded. Tap to fetch.
  final bool fetched;

  /// Device id of the collaborator who added it — shared collections only.
  final String? addedBy;

  bool get isReady => localPath != null && progress >= 1.0;
}

class Collaborator {
  const Collaborator({
    required this.deviceId,
    required this.name,
    this.isAdmin = false,
  });

  factory Collaborator.fromInfo(bridge.CollaboratorInfo c) => Collaborator(
        deviceId: c.deviceId,
        name: c.displayName,
        isAdmin: c.isAdmin,
      );

  final String deviceId;
  final String name;
  final bool isAdmin;

  String get initials => name.isEmpty ? '?' : name[0].toUpperCase();
}

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
    this.pendingMedia = 0,
    this.state = '',
  });

  factory Collection.fromInfo(bridge.CollectionInfo c) => Collection(
        id: c.id,
        name: c.name,
        kind: c.kind,
        inviteCode: c.inviteCode,
        collaborators: c.collaborators.map(Collaborator.fromInfo).toList(),
        media: c.media.map(MediaItem.fromInfo).toList(),
        progress: c.progress,
        totalBytes: c.totalBytes.toInt(),
        downloadedBytes: c.downloadedBytes.toInt(),
        uploadedBytes: c.uploadedBytes.toInt(),
        downloadMbps: c.downloadMbps,
        uploadMbps: c.uploadMbps,
        livePeers: c.livePeers,
        pendingMedia: c.pendingMedia,
        state: c.state,
      );

  /// A shared collection's id, or a plain torrent's info-hash.
  final String id;
  final String name;
  final bridge.CollectionKind kind;

  /// Present only on shared collections — a plain torrent has nothing to
  /// invite anyone to.
  final String? inviteCode;

  final List<Collaborator> collaborators;
  final List<MediaItem> media;

  final double progress;
  final int totalBytes;
  final int downloadedBytes;
  final int uploadedBytes;
  final double downloadMbps;
  final double uploadMbps;

  /// Currently-connected peers across this collection's torrents.
  final int livePeers;

  /// Manifest entries not yet fetched — see [MediaItem.fetched].
  final int pendingMedia;

  /// `seeding` / `downloading` / `pending` / `empty`, decided in Rust so both
  /// kinds of collection describe themselves the same way.
  final String state;

  bool get isShared => kind == bridge.CollectionKind.shared;

  bool get isComplete => progress >= 1.0 && pendingMedia == 0;

  /// Whether this device is actually serving this collection to others right
  /// now — not "could", not "has the files", but *is*.
  ///
  /// The engine has to be up and the collection has to have something live in
  /// it. Anything short of that is honestly reported as not shared, because
  /// the alternative is telling someone their photos are available when no
  /// socket is listening for them.
  bool get isSharing => state == 'seeding' && media.isNotEmpty;

  /// A fetch has been started but no peer has served the metadata yet, so
  /// there is nothing to measure — not even a total size.
  ///
  /// Worth its own state because it is otherwise identical to "nobody has
  /// asked for this yet": both show zero bytes of zero. Telling them apart is
  /// the difference between "the app is looking for the other device" and
  /// "nothing is happening".
  bool get isConnecting => state == 'connecting';

  /// Energy for this collection's card, by what it is genuinely doing.
  GlowLevel get glow {
    if (downloadMbps > 0 || uploadMbps > 0) {
      return (downloadMbps + uploadMbps) > 4 ? GlowLevel.vivid : GlowLevel.active;
    }
    // Reaching for a peer counts as alive — something is being attempted, and
    // a dark card would read as nothing happening.
    if (isConnecting) return GlowLevel.calm;
    // Shared and standing by: alive, but nothing is flowing.
    return isSharing ? GlowLevel.calm : GlowLevel.none;
  }

  /// Stable per-collection accent, derived from the id so a collection keeps
  /// its colour across restarts.
  Color get hue => AppColors.hueAt(id.hashCode.abs());

  String get subtitle {
    final count = media.length;
    final items = '$count item${count == 1 ? '' : 's'}';
    if (isConnecting) return '$items · looking for a peer';
    return pendingMedia > 0 ? '$items · $pendingMedia to fetch' : items;
  }

  String get peersLabel =>
      '$livePeers peer${livePeers == 1 ? '' : 's'}';

  /// The "live copies" line. Counts this device explicitly when it's
  /// seeding: `livePeers` is remote peers only, so a healthy collection this
  /// device just created would otherwise read as zero copies alive.
  String get copiesLabel {
    if (!isComplete) return '${(progress * 100).round()}% · $peersLabel';
    return livePeers == 0
        ? 'Seeding · this device'
        : 'Seeding · this device + $peersLabel';
  }

  /// [media] regrouped back into the manifest entries it was flattened from —
  /// one per torrent, in first-appearance order.
  ///
  /// The flat list is what the grid renders, but an *entry* is the unit the
  /// collection actually grows by: one signed manifest entry, one info-hash,
  /// one collaborator's contribution. The details screen shows that structure,
  /// which is otherwise invisible once the files are flattened together.
  List<CollectionEntry> get entries {
    final byHash = <String, List<MediaItem>>{};
    for (final m in media) {
      byHash.putIfAbsent(m.infoHash, () => []).add(m);
    }
    return [
      for (final e in byHash.entries)
        CollectionEntry(
          infoHash: e.key,
          addedBy: e.value.first.addedBy,
          media: e.value,
        ),
    ];
  }
}

/// One manifest entry: a single torrent contributed to a collection, and the
/// files inside it. Derived from [Collection.media] rather than sent
/// separately — every field traces back to a `MediaInfo` from Rust.
class CollectionEntry {
  const CollectionEntry({
    required this.infoHash,
    required this.media,
    this.addedBy,
  });

  final String infoHash;
  final List<MediaItem> media;

  /// Device id of the collaborator who signed this entry — shared
  /// collections only.
  final String? addedBy;

  /// An entry is fetched once its torrent is in the local session; until then
  /// it's a single not-fetched placeholder standing for the whole entry.
  bool get fetched => media.any((m) => m.fetched);

  int get totalBytes =>
      media.fold(0, (sum, m) => sum + m.sizeBytes);

  int get downloadedBytes =>
      media.fold(0, (sum, m) => sum + m.downloadedBytes);

  double get progress =>
      totalBytes == 0 ? 0.0 : (downloadedBytes / totalBytes).clamp(0.0, 1.0);

  /// The entry's own signed label — the batch it was added as. Every file in
  /// the entry carries it, so any of them will do.
  String get label => media.isEmpty ? infoHash : media.first.entryLabel;
}
