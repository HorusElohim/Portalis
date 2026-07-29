import 'package:flutter/foundation.dart';

import '../bridge_generated/collab.dart' as bridge;
import '../bridge_generated/torrent.dart' as torrent_bridge;

/// Real, growable, invite-based Collections — Phase 1 of the "Add Collab"
/// plan (see `rust/backend/README.md`). Deliberately parallel to, not
/// merged with, [TorrentCollections]: this is a separate concept for now
/// (single-device only, no manifest-sync networking yet — a joined
/// collection stays empty until a later phase), so it doesn't touch the
/// already-working torrent flow while this is being built out. Unifying
/// the two is a later phase once sync actually works end to end.
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
}
