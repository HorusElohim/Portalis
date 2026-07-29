import 'dart:async';

import 'package:flutter/foundation.dart';

import '../bridge_generated/torrent.dart' as bridge;
import '../models.dart';

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

  Future<void> _refresh() async {
    try {
      final torrents = await bridge.listTorrents();
      _collections = torrents.map(_toCollection).toList();
      lastError = null;
    } catch (e) {
      lastError = '$e';
    }
    notifyListeners();
  }

  Future<void> addFromMagnet(String magnetOrHash) async {
    await bridge.addTorrentFromMagnet(magnetOrHash: magnetOrHash);
    await _refresh();
  }

  Future<void> addFromFileBytes(Uint8List bytes) async {
    await bridge.addTorrentFromFileBytes(bytes: bytes);
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
      );
    }).toList();

    final copiesLabel = info.finished
        ? '${info.livePeers} peer${info.livePeers == 1 ? '' : 's'} seeding'
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
    );
  }
}
