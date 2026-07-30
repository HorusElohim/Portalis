import 'package:flutter/foundation.dart';

import '../bridge_generated/collab.dart' as bridge;
import '../bridge_generated/torrent.dart' as torrent_bridge;

/// Real, growable, invite-based Collections — Phases 1–2 of the "Add
/// Collab" plan (see `rust/backend/README.md`). Deliberately parallel to,
/// not merged with, [TorrentCollections] — unifying the two is a later
/// phase. As of Phase 2, two devices holding the same invite can really
/// exchange manifests over the LAN ([sync]), using a manually-entered
/// peer address ([syncAddress]) — Phase 3's DHT rendezvous removes the
/// manual-address step.
class CollabCollections extends ChangeNotifier {
  CollabCollections._();
  static final instance = CollabCollections._();

  List<bridge.CollabCollectionInfo> _collections = [];
  List<bridge.CollabCollectionInfo> get collections =>
      List.unmodifiable(_collections);

  String? lastError;

  Future<void> refresh() async {
    try {
      _collections = await bridge.listCollabCollections();
      lastError = null;
    } catch (e) {
      lastError = '$e';
    }
    notifyListeners();
  }

  Future<bridge.CollabCollectionInfo> createCollection(String name) async {
    final info = await bridge.createCollabCollection(name: name);
    await refresh();
    return info;
  }

  /// Reuses an existing collab collection with this exact [name] if one's
  /// already been created, instead of minting a duplicate — Phase 1 has no
  /// real stored mapping between a torrent-backed Collection and its collab
  /// counterpart yet, so name is the closest proxy available. Refreshes
  /// first so the match (and its invite code, which is regenerated with
  /// this device's *current* sync addresses on every list/refresh call) is
  /// never based on stale in-memory data.
  Future<bridge.CollabCollectionInfo> createOrReuseCollection(String name) async {
    await refresh();
    final existing = _collections.where((c) => c.name == name);
    if (existing.isNotEmpty) return existing.first;
    return createCollection(name);
  }

  Future<void> deleteCollection(String collectionId) async {
    await bridge.deleteCollabCollection(collectionId: collectionId);
    await refresh();
  }

  Future<bridge.CollabCollectionInfo> joinCollection(
    String inviteCode,
    String displayName,
  ) async {
    final info = await bridge.joinCollabCollection(
      inviteCode: inviteCode,
      displayName: displayName,
    );
    await refresh();
    return info;
  }

  Future<bridge.CollabCollectionInfo> addMedia(
    String collectionId,
    String label,
    List<({String name, Uint8List bytes})> files,
  ) async {
    final info = await bridge.addMediaToCollabCollection(
      collectionId: collectionId,
      label: label,
      files: files
          .map((f) => torrent_bridge.NewFile(name: f.name, bytes: f.bytes))
          .toList(),
    );
    await refresh();
    return info;
  }

  /// This device's `ip:port` for incoming syncs — calling this is also what
  /// starts the listener, so the screen showing it makes the device
  /// reachable.
  Future<String> syncAddress() => bridge.collabSyncAddress();

  /// One full manifest exchange with the device at [peerAddr] (its
  /// [syncAddress] value). Both sides end up with the union of entries and
  /// collaborators.
  Future<bridge.CollabCollectionInfo> sync(
    String collectionId,
    String peerAddr,
  ) async {
    final info = await bridge.syncCollabCollection(
      collectionId: collectionId,
      peerAddr: peerAddr,
    );
    await refresh();
    return info;
  }

  /// Starts downloading every media item in a synced collection over
  /// ordinary BitTorrent. Rust hands librqbit the peer addresses learned
  /// during sync as direct connection hints, so a LAN fetch connects to
  /// the device that has the files immediately — no DHT wait. Returns how
  /// many downloads were started (items already added just re-resolve to
  /// the same torrent).
  Future<int> fetchAllMedia(bridge.CollabCollectionInfo collection) =>
      bridge.fetchCollabCollectionMedia(collectionId: collection.id);
}
