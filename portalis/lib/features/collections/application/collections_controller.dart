import 'dart:async';
import 'package:flutter/foundation.dart';

import '../data/collections_repository.dart';
import '../data/peer_history_store.dart';
import '../data/transfer_history_store.dart';
import '../domain/collection.dart';
import '../domain/peer_observation.dart';
import '../domain/transfer_history.dart';

/// Owns collection application state: lifecycle, polling cadence, commands,
/// and change notification. Native calls live in [CollectionsRepository].
class CollectionsController extends ChangeNotifier {
  static const _peerWriteSpacing = Duration(seconds: 5);
  CollectionsController({
    required CollectionsRepository repository,
    required PeerHistoryStore peerHistoryStore,
    required TransferHistoryStore transferHistoryStore,
  })  : _repository = repository,
        _peerHistoryStore = peerHistoryStore,
        _transferHistoryStore = transferHistoryStore;

  factory CollectionsController.production() => CollectionsController(
        repository: const FrbCollectionsRepository(),
        peerHistoryStore: const SharedPreferencesPeerHistoryStore(),
        transferHistoryStore: const SharedPreferencesTransferHistoryStore(),
      );

  final CollectionsRepository _repository;
  final PeerHistoryStore _peerHistoryStore;
  final TransferHistoryStore _transferHistoryStore;

  List<Collection> _collections = const [];
  List<PeerObservation> _peerHistory = const [];
  final Set<String> _hiddenPeerAddresses = {};
  bool _peerHistoryLoaded = false;
  final Map<String, TransferHistory> _transferHistories = {};
  List<Collection> get collections => List.unmodifiable(_collections);
  List<Collection> get shared =>
      _collections.where((collection) => collection.isShared).toList(growable: false);

  String? lastError;
  bool engineReady = false;

  int? _lastSeen;
  Timer? _timer;
  Future<void>? _refreshing;
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
    unawaited(_loadPeerHistoryThenRefresh());
    _schedule(_paused ? _backgroundInterval : _interval);
  }

  Future<void> _loadPeerHistoryThenRefresh() async {
    try {
      _peerHistory = await _peerHistoryStore.load();
    } catch (_) {
      _peerHistory = const [];
    }
    try {
      _transferHistories.addAll(await _transferHistoryStore.load());
    } catch (_) {
      // A corrupt history file must not hide peer history or block startup.
    }
    _peerHistoryLoaded = true;
    await refresh();
    notifyListeners();
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

  /// Coalesces timer ticks and manual refreshes onto one native snapshot.
  ///
  /// The old periodic timer could start a second bridge call while the first
  /// one was still collecting torrent and peer stats. That made updates arrive
  /// late and occasionally let an older snapshot overwrite a newer one.
  Future<void> refresh() {
    final active = _refreshing;
    if (active != null) return active;

    final future = _refreshNow();
    _refreshing = future;
    future.then<void>(
      (_) {
        if (identical(_refreshing, future)) _refreshing = null;
      },
      onError: (Object _, StackTrace __) {
        if (identical(_refreshing, future)) _refreshing = null;
      },
    );
    return future;
  }

  Future<void> _refreshNow() async {
    var historyChanged = false;
    var peerHistoryChanged = false;
    try {
      final collectionsFuture = _repository.list();
      final engineReadyFuture = _repository.isEngineReady();
      _collections = await collectionsFuture;
      historyChanged = _recordTransferHistory(_collections);
      if (historyChanged) unawaited(_saveTransferHistories());
      peerHistoryChanged = _recordPeerHistory(_collections);
      engineReady = await engineReadyFuture;
      lastError = null;
    } catch (error) {
      lastError = '$error';
    }
    _retuneInterval();
    if (_changed() || historyChanged || peerHistoryChanged) notifyListeners();
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

  List<PeerObservation> peerHistoryFor(String collectionId) =>
      _visiblePeerHistory().where((peer) => peer.collectionId == collectionId).toList();

  List<PeerObservation> get peerHistory => _visiblePeerHistory();

  Future<void> forgetPeer(String address) async {
    _hiddenPeerAddresses.add(address);
    _peerHistory = _peerHistory.where((peer) => peer.address != address).toList();
    await _savePeerHistory();
    notifyListeners();
  }

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

  Future<void> delete(String collectionId) async {
    await _repository.delete(collectionId);
    _transferHistories.remove(collectionId);
    await _saveTransferHistories();
    await refresh();
  }

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

  Future<void> addFromFilePath(String path) =>
      _refreshAfter(() => _repository.addTorrentFromPath(path));

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

  List<PeerObservation> _visiblePeerHistory() {
    final byKey = <String, PeerObservation>{
      for (final peer in _peerHistory) _peerKey(peer): peer,
    };
    final now = DateTime.now();
    for (final collection in _collections) {
      for (final address in collection.torrentPeers) {
        if (_hiddenPeerAddresses.contains(address)) continue;
        final peer = PeerObservation(
          collectionId: collection.id,
          collectionName: collection.name,
          address: address,
          lastSeen: now,
        );
        byKey[_peerKey(peer)] = peer;
      }
    }
    return byKey.values.toList()
      ..sort((a, b) => b.lastSeen.compareTo(a.lastSeen));
  }

  bool _recordPeerHistory(List<Collection> collections) {
    if (!_peerHistoryLoaded) return false;
    final now = DateTime.now();
    final ids = collections.map((collection) => collection.id).toSet();
    final next = _peerHistory.where((peer) => ids.contains(peer.collectionId)).toList();
    final byKey = <String, PeerObservation>{
      for (final peer in next) _peerKey(peer): peer,
    };
    final liveAddresses = <String>{};
    var changed = next.length != _peerHistory.length;

    for (final collection in collections) {
      for (final address in collection.torrentPeers) {
        liveAddresses.add(address);
        if (_hiddenPeerAddresses.contains(address)) continue;
        final peer = PeerObservation(
          collectionId: collection.id,
          collectionName: collection.name,
          address: address,
          lastSeen: now,
        );
        final previous = byKey[_peerKey(peer)];
        final age = previous == null
            ? _peerWriteSpacing
            : (now.isAfter(previous.lastSeen)
                ? now.difference(previous.lastSeen)
                : previous.lastSeen.difference(now));
        if (previous == null ||
            age >= _peerWriteSpacing ||
            previous.collectionName != peer.collectionName) {
          byKey[_peerKey(peer)] = peer;
          changed = true;
        }
      }
    }
    _hiddenPeerAddresses.removeWhere((address) => !liveAddresses.contains(address));
    if (!changed) return false;
    _peerHistory = byKey.values.toList();
    unawaited(_savePeerHistory());
    return true;
  }

  Future<void> _savePeerHistory() async {
    try {
      await _peerHistoryStore.save(_peerHistory);
    } catch (_) {
      // Peer history is auxiliary UI state; a storage failure must not stop
      // transfers or make the collection list fail.
    }
  }

  Future<void> _saveTransferHistories() async {
    try {
      await _transferHistoryStore.save(_transferHistories);
    } catch (_) {
      // Transfer history is auxiliary UI state; transfer operation continues
      // even if the preferences store is temporarily unavailable.
    }
  }

  String _peerKey(PeerObservation peer) =>
      '${peer.collectionId}\u0000${peer.address}';

  bool _recordTransferHistory(List<Collection> collections) {
    final now = DateTime.now();
    var changed = false;
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
    final wanted = _collections.any((collection) => collection.isMoving)
        ? _activeInterval
        : _idleInterval;
    if (wanted != _interval) _schedule(wanted);
  }
}
