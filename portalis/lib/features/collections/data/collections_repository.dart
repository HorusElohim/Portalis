import 'dart:typed_data';

import '../../../bridge_generated/collections.dart' as bridge;
import '../../../bridge_generated/torrent.dart' as torrent_bridge;
import '../domain/collection.dart';
import 'collection_mapper.dart';

/// The collection operations the application needs from its native backend.
/// A controller depends on this interface rather than Flutter-Rust Bridge, so
/// its state transitions can be tested with a small in-memory fake.
abstract interface class CollectionsRepository {
  Future<void> startEngine();
  Future<void> setActive(bool active);
  Future<bool> isEngineReady();
  Future<List<Collection>> list();
  Future<Collection> create(String name);
  Future<Collection> createWithMedia(String name, CollectionFiles files);
  Future<Collection> join(String inviteCode, String displayName);
  Future<Collection> addMedia(
    String collectionId,
    String label,
    CollectionFiles files,
  );
  Future<int> fetchMedia(String collectionId);
  Future<Collection> sync(String collectionId, String peerAddress);
  Future<void> delete(String collectionId);
  Future<String> syncAddress();
  Future<void> addTorrentFromMagnet(String magnetOrHash);
  Future<void> addTorrentFromBytes(Uint8List bytes);
}

/// Bytes selected by the user before they are seeded. This stays at the
/// application boundary; a collection snapshot never contains source bytes.
typedef CollectionFiles = List<({String name, Uint8List bytes})>;

/// Production adapter. No widget, controller, or domain model imports the
/// generated bridge directly; any bridge schema change is contained here and
/// in [CollectionMapper].
class FrbCollectionsRepository implements CollectionsRepository {
  const FrbCollectionsRepository();

  @override
  Future<void> startEngine() => bridge.startEngine();

  @override
  Future<void> setActive(bool active) => bridge.setActive(active: active);

  @override
  Future<bool> isEngineReady() => bridge.engineReady();

  @override
  Future<List<Collection>> list() async =>
      (await bridge.listCollections()).map(CollectionMapper.fromInfo).toList();

  @override
  Future<Collection> create(String name) async =>
      CollectionMapper.fromInfo(await bridge.createCollection(name: name));

  @override
  Future<Collection> createWithMedia(String name, CollectionFiles files) async =>
      CollectionMapper.fromInfo(
        await bridge.createCollectionWithMedia(
          name: name,
          files: _files(files),
        ),
      );

  @override
  Future<Collection> join(String inviteCode, String displayName) async =>
      CollectionMapper.fromInfo(
        await bridge.joinCollection(
          inviteCode: inviteCode,
          displayName: displayName,
        ),
      );

  @override
  Future<Collection> addMedia(
    String collectionId,
    String label,
    CollectionFiles files,
  ) async =>
      CollectionMapper.fromInfo(
        await bridge.addMediaToCollection(
          collectionId: collectionId,
          label: label,
          files: _files(files),
        ),
      );

  @override
  Future<int> fetchMedia(String collectionId) =>
      bridge.fetchCollectionMedia(collectionId: collectionId);

  @override
  Future<Collection> sync(String collectionId, String peerAddress) async =>
      CollectionMapper.fromInfo(
        await bridge.syncCollection(
          collectionId: collectionId,
          peerAddr: peerAddress,
        ),
      );

  @override
  Future<void> delete(String collectionId) =>
      bridge.deleteCollection(collectionId: collectionId);

  @override
  Future<String> syncAddress() => bridge.syncAddress();

  @override
  Future<void> addTorrentFromMagnet(String magnetOrHash) =>
      torrent_bridge.addTorrentFromMagnet(magnetOrHash: magnetOrHash);

  @override
  Future<void> addTorrentFromBytes(Uint8List bytes) =>
      torrent_bridge.addTorrentFromFileBytes(bytes: bytes);

  List<torrent_bridge.NewFile> _files(CollectionFiles files) => files
      .map((file) => torrent_bridge.NewFile(name: file.name, bytes: file.bytes))
      .toList();
}
