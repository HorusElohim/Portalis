/// Engine configuration independent of Flutter and the native bridge.
///
/// Rust remains the source of truth for validation and persistence. This
/// model gives the UI a stable contract and keeps generated DTOs at the data
/// boundary.
class EngineSettings {
  const EngineSettings({
    this.uploadLimitBps,
    this.downloadLimitBps,
    this.downloadDir,
    required this.listenPortStart,
    required this.listenPortEnd,
    required this.enableUpnpPortForwarding,
    this.socksProxyUrl,
    required this.disableDht,
    required this.disableDhtPersistence,
    required this.persistSession,
    required this.fastresume,
    this.deferWritesUpToMb,
    this.concurrentInitLimit,
    this.peerConnectTimeoutSecs,
    this.peerReadWriteTimeoutSecs,
    this.peerKeepAliveIntervalSecs,
    this.blocklistUrl,
    required this.trackers,
  });

  final int? uploadLimitBps;
  final int? downloadLimitBps;
  final String? downloadDir;
  final int listenPortStart;
  final int listenPortEnd;
  final bool enableUpnpPortForwarding;
  final String? socksProxyUrl;
  final bool disableDht;
  final bool disableDhtPersistence;
  final bool persistSession;
  final bool fastresume;
  final int? deferWritesUpToMb;
  final int? concurrentInitLimit;
  final int? peerConnectTimeoutSecs;
  final int? peerReadWriteTimeoutSecs;
  final int? peerKeepAliveIntervalSecs;
  final String? blocklistUrl;
  final List<String> trackers;

  /// A nullable field can deliberately be set to null. Omitting it preserves
  /// the current value.
  EngineSettings copyWith({
    Object? uploadLimitBps = _unchanged,
    Object? downloadLimitBps = _unchanged,
    Object? downloadDir = _unchanged,
    int? listenPortStart,
    int? listenPortEnd,
    bool? enableUpnpPortForwarding,
    Object? socksProxyUrl = _unchanged,
    bool? disableDht,
    bool? disableDhtPersistence,
    bool? persistSession,
    bool? fastresume,
    Object? deferWritesUpToMb = _unchanged,
    Object? concurrentInitLimit = _unchanged,
    Object? peerConnectTimeoutSecs = _unchanged,
    Object? peerReadWriteTimeoutSecs = _unchanged,
    Object? peerKeepAliveIntervalSecs = _unchanged,
    Object? blocklistUrl = _unchanged,
    List<String>? trackers,
  }) =>
      EngineSettings(
        uploadLimitBps: _value(uploadLimitBps, this.uploadLimitBps),
        downloadLimitBps: _value(downloadLimitBps, this.downloadLimitBps),
        downloadDir: _value(downloadDir, this.downloadDir),
        listenPortStart: listenPortStart ?? this.listenPortStart,
        listenPortEnd: listenPortEnd ?? this.listenPortEnd,
        enableUpnpPortForwarding:
            enableUpnpPortForwarding ?? this.enableUpnpPortForwarding,
        socksProxyUrl: _value(socksProxyUrl, this.socksProxyUrl),
        disableDht: disableDht ?? this.disableDht,
        disableDhtPersistence:
            disableDhtPersistence ?? this.disableDhtPersistence,
        persistSession: persistSession ?? this.persistSession,
        fastresume: fastresume ?? this.fastresume,
        deferWritesUpToMb: _value(deferWritesUpToMb, this.deferWritesUpToMb),
        concurrentInitLimit:
            _value(concurrentInitLimit, this.concurrentInitLimit),
        peerConnectTimeoutSecs:
            _value(peerConnectTimeoutSecs, this.peerConnectTimeoutSecs),
        peerReadWriteTimeoutSecs: _value(
          peerReadWriteTimeoutSecs,
          this.peerReadWriteTimeoutSecs,
        ),
        peerKeepAliveIntervalSecs: _value(
          peerKeepAliveIntervalSecs,
          this.peerKeepAliveIntervalSecs,
        ),
        blocklistUrl: _value(blocklistUrl, this.blocklistUrl),
        trackers: trackers ?? this.trackers,
      );

  static T? _value<T>(Object? next, T? current) =>
      identical(next, _unchanged) ? current : next as T?;
}

const _unchanged = Object();
