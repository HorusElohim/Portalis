import 'dart:async';

import 'package:flutter/foundation.dart';
import 'package:gal/gal.dart';

import '../bridge_generated/torrent.dart' as bridge;
import '../media_kind.dart';
import '../models.dart';

/// iOS/Android only — desktop already gets its own direct-to-Downloads
/// treatment on the Rust side (see `torrent.rs::output_dir`), and "the
/// Photos gallery" isn't a concept `dart:io`'s `Platform` is even safe to
/// probe on web (this check uses `defaultTargetPlatform` instead, which is
/// safe everywhere, including web).
bool get _hasPhotoGallery =>
    defaultTargetPlatform == TargetPlatform.iOS ||
    defaultTargetPlatform == TargetPlatform.android;

/// Turns real torrents into [Collection]s so the Torrent tab's "add a
/// magnet/.torrent file" flow feeds directly into the same Home/Collection/
/// MediaViewer screens as everything else — one collection experience, not
/// two. A torrent's files become a collection's media (this is exactly why
/// BitTorrent's own multi-file-torrent support maps so well onto
/// "collection": most torrents already are a bundle of related files).
///
/// This is still the debug/smoke-test path from `rust/backend/src/torrent.rs`
/// under the hood — no invite secrets, manifests, or collaborator identity
/// yet (see the backend README's open questions). It's real download
/// progress and real files, just not the final serverless-collection design.
class TorrentCollections extends ChangeNotifier {
  TorrentCollections._();
  static final instance = TorrentCollections._();

  List<Collection> _collections = [];
  List<Collection> get collections => List.unmodifiable(_collections);

  String? lastError;
  Timer? _timer;

  void start() {
    if (_timer != null) return;
    _refresh();
    _timer = Timer.periodic(const Duration(seconds: 1), (_) => _refresh());
  }

  /// Cancels polling without disposing the singleton (a real `dispose()`
  /// would make this permanently unusable — [ChangeNotifier] throws on any
  /// further `notifyListeners()` once disposed). `RootShell` calls this
  /// from its own `dispose()` so the timer doesn't leak past its widget's
  /// lifetime — most visible in widget tests, where each test's `RootShell`
  /// mount would otherwise leave a stray periodic timer running into the
  /// next test.
  void stop() {
    _timer?.cancel();
    _timer = null;
  }

  /// `"$infoHash:$fileName"` keys already saved to the gallery, so a file
  /// that finishes downloading gets imported exactly once rather than
  /// every polling tick for as long as it stays "ready".
  final Set<String> _importedToGallery = {};

  /// Guards against two `_importNewlyReadyMedia` calls running at once.
  /// `_refresh()` fires every second and each run is fire-and-forget
  /// (`unawaited`), so without this a second file finishing mid-import
  /// would start a second, overlapping `Gal.putImage`/`putVideo` call —
  /// concurrent calls into iOS's Photos framework are a known crash source
  /// (racing to create/find the same destination album). Skipping a
  /// re-entrant call is safe: any newly-ready file it would have caught is
  /// still unmarked and gets picked up on the very next non-overlapping
  /// tick.
  bool _importingToGallery = false;

  Future<void> _refresh() async {
    try {
      final torrents = await bridge.listTorrents();
      _collections = torrents.map(_toCollection).toList();
      lastError = null;
      if (_hasPhotoGallery) {
        unawaited(_importNewlyReadyMedia(torrents));
      }
    } catch (e) {
      lastError = '$e';
    }
    notifyListeners();
  }

  /// Saves newly-completed photos/videos into the Photos/gallery app, in an
  /// album named after the collection (auto-created if it doesn't exist).
  /// This is the mobile half of the storage design in the backend README —
  /// desktop writes straight to Downloads; mobile has no such folder, so the
  /// real user-visible destination is the system gallery instead.
  Future<void> _importNewlyReadyMedia(List<bridge.TorrentInfo> torrents) async {
    if (_importingToGallery) return;
    _importingToGallery = true;
    try {
      for (final torrent in torrents) {
        for (final file in torrent.files) {
          final key = '${torrent.infoHash}:${file.name}';
          if (_importedToGallery.contains(key)) continue;
          final ready = file.lengthBytes > BigInt.zero &&
              file.downloadedBytes >= file.lengthBytes;
          if (!ready) continue;
          if (!isImage(file.name) && !isVideo(file.name)) continue;

          // Mark before attempting, not after succeeding: a permission
          // denial or transient error would otherwise retry every polling
          // tick forever. Losing a genuinely-transient failure is an
          // acceptable trade for not spamming the OS with repeated prompts.
          _importedToGallery.add(key);
          try {
            // Sequential, one at a time (we're already inside the
            // `_importingToGallery` guard) — concurrent calls into iOS's
            // Photos framework are unsafe, see the field doc above.
            if (isImage(file.name)) {
              await Gal.putImage(file.absolutePath, album: torrent.name);
            } else {
              await Gal.putVideo(file.absolutePath, album: torrent.name);
            }
          } catch (_) {
            // Non-fatal — the file is still there and usable inside the
            // app even if the gallery copy failed (e.g. permission
            // denied).
          }
        }
      }
    } finally {
      _importingToGallery = false;
    }
  }

  Future<void> addFromMagnet(String magnetOrHash) async {
    await bridge.addTorrentFromMagnet(magnetOrHash: magnetOrHash);
    await _refresh();
  }

  Future<void> addFromFileBytes(Uint8List bytes) async {
    await bridge.addTorrentFromFileBytes(bytes: bytes);
    await _refresh();
  }

  /// The "share something" side: seed a brand-new collection built from
  /// local files (photos, videos, audio, anything) rather than joining an
  /// existing swarm. See `torrent.rs::create_collection` for how this
  /// produces the exact same `TorrentInfo` shape as joining does.
  Future<void> createCollection(
    String name,
    List<({String name, Uint8List bytes})> files,
  ) async {
    await bridge.createCollection(
      name: name,
      files: files
          .map((f) => bridge.NewFile(name: f.name, bytes: f.bytes))
          .toList(),
    );
    await _refresh();
  }

  Collection _toCollection(bridge.TorrentInfo info) {
    final total = info.totalBytes.toDouble();
    final progress =
        total > 0 ? (info.progressBytes.toDouble() / total).clamp(0.0, 1.0) : 0.0;

    final media = info.files.map((f) {
      final fileTotal = f.lengthBytes.toDouble();
      final fileProgress = fileTotal > 0
          ? (f.downloadedBytes.toDouble() / fileTotal).clamp(0.0, 1.0)
          : 0.0;
      return MediaItem(
        label: f.name,
        localPath: fileProgress >= 1.0 ? f.absolutePath : null,
        progress: fileProgress,
        sizeBytes: f.lengthBytes.toInt(),
        downloadedBytes: f.downloadedBytes.toInt(),
      );
    }).toList();

    // livePeers counts currently-connected *remote* peers — this device is
    // never in that set, so a freshly created (or idle) collection that's
    // seeding fine reports 0. Count ourselves explicitly: when finished,
    // this device itself is a live copy.
    final copiesLabel = info.finished
        ? (info.livePeers == 0
            ? 'Seeding · this device'
            : 'Seeding · this device + ${info.livePeers} peer${info.livePeers == 1 ? '' : 's'}')
        : '${(progress * 100).toStringAsFixed(0)}% · ${info.livePeers} peer${info.livePeers == 1 ? '' : 's'}';

    return Collection(
      name: info.name,
      subtitle: '${info.files.length} file${info.files.length == 1 ? '' : 's'}',
      categories: [info.state],
      hueIndex: info.infoHash.hashCode.abs(),
      copiesLabel: copiesLabel,
      collaboratorCount: info.livePeers,
      media: media,
      // Real peer identity isn't modeled yet — a torrent peer isn't a
      // named collaborator, just an IP:port. Swarm/peer screens degrade to
      // "no collaborators" for these rather than showing fabricated names.
      collaborators: const [],
      progress: progress,
      downloadedBytes: info.progressBytes.toInt(),
      uploadedBytes: info.uploadedBytes.toInt(),
      downloadMbps: info.downloadMbps,
      uploadMbps: info.uploadMbps,
      state: info.state,
      infoHash: info.infoHash,
    );
  }
}
