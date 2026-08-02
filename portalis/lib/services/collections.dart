import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:gal/gal.dart';

import '../bridge_generated/collections.dart' as bridge;
import '../bridge_generated/torrent.dart' as torrent_bridge;
import '../media/formats.dart';
import '../models.dart';

/// iOS/Android only — desktop already writes straight to Downloads (see
/// `torrent.rs::output_dir`), and "the Photos gallery" isn't a concept
/// `dart:io`'s `Platform` is even safe to probe on web. `defaultTargetPlatform`
/// is safe everywhere, including web.
bool get _hasPhotoGallery =>
    defaultTargetPlatform == TargetPlatform.iOS ||
    defaultTargetPlatform == TargetPlatform.android;

/// Whether [Collections.addFromMagnet] will accept this — a magnet URI, or the
/// bare 40-character info-hash librqbit's URL parser treats as one.
///
/// Lives here rather than in a screen because it is a fact about what the
/// backend takes, and two surfaces now offer to add a torrent: the full screen
/// and the desktop sidebar's inline field.
bool looksLikeMagnet(String value) {
  final v = value.trim();
  return v.startsWith('magnet:?') ||
      RegExp(r'^[0-9a-fA-F]{40}$').hasMatch(v);
}

/// The app's single source of collections.
///
/// Replaces the former `TorrentCollections` + `CollabCollections` pair, which
/// modelled the same user-facing concept twice and were never joined — so a
/// shared collection you created or joined never appeared in the collection
/// list at all. The join now happens in Rust (`collections.rs`), which is the
/// only layer that can see both the persisted manifest and the live
/// BitTorrent session; this class is a thin polling cache over it.
class Collections extends ChangeNotifier {
  Collections._();
  static final instance = Collections._();

  List<Collection> _collections = [];
  List<Collection> get collections => List.unmodifiable(_collections);

  /// Shared (invite-based) collections only — the ones that can be invited
  /// to, synced, and grown.
  List<Collection> get shared =>
      _collections.where((c) => c.isShared).toList(growable: false);

  String? lastError;

  /// Whether the transfer engine has finished starting.
  ///
  /// Until it has, *nothing* is being shared — the collections exist on disk
  /// but no torrent is live. Without this the UI showed those collections as
  /// though they were simply empty, which is why a freshly-launched app looked
  /// broken and it was never clear whether sharing had actually begun.
  bool engineReady = false;

  /// Hash of what the last poll saw, so an identical one can be dropped
  /// without allocating a description of it.
  int? _lastSeen;
  Timer? _timer;
  Duration _interval = _activeInterval;
  bool _paused = false;

  /// While bytes are moving, the numbers on screen are worth a second.
  static const _activeInterval = Duration(seconds: 1);

  /// When nothing is in flight, nothing changes second to second: a settled
  /// collection's size, peers and state are static. Polling five times less
  /// often costs an idle app five times fewer FFI round trips, JSON decodes,
  /// and rebuilds of every listening widget.
  static const _idleInterval = Duration(seconds: 5);

  void start() {
    if (_timer != null) return;
    // Bring the engine up before asking it anything. It used to come up as a
    // side effect of the first question, which meant the answer to that
    // question was taken before it was ready.
    //
    // Wrapped rather than awaited-and-caught: with no RustLib — every widget
    // test — the bridge throws *before* returning a future, so catchError
    // never sees it and the exception escapes into whatever called us. A
    // microtask and not a Future(): the test binding counts the latter as a
    // pending timer and fails the test that happened to be running.
    unawaited(Future.microtask(bridge.startEngine).catchError((_) {}));
    unawaited(refresh());
    _schedule(_interval);
  }

  void _schedule(Duration interval) {
    _timer?.cancel();
    _interval = interval;
    _timer = Timer.periodic(interval, (_) => refresh());
  }

  /// Stops polling entirely while the app isn't in front of the user.
  ///
  /// The engine keeps seeding — that happens in Rust and is the whole point —
  /// but there is no reason to wake the Dart isolate to redraw a list nobody
  /// is looking at.
  void setPaused(bool paused) {
    if (_paused == paused) return;
    _paused = paused;
    // Told, not inferred. The engine used to decide whether anyone was looking
    // by watching how recently this class had asked it for collections, which
    // made its network behaviour a side effect of how often a screen redrew.
    unawaited(
        Future.microtask(() => bridge.setActive(active: !paused)).catchError((_) {}));
    if (paused) {
      _timer?.cancel();
      _timer = null;
    } else {
      start();
      unawaited(refresh());
    }
  }

  /// Cancels polling without disposing the singleton — a real `dispose()`
  /// would make this permanently unusable, since [ChangeNotifier] throws on
  /// any further `notifyListeners()` once disposed.
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  /// Test seam: puts the cache into a known state and stops polling.
  ///
  /// Widget tests run without `RustLib`, so every backend call throws and the
  /// cache would otherwise be permanently empty-with-an-error — which makes
  /// the states worth testing (a live transfer, a filter, an empty list
  /// versus a failed load) unreachable. Seeding the cache directly keeps the
  /// tests honest about *rendering* without pretending the FFI layer works.
  @visibleForTesting
  void debugSeed(List<Collection> collections, {String? error}) {
    stop();
    _collections = List.of(collections);
    lastError = error;
    _lastSeen = null;
    notifyListeners();
  }

  /// What the last poll saw, so an identical one can be dropped.

  Future<void> refresh() async {
    try {
      final infos = await bridge.listCollections();
      _collections = infos.map(Collection.fromInfo).toList();
      engineReady = await bridge.engineReady();
      lastError = null;
      if (_hasPhotoGallery) {
        unawaited(_importNewlyReadyMedia());
      }
    } catch (e) {
      lastError = '$e';
    }
    _retuneInterval();
    if (_changed()) notifyListeners();
  }

  /// True when this poll differs from the last one. A settled app polls for
  /// minutes without anything moving, and every one of those polls used to
  /// rebuild every widget listening to this.
  bool _changed() {
    final seen = Object.hashAll(
        [lastError, engineReady, ..._collections.map((c) => c.signature)]);
    if (seen == _lastSeen) return false;
    _lastSeen = seen;
    return true;
  }

  /// Aggregate throughput right now, in MB/s. Drives both the ambient
  /// background and the polling cadence.
  double get liveRate => _collections.fold<double>(
      0, (sum, c) => sum + c.downloadMbps + c.uploadMbps);

  /// Speeds polling up while anything is moving and slows it down when
  /// everything settles. Only reschedules on an actual change of cadence, so
  /// a busy app isn't cancelling and recreating a timer every second.
  void _retuneInterval() {
    if (_paused || _timer == null) return;
    final wanted = liveRate > 0 ? _activeInterval : _idleInterval;
    if (wanted != _interval) _schedule(wanted);
  }

  Collection? byId(String id) {
    for (final c in _collections) {
      if (c.id == id) return c;
    }
    return null;
  }

  // ---------------------------------------------------------------------
  // Commands. Each returns after Rust has re-read the collection through the
  // same join `listCollections` uses, so the value handed back is exactly
  // what the list will show.
  // ---------------------------------------------------------------------

  /// Creates an empty shared collection.
  Future<Collection> create(String name) async {
    final info = await bridge.createCollection(name: name);
    await refresh();
    return Collection.fromInfo(info);
  }

  /// The "share something" flow: creates a shared collection *and* seeds the
  /// picked files into it as its first manifest entry — so what you share is
  /// invitable and can grow later, rather than a fixed one-off torrent.
  Future<Collection> createWithMedia(
    String name,
    List<({String name, Uint8List bytes})> files,
  ) async {
    final info = await bridge.createCollectionWithMedia(
      name: name,
      files: files
          .map((f) => torrent_bridge.NewFile(name: f.name, bytes: f.bytes))
          .toList(),
    );
    await refresh();
    return Collection.fromInfo(info);
  }

  Future<Collection> join(String inviteCode, String displayName) async {
    final info = await bridge.joinCollection(
      inviteCode: inviteCode,
      displayName: displayName,
    );
    await refresh();
    return Collection.fromInfo(info);
  }

  Future<Collection> addMedia(
    String collectionId,
    String label,
    List<({String name, Uint8List bytes})> files,
  ) async {
    final info = await bridge.addMediaToCollection(
      collectionId: collectionId,
      label: label,
      files: files
          .map((f) => torrent_bridge.NewFile(name: f.name, bytes: f.bytes))
          .toList(),
    );
    await refresh();
    return Collection.fromInfo(info);
  }

  /// Starts downloading every not-yet-fetched manifest entry. Returns how
  /// many were started.
  Future<int> fetchMedia(String collectionId) async {
    final count = await bridge.fetchCollectionMedia(collectionId: collectionId);
    await refresh();
    return count;
  }

  Future<Collection> sync(String collectionId, String peerAddr) async {
    final info = await bridge.syncCollection(
      collectionId: collectionId,
      peerAddr: peerAddr,
    );
    await refresh();
    return Collection.fromInfo(info);
  }

  Future<void> delete(String collectionId) async {
    await bridge.deleteCollection(collectionId: collectionId);
    await refresh();
  }

  /// This device's sync endpoints. Calling it starts the listener, which is
  /// what makes this device reachable by collaborators.
  Future<String> syncAddress() => bridge.syncAddress();

  /// Joining a plain BitTorrent swarm — surfaces as its own collection.
  Future<void> addFromMagnet(String magnetOrHash) async {
    await torrent_bridge.addTorrentFromMagnet(magnetOrHash: magnetOrHash);
    await refresh();
  }

  Future<void> addFromFileBytes(Uint8List bytes) async {
    await torrent_bridge.addTorrentFromFileBytes(bytes: bytes);
    await refresh();
  }

  // ---------------------------------------------------------------------
  // Gallery import (mobile)
  // ---------------------------------------------------------------------

  /// `"$infoHash:$fileName"` keys already saved to the gallery, so a file
  /// that finishes downloading is imported exactly once rather than on every
  /// polling tick for as long as it stays ready.
  final Set<String> _importedToGallery = {};

  /// Guards against two imports running at once. `refresh()` fires every
  /// second and each import is fire-and-forget, so without this a second file
  /// finishing mid-import would start an overlapping `Gal.putImage`/`putVideo`
  /// call — concurrent calls into iOS's Photos framework are a known crash
  /// source (racing to create or find the same destination album). Skipping a
  /// re-entrant call is safe: anything it would have caught is still unmarked
  /// and gets picked up on the next non-overlapping tick.
  bool _importingToGallery = false;

  /// Saves newly-completed photos/videos into the system gallery, in an album
  /// named after the collection. This is the mobile half of the storage design
  /// in the backend README — desktop writes straight to Downloads; mobile has
  /// no such folder, so the real user-visible destination is the gallery.
  Future<void> _importNewlyReadyMedia() async {
    if (_importingToGallery) return;
    _importingToGallery = true;
    try {
      for (final collection in _collections) {
        for (final media in collection.media) {
          final path = media.localPath;
          if (path == null || !media.isReady) continue;
          if (!isImage(media.label) && !isVideo(media.label)) continue;
          final key = '${media.infoHash}:${media.label}';
          if (_importedToGallery.contains(key)) continue;

          // Mark before attempting, not after succeeding: a permission denial
          // would otherwise retry every tick forever. Losing a genuinely
          // transient failure is an acceptable trade for not spamming the OS
          // with repeated prompts.
          _importedToGallery.add(key);
          try {
            // Sequential, one at a time — see the `_importingToGallery` note.
            if (isImage(media.label)) {
              await Gal.putImage(path, album: collection.name);
            } else {
              await Gal.putVideo(path, album: collection.name);
            }
          } catch (_) {
            // Non-fatal: the file is still there and usable inside the app
            // even if the gallery copy failed.
          }
        }
      }
    } finally {
      _importingToGallery = false;
    }
  }
}
