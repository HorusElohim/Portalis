import '../../../bridge_generated/collections.dart' as collections_bridge;
import '../../../bridge_generated/settings.dart' as settings_bridge;
import '../../../bridge_generated/torrent.dart' as torrent_bridge;
import '../domain/engine_settings.dart';
import '../domain/storage_entry.dart';

/// Native operations needed by the settings feature.
abstract interface class SettingsRepository {
  Future<EngineSettings> load();
  Future<EngineSettings> defaults();
  Future<bool> save(EngineSettings settings);
  Future<int> storageUsageBytes();
  Future<List<StorageEntry>> storageBreakdown();
}

/// Flutter-Rust Bridge implementation. Generated types stay in this file.
class FrbSettingsRepository implements SettingsRepository {
  const FrbSettingsRepository();

  @override
  Future<EngineSettings> load() async =>
      _fromBridge(await settings_bridge.engineSettings());

  @override
  Future<EngineSettings> defaults() async =>
      _fromBridge(await settings_bridge.defaultEngineSettings());

  @override
  Future<bool> save(EngineSettings settings) =>
      settings_bridge.setEngineSettings(settings: _toBridge(settings));

  @override
  Future<int> storageUsageBytes() async =>
      (await torrent_bridge.storageUsageBytes()).toInt();

  @override
  Future<List<StorageEntry>> storageBreakdown() async =>
      (await collections_bridge.storageBreakdown())
          .map(
            (entry) => StorageEntry(
              name: entry.name,
              bytes: entry.bytes.toInt(),
              path: entry.path,
              collectionId: entry.collectionId,
              collectionName: entry.collectionName,
            ),
          )
          .toList(growable: false);

  static EngineSettings _fromBridge(settings_bridge.EngineSettings settings) =>
      EngineSettings(
        uploadLimitBps: settings.uploadLimitBps,
        downloadLimitBps: settings.downloadLimitBps,
        downloadDir: settings.downloadDir,
        listenPortStart: settings.listenPortStart,
        listenPortEnd: settings.listenPortEnd,
        enableUpnpPortForwarding: settings.enableUpnpPortForwarding,
        socksProxyUrl: settings.socksProxyUrl,
        disableDht: settings.disableDht,
        disableDhtPersistence: settings.disableDhtPersistence,
        persistSession: settings.persistSession,
        fastresume: settings.fastresume,
        deferWritesUpToMb: settings.deferWritesUpToMb,
        concurrentInitLimit: settings.concurrentInitLimit,
        peerConnectTimeoutSecs: settings.peerConnectTimeoutSecs,
        peerReadWriteTimeoutSecs: settings.peerReadWriteTimeoutSecs,
        peerKeepAliveIntervalSecs: settings.peerKeepAliveIntervalSecs,
        blocklistUrl: settings.blocklistUrl,
        trackers: List.unmodifiable(settings.trackers),
      );

  static settings_bridge.EngineSettings _toBridge(EngineSettings settings) =>
      settings_bridge.EngineSettings(
        uploadLimitBps: settings.uploadLimitBps,
        downloadLimitBps: settings.downloadLimitBps,
        downloadDir: settings.downloadDir,
        listenPortStart: settings.listenPortStart,
        listenPortEnd: settings.listenPortEnd,
        enableUpnpPortForwarding: settings.enableUpnpPortForwarding,
        socksProxyUrl: settings.socksProxyUrl,
        disableDht: settings.disableDht,
        disableDhtPersistence: settings.disableDhtPersistence,
        persistSession: settings.persistSession,
        fastresume: settings.fastresume,
        deferWritesUpToMb: settings.deferWritesUpToMb,
        concurrentInitLimit: settings.concurrentInitLimit,
        peerConnectTimeoutSecs: settings.peerConnectTimeoutSecs,
        peerReadWriteTimeoutSecs: settings.peerReadWriteTimeoutSecs,
        peerKeepAliveIntervalSecs: settings.peerKeepAliveIntervalSecs,
        blocklistUrl: settings.blocklistUrl,
        trackers: settings.trackers,
      );
}
