import 'dart:async';
import 'package:flutter/foundation.dart';

import '../data/collections_repository.dart';
import '../domain/collection.dart';
import '../domain/transfer_history.dart';
import '../platform/media_gallery_importer.dart';

/// Owns collection application state: lifecycle, polling cadence, commands,
/// and change notification. Native calls live in [CollectionsRepository] and
/// mobile gallery writes live in [MediaGalleryImporter].
class CollectionsController extends ChangeNotifier {
  CollectionsController({
    required CollectionsRepository repository,
    required MediaGalleryImporter galleryImporter,
  })  : _repository = repository,
        _galleryImporter = galleryImporter;

  factory CollectionsController.production() => CollectionsController(
        repository: const FrbCollectionsRepository(),
        galleryImporter: mediaGalleryImporterForCurrentPlatform(),
      );

  final CollectionsRepository _repository;
  final MediaGalleryImporter _galleryImporter;

  List<Collection> _collections = const [];
  final Map<String, TransferHistory> _transferHistories = {};
  List<Collection> get collections => List.unmodifiable(_collections);
  List<Collection> get shared =>
      _collections.where((collection) => collection.isShared).toList(growable: false);

  String? lastError;
  bool engineReady = false;

  int? _lastSeen;
  Timer? _timer;
  Duration _interval = _activeInterval;
  bool _paused = false;

  static const _activeInterval = Duration(milliseconds: 500);
  static const _idleInterval = Duration(seconds: 5);
  // Backgrounded still polls, just slowly and at a fixed rate — enough to
  // keep a transfer's numbers honest for whoever glances at a still-visible
  // window or comes back to it, without redrawing at foreground speed for a
  // window nobody is actively looking at.
  static const _backgroundInterval = Duration(seconds: 1);

  void start() {
    if (_timer != null) return;
    unawaited(Future.microtask(_repository.startEngine).catchError((_) {}));
    unawaited(refresh());
    _schedule(_paused ? _backgroundInterval : _interval);
  }

  void setPaused(bool paused) {
    if (_paused == paused) return;
    _paused = paused;
    unawaited(
      Future.microtask(() => _repository.setActive(!paused)).catchError((_) {}),
    );
    _schedule(paused ? _backgroundInterval : _activeInterval);
    unawaited(refresh());
  }

  /// Stops polling without disposing a controller that may be reused if the
  /// shell is rebuilt after a window-size change.
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  Future<void> refresh() async {
    var historyChanged = false;
    try {
      _collections = await _repository.list();
      historyChanged = _recordTransferHistory(_collections);
      engineReady = await _repository.isEngineReady();
      lastError = null;
      unawaited(_galleryImporter.importReadyMedia(_collections));
    } catch (error) {
      lastError = '$error';
    }
    _retuneInterval();
    if (_changed() || historyChanged) notifyListeners();
  }

  double get liveRate => _collections.fold<double>(
        0,
        (sum, collection) =>
            sum + collection.downloadMbps + collection.uploadMbps,
      );

  Collection? byId(String id) {
    for (final collection in _collections) {
      if (collection.id == id) return collection;
    }
    return null;
  }

  TransferHistory? historyFor(String collectionId) =>
      _transferHistories[collectionId];

  Future<Collection> create(String name) => _refreshAfter(
        () => _repository.create(name),
      );

  Future<Collection> createWithMedia(String name, CollectionFiles files) =>
      _refreshAfter(() => _repository.createWithMedia(name, files));

  Future<Collection> join(String inviteCode, String displayName) =>
      _refreshAfter(() => _repository.join(inviteCode, displayName));

  Future<Collection> addMedia(
    String collectionId,
    String label,
    CollectionFiles files,
  ) =>
      _refreshAfter(() => _repository.addMedia(collectionId, label, files));

  Future<int> fetchMedia(String collectionId) =>
      _refreshAfter(() => _repository.fetchMedia(collectionId));

  Future<Collection> sync(String collectionId, String peerAddress) =>
      _refreshAfter(() => _repository.sync(collectionId, peerAddress));

  Future<void> delete(String collectionId) =>
      _refreshAfter(() => _repository.delete(collectionId));

  Future<void> pause(String collectionId) =>
      _refreshAfter(() => _repository.pause(collectionId));

  Future<void> restart(String collectionId) =>
      _refreshAfter(() => _repository.restart(collectionId));

  Future<void> stopCollection(String collectionId) =>
      _refreshAfter(() => _repository.stop(collectionId));

  Future<void> deleteFiles(String collectionId) =>
      _refreshAfter(() => _repository.deleteFiles(collectionId));

  Future<String> syncAddress() => _repository.syncAddress();

  Future<void> addFromMagnet(String magnetOrHash) =>
      _refreshAfter(() => _repository.addTorrentFromMagnet(magnetOrHash));

  Future<void> addFromFileBytes(Uint8List bytes) =>
      _refreshAfter(() => _repository.addTorrentFromBytes(bytes));

  /// Test-only state injection. Production tests should prefer a repository
  /// fake and call [refresh], but this keeps the existing widget suite stable
  /// while its tests migrate away from global state.
  @visibleForTesting
  void debugSeed(List<Collection> collections, {String? error}) {
    stop();
    _transferHistories.clear();
    _collections = List.of(collections);
    lastError = error;
    _lastSeen = null;
    notifyListeners();
  }

  bool _recordTransferHistory(List<Collection> collections) {
    final now = DateTime.now();
    var changed = false;
    final ids = collections.map((collection) => collection.id).toSet();
    _transferHistories.removeWhere((id, _) => !ids.contains(id));

    for (final collection in collections) {
      final history = _transferHistories[collection.id];
      if (history?.completedAt != null) continue;
      final hasTransferStarted = history != null ||
          collection.downloadedBytes > 0 ||
          collection.downloadMbps > 0 ||
          collection.isComplete;
      if (!hasTransferStarted) continue;

      final active = history ?? TransferHistory(startedAt: now);
      _transferHistories[collection.id] = active;
      changed = active.record(
        at: now,
        downloadMbps: collection.downloadMbps,
        uploadMbps: collection.uploadMbps,
        progress: collection.progress,
      ) ||
          changed;
      if (collection.isComplete && active.completedAt == null) {
        active.completedAt = now;
        changed = true;
      }
    }
    return changed;
  }

  Future<T> _refreshAfter<T>(Future<T> Function() operation) async {
    final result = await operation();
    await refresh();
    return result;
  }

  void _schedule(Duration interval) {
    _timer?.cancel();
    _interval = interval;
    _timer = Timer.periodic(interval, (_) => refresh());
  }

  bool _changed() {
    final seen = Object.hashAll(
      [lastError, engineReady, ..._collections.map((collection) => collection.signature)],
    );
    if (seen == _lastSeen) return false;
    _lastSeen = seen;
    return true;
  }

  void _retuneInterval() {
    if (_paused || _timer == null) return;
    final wanted = liveRate > 0 ? _activeInterval : _idleInterval;
    if (wanted != _interval) _schedule(wanted);
  }
}
