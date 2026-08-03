import 'dart:async';
import 'dart:math' as math;

import 'package:flutter/material.dart';

import '../bridge_generated/collections.dart' as bridge;
import '../services/collections.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import '../ui/ui.dart';



/// Every setting the BitTorrent engine honours, and nothing else.
///
/// Each control maps to a real librqbit `SessionOptions` field via
/// `rust/backend/src/settings.rs` — no app-invented preferences that persist
/// a value nothing reads. Sections are ordered by how likely they are to
/// matter, with the two live-adjustable rate limits first; everything below
/// them is read once at session construction, which the UI states rather than
/// implying an immediate effect.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    this.embedded = false,
    this.advanced = false,
  });

  /// Rendered inside the desktop shell's centre pane: no Scaffold chrome and
  /// no back button, because the sidebar is the navigation.
  final bool embedded;

  /// The engine internals, reached from "Network & engine". Same state class
  /// and the same editors — only which sections are shown differs.
  final bool advanced;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _settings = SettingsService.instance;
  Timer? _storagePoll;
  bool _restartPending = false;

  /// Which sections show. Starts from [SettingsScreen.advanced] but, when
  /// embedded, toggles in place rather than through a second pushed
  /// instance — see [_openAdvanced].
  late bool _advanced = widget.advanced;

  @override
  void initState() {
    super.initState();
    if (!_settings.loaded) _settings.load();
    // Storage grows as downloads finish in the background.
    _storagePoll = Timer.periodic(
      const Duration(seconds: 2),
      (_) => _settings.refreshStorageUsage(),
    );
  }

  @override
  void dispose() {
    _storagePoll?.cancel();
    super.dispose();
  }

  /// "Network & engine" used to always push a second [SettingsScreen] with
  /// `advanced: true` — a full-screen route over the desktop shell's sidebar
  /// and collection list even when this instance was already embedded in its
  /// centre pane. Embedded, there is nowhere better for that route to go, so
  /// it toggles this instance's own sections instead; pushed (mobile), the
  /// drill-down keeps its own back-stack entry as before.
  void _openAdvanced() {
    if (widget.embedded) {
      setState(() => _advanced = true);
    } else {
      Navigator.of(context).push(
        MaterialPageRoute(builder: (_) => const SettingsScreen(advanced: true)),
      );
    }
  }

  /// Embedded and showing Advanced: collapse back to Basic in place — there
  /// is no pushed route here to pop. Embedded and already Basic: no back
  /// button at all, since the sidebar is the only way in or out of this pane.
  /// Not embedded: always a real pushed screen, so always pop.
  bool get _showBackButton => !widget.embedded || _advanced;

  void _handleBack() {
    if (widget.embedded) {
      setState(() => _advanced = false);
    } else {
      Navigator.of(context).pop();
    }
  }

  Future<void> _apply(EngineSettings next) async {
    try {
      final restartRequired = await _settings.save(next);
      if (restartRequired && mounted) setState(() => _restartPending = true);
    } catch (e) {
      if (!mounted) return;
      showToast(context, 'Couldn\'t save: $e',
          severity: ToastSeverity.error);
    }
  }

  /// Shared editor for the optional numeric/text fields. Returns the raw
  /// string, or null if cancelled; an empty result means "unset".
  Future<String?> _edit({
    required String title,
    required String? current,
    String? hint,
    String? helper,
    TextInputType? keyboard,
    int maxLines = 1,
  }) {
    final controller = TextEditingController(text: current ?? '');
    return showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text(title),
        content: SizedBox(
          width: 320,
          child: TextField(
            controller: controller,
            autofocus: true,
            keyboardType: keyboard,
            maxLines: maxLines,
            style: const TextStyle(
                color: AppColors.text, fontSize: 13, fontFamily: 'monospace'),
            decoration: InputDecoration(
              hintText: hint,
              hintStyle: const TextStyle(color: AppColors.textGhost),
              helperText: helper,
              helperMaxLines: 3,
              helperStyle:
                  const TextStyle(fontSize: 10.5, color: AppColors.textDim),
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(controller.text.trim()),
            child: const Text('Save'),
          ),
        ],
      ),
    );
  }

  Future<void> _confirmReset(EngineSettings _) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Reset engine settings?'),
        content: const Text(
          'Restores every value below to the built-in default. Your '
          'collections and identity are untouched.',
          style: TextStyle(fontSize: 12, color: AppColors.textDim),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Reset'),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      final restartRequired = await _settings.resetToDefaults();
      if (restartRequired && mounted) setState(() => _restartPending = true);
    } catch (e) {
      if (!mounted) return;
      showToast(context, 'Couldn\'t reset: $e',
          severity: ToastSeverity.error);
    }
  }

  /// Times `listCollections` across the Dart<->Rust bridge — the call the
  /// UI polls every second (see `Collections._activeInterval`) and the one
  /// `docs/future-engine.md` names as the app's actual performance cost:
  /// every collection's full manifest, joined against the live session and
  /// marshalled across FFI, on every tick.
  static const _benchIterations = 10;

  Future<void> _runBenchmark() async {
    final samplesUs = <int>[];
    int dtoCount = 0;
    List<bridge.CollectionInfo> infos = const [];
    try {
      for (var i = 0; i < _benchIterations; i++) {
        final sw = Stopwatch()..start();
        infos = await bridge.listCollections();
        sw.stop();
        samplesUs.add(sw.elapsedMicroseconds);
      }
      dtoCount = infos.length +
          infos.fold<int>(
              0, (sum, c) => sum + c.media.length + c.collaborators.length);
    } catch (e) {
      if (!mounted) return;
      showToast(context, 'Benchmark failed: $e', severity: ToastSeverity.error);
      return;
    }
    if (!mounted) return;

    final avgMs = samplesUs.reduce((a, b) => a + b) / samplesUs.length / 1000;
    final minMs = samplesUs.reduce(math.min) / 1000;
    final maxMs = samplesUs.reduce(math.max) / 1000;

    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('listCollections benchmark'),
        content: SizedBox(
          width: 300,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                '$_benchIterations calls, ${infos.length} collection'
                '${infos.length == 1 ? '' : 's'} each.',
                style: const TextStyle(fontSize: 12, color: AppColors.textDim),
              ),
              const SizedBox(height: 12),
              Text(
                'Average    ${avgMs.toStringAsFixed(2)} ms\n'
                'Min        ${minMs.toStringAsFixed(2)} ms\n'
                'Max        ${maxMs.toStringAsFixed(2)} ms\n'
                'Calls/sec  ${(1000 / avgMs).toStringAsFixed(1)}\n'
                'DTOs/call  $dtoCount',
                style: monoLabel(
                    size: 12.5, color: AppColors.text, letterSpacing: 0),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Close'),
          ),
        ],
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(
        child: PageBody(
          child: ListenableBuilder(
            listenable: _settings,
            builder: (context, _) {
              final s = _settings.settings;
              return SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    if (_showBackButton)
                      Align(
                        alignment: Alignment.centerLeft,
                        child: NavBackButton(onTap: _handleBack),
                      ),
                    const Padding(
                      padding: EdgeInsets.fromLTRB(20, 6, 20, 6),
                      child: Text(
                        'Settings',
                        style:
                            TextStyle(fontSize: 20, fontWeight: FontWeight.w500),
                      ),
                    ),
                    if (_settings.lastError != null)
                      InfoBanner(
                        color: const Color(0xFFEB5757),
                        icon: Icons.error_outline,
                        text: _settings.lastError!,
                      ),
                    if (_restartPending)
                      const InfoBanner(
                        color: AppColors.signalSoft,
                        icon: Icons.restart_alt,
                        text: 'Some changes apply the next time Portalis '
                            'starts — the transfer engine reads them once, '
                            'when it starts up.',
                      ),
                    if (s == null)
                      const Padding(
                        padding: EdgeInsets.all(20),
                        child: Text(
                          'Loading engine settings…',
                          style: TextStyle(
                              fontSize: 12, color: AppColors.textDim),
                        ),
                      )
                    else
                      ..._sections(s),
                    const SizedBox(height: 24),
                  ],
                ),
              );
            },
          ),
        ),
      ),
    );
  }

  List<Widget> _sections(EngineSettings s) =>
      _advanced ? _advancedSections(s) : _basicSections(s);

  /// What most people ever need: how fast, and whether to keep sharing.
  List<Widget> _basicSections(EngineSettings s) {
    return [
      _HealthCard(settings: s),
      SettingsSection(
        label: 'SPEED · APPLIES IMMEDIATELY',
        children: [
          ValueRow(
            label: 'Upload limit',
            value: formatLimit(s.uploadLimitBps),
            subtitle: 'Across all torrents, not per torrent.',
            onTap: () async {
              final raw = await _edit(
                title: 'Upload limit',
                current: s.uploadLimitBps?.toString(),
                hint: 'bytes per second',
                helper: 'Leave empty for unlimited.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s, uploadLimitBps: _parseInt(raw), clearUpload: raw.isEmpty));
            },
          ),
          ValueRow(
            label: 'Download limit',
            value: formatLimit(s.downloadLimitBps),
            subtitle: 'Across all torrents, not per torrent.',
            onTap: () async {
              final raw = await _edit(
                title: 'Download limit',
                current: s.downloadLimitBps?.toString(),
                hint: 'bytes per second',
                helper: 'Leave empty for unlimited.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s, downloadLimitBps: _parseInt(raw), clearDownload: raw.isEmpty));
            },
          ),
        ],
      ),
      SettingsSection(
        label: 'SHARING',
        children: [
          SwitchRow(
            label: 'Keep sharing after restart',
            subtitle: 'Friends can still pull your collections when Portalis '
                'starts again. Off means the engine forgets them and silently '
                'seeds nothing.',
            value: s.persistSession,
            onChanged: (v) => _apply(_copy(s, persistSession: v)),
          ),
        ],
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
        child: SurfaceCard(
          onTap: _openAdvanced,
          padding: const EdgeInsets.all(16),
          child: Row(
            children: [
              const Icon(Icons.tune, size: 19, color: AppColors.textDim),
              const SizedBox(width: 13),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text('Network & engine',
                        style: TextStyle(
                            fontSize: 14.5, fontWeight: FontWeight.w600)),
                    const SizedBox(height: 3),
                    Text('Ports, DHT, proxy, trackers, disk',
                        style: const TextStyle(
                            fontSize: 12.5, color: AppColors.textFaint)),
                  ],
                ),
              ),
              const Icon(Icons.chevron_right,
                  size: 16, color: AppColors.textGhost),
            ],
          ),
        ),
      ),
      SettingsSection(
        label: 'STORAGE',
        children: [
          ValueRow(
            label: 'Storage used',
            value: formatBytes(_settings.storageUsedBytes),
            subtitle: 'Reported by the engine. Not capped by anything.',
          ),
        ],
      ),
    ];
  }

  /// Everything librqbit exposes. Every row here is construction-time, so the
  /// restart banner is expected rather than exceptional.
  List<Widget> _advancedSections(EngineSettings s) {
    return [
      SettingsSection(
        label: 'NETWORK · NEEDS RESTART',
        children: [
          ValueRow(
            label: 'Listen ports',
            value: '${s.listenPortStart}–${s.listenPortEnd}',
            subtitle:
                'The first free port in this range accepts incoming peers. '
                'Without one, nobody can download from this device.',
            onTap: () async {
              final raw = await _edit(
                title: 'Listen port range',
                current: '${s.listenPortStart}-${s.listenPortEnd}',
                hint: '6881-6999',
                helper: 'start-end',
              );
              if (raw == null || raw.isEmpty) return;
              final parts = raw.split(RegExp(r'[-u2013s]+'));
              final start = _parseInt(parts.first);
              final end = parts.length > 1 ? _parseInt(parts[1]) : start;
              if (start == null || end == null) return;
              await _apply(_copy(s, listenPortStart: start, listenPortEnd: end));
            },
          ),
          SwitchRow(
            label: 'UPnP port forwarding',
            subtitle: 'Ask the router to forward the listen port. No effect '
                'if the router has UPnP off, or while a VPN owns the default '
                'route.',
            value: s.enableUpnpPortForwarding,
            onChanged: (v) => _apply(_copy(s, enableUpnpPortForwarding: v)),
          ),
          ValueRow(
            label: 'SOCKS5 proxy',
            value: s.socksProxyUrl ?? 'None',
            subtitle: 'Routes peer traffic through a proxy.',
            onTap: () async {
              final raw = await _edit(
                title: 'SOCKS5 proxy',
                current: s.socksProxyUrl,
                hint: 'socks5://[user:pass@]host:port',
                helper: 'Leave empty to connect directly.',
              );
              if (raw == null) return;
              await _apply(_copy(s, socksProxyUrl: raw, clearProxy: raw.isEmpty));
            },
          ),
        ],
      ),
      SettingsSection(
        label: 'PEER DISCOVERY · NEEDS RESTART',
        children: [
          SwitchRow(
            label: 'Disable DHT',
            subtitle: 'Without the distributed hash table, peers are only '
                'found via trackers or addresses shared directly.',
            value: s.disableDht,
            onChanged: (v) => _apply(_copy(s, disableDht: v)),
          ),
          SwitchRow(
            label: 'Disable DHT persistence',
            subtitle: 'Stop reusing the stored DHT identity and port between '
                'runs. That stored port is why two copies of Portalis can\'t '
                'run at once.',
            value: s.disableDhtPersistence,
            onChanged: (v) => _apply(_copy(s, disableDhtPersistence: v)),
          ),
          ValueRow(
            label: 'Extra trackers',
            value: s.trackers.isEmpty
                ? 'None'
                : '${s.trackers.length} tracker'
                    '${s.trackers.length == 1 ? '' : 's'}',
            subtitle: 'Added to every torrent, on top of any it already lists.',
            onTap: () async {
              final raw = await _edit(
                title: 'Extra trackers',
                current: s.trackers.join('\n'),
                hint: 'udp://tracker.example:1337',
                helper: 'One URL per line.',
                maxLines: 5,
              );
              if (raw == null) return;
              final trackers = raw
                  .split('\n')
                  .map((t) => t.trim())
                  .where((t) => t.isNotEmpty)
                  .toList();
              await _apply(_copy(s, trackers: trackers));
            },
          ),
          ValueRow(
            label: 'Blocklist URL',
            value: s.blocklistUrl ?? 'None',
            subtitle: 'An IP blocklist to fetch and enforce.',
            onTap: () async {
              final raw = await _edit(
                title: 'Blocklist URL',
                current: s.blocklistUrl,
                hint: 'https://example.com/blocklist.txt',
                helper: 'Leave empty for none.',
              );
              if (raw == null) return;
              await _apply(_copy(s, blocklistUrl: raw, clearBlocklist: raw.isEmpty));
            },
          ),
        ],
      ),
      SettingsSection(
        label: 'SESSION · NEEDS RESTART',
        children: [
          SwitchRow(
            label: 'Fast resume',
            subtitle: 'Trust the saved piece state instead of re-hashing every '
                'file at launch.',
            value: s.fastresume,
            onChanged: (v) => _apply(_copy(s, fastresume: v)),
          ),
        ],
      ),
      SettingsSection(
        label: 'PERFORMANCE · NEEDS RESTART',
        children: [
          ValueRow(
            label: 'Deferred writes',
            value: s.deferWritesUpToMb == null
                ? 'Write through'
                : '${s.deferWritesUpToMb} MB',
            subtitle: 'Buffer writes in memory instead of writing straight to '
                'disk.',
            onTap: () async {
              final raw = await _edit(
                title: 'Deferred writes',
                current: s.deferWritesUpToMb?.toString(),
                hint: 'megabytes',
                helper: 'Leave empty to write straight through.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s,
                  deferWritesUpToMb: _parseInt(raw), clearDefer: raw.isEmpty));
            },
          ),
          ValueRow(
            label: 'Concurrent inits',
            value: s.concurrentInitLimit?.toString() ?? 'Engine default',
            subtitle: 'How many torrents may start up at once.',
            onTap: () async {
              final raw = await _edit(
                title: 'Concurrent initialisations',
                current: s.concurrentInitLimit?.toString(),
                hint: 'count',
                helper: 'Leave empty for the engine default.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s,
                  concurrentInitLimit: _parseInt(raw), clearInit: raw.isEmpty));
            },
          ),
        ],
      ),
      SettingsSection(
        label: 'PEER TIMEOUTS · NEEDS RESTART',
        children: [
          ValueRow(
            label: 'Connect',
            value: _secs(s.peerConnectTimeoutSecs),
            onTap: () async {
              final raw = await _edit(
                title: 'Peer connect timeout',
                current: s.peerConnectTimeoutSecs?.toString(),
                hint: 'seconds',
                helper: 'Leave empty for the engine default.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s,
                  peerConnectTimeoutSecs: _parseInt(raw),
                  clearConnect: raw.isEmpty));
            },
          ),
          ValueRow(
            label: 'Read / write',
            value: _secs(s.peerReadWriteTimeoutSecs),
            onTap: () async {
              final raw = await _edit(
                title: 'Peer read/write timeout',
                current: s.peerReadWriteTimeoutSecs?.toString(),
                hint: 'seconds',
                helper: 'Leave empty for the engine default.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s,
                  peerReadWriteTimeoutSecs: _parseInt(raw),
                  clearReadWrite: raw.isEmpty));
            },
          ),
          ValueRow(
            label: 'Keep-alive',
            value: _secs(s.peerKeepAliveIntervalSecs),
            onTap: () async {
              final raw = await _edit(
                title: 'Peer keep-alive interval',
                current: s.peerKeepAliveIntervalSecs?.toString(),
                hint: 'seconds',
                helper: 'Leave empty for the engine default.',
                keyboard: TextInputType.number,
              );
              if (raw == null) return;
              await _apply(_copy(s,
                  peerKeepAliveIntervalSecs: _parseInt(raw),
                  clearKeepAlive: raw.isEmpty));
            },
          ),
        ],
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
        child: PillButton(
          label: 'Reset to defaults',
          dim: true,
          onTap: () => _confirmReset(s),
        ),
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(20, 10, 20, 0),
        child: PillButton(
          label: 'Run FFI benchmark',
          dim: true,
          onTap: _runBenchmark,
        ),
      ),
    ];
  }

  static String _secs(int? v) => v == null ? 'Engine default' : '${v}s';

  static int? _parseInt(String raw) {
    final v = int.tryParse(raw.trim());
    return (v == null || v <= 0) ? null : v;
  }

  /// FRB's generated DTO has no `copyWith`, and every field is final, so this
  /// rebuilds it. The `clear*` flags exist because a null argument can't
  /// distinguish "leave alone" from "unset" for the optional fields.
  static EngineSettings _copy(
    EngineSettings s, {
    int? uploadLimitBps,
    bool clearUpload = false,
    int? downloadLimitBps,
    bool clearDownload = false,
    int? listenPortStart,
    int? listenPortEnd,
    bool? enableUpnpPortForwarding,
    String? socksProxyUrl,
    bool clearProxy = false,
    bool? disableDht,
    bool? disableDhtPersistence,
    bool? persistSession,
    bool? fastresume,
    int? deferWritesUpToMb,
    bool clearDefer = false,
    int? concurrentInitLimit,
    bool clearInit = false,
    int? peerConnectTimeoutSecs,
    bool clearConnect = false,
    int? peerReadWriteTimeoutSecs,
    bool clearReadWrite = false,
    int? peerKeepAliveIntervalSecs,
    bool clearKeepAlive = false,
    String? blocklistUrl,
    bool clearBlocklist = false,
    List<String>? trackers,
  }) {
    return EngineSettings(
      uploadLimitBps: clearUpload ? null : (uploadLimitBps ?? s.uploadLimitBps),
      downloadLimitBps:
          clearDownload ? null : (downloadLimitBps ?? s.downloadLimitBps),
      listenPortStart: listenPortStart ?? s.listenPortStart,
      listenPortEnd: listenPortEnd ?? s.listenPortEnd,
      enableUpnpPortForwarding:
          enableUpnpPortForwarding ?? s.enableUpnpPortForwarding,
      socksProxyUrl: clearProxy ? null : (socksProxyUrl ?? s.socksProxyUrl),
      disableDht: disableDht ?? s.disableDht,
      disableDhtPersistence: disableDhtPersistence ?? s.disableDhtPersistence,
      persistSession: persistSession ?? s.persistSession,
      fastresume: fastresume ?? s.fastresume,
      deferWritesUpToMb:
          clearDefer ? null : (deferWritesUpToMb ?? s.deferWritesUpToMb),
      concurrentInitLimit:
          clearInit ? null : (concurrentInitLimit ?? s.concurrentInitLimit),
      peerConnectTimeoutSecs: clearConnect
          ? null
          : (peerConnectTimeoutSecs ?? s.peerConnectTimeoutSecs),
      peerReadWriteTimeoutSecs: clearReadWrite
          ? null
          : (peerReadWriteTimeoutSecs ?? s.peerReadWriteTimeoutSecs),
      peerKeepAliveIntervalSecs: clearKeepAlive
          ? null
          : (peerKeepAliveIntervalSecs ?? s.peerKeepAliveIntervalSecs),
      blocklistUrl: clearBlocklist ? null : (blocklistUrl ?? s.blocklistUrl),
      trackers: trackers ?? s.trackers,
    );
  }
}




/// A label/value row; tappable when [onTap] is given, read-only otherwise.

/// A summary of what the engine is actually doing.
///
/// The design's version claimed "Everything is healthy · PORT OPEN · DHT ON ·
/// 14 PEERS". Two of those three are knowable and one is not: nothing here
/// verifies that the listen port is *reachable* from outside, only which port
/// range was configured. So this states the configured port and the real DHT
/// and peer figures, and never asserts overall health.
class _HealthCard extends StatelessWidget {
  const _HealthCard({required this.settings});

  final EngineSettings settings;

  @override
  Widget build(BuildContext context) {
    final peers = Collections.instance.collections
        .fold<int>(0, (sum, c) => sum + c.livePeers);
    final dhtOn = !settings.disableDht;
    final facts = [
      'PORT ${settings.listenPortStart}–${settings.listenPortEnd}',
      dhtOn ? 'DHT ON' : 'DHT OFF',
      plural(peers, 'PEER').toUpperCase(),
    ];

    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 14, 20, 0),
      child: SurfaceCard(
        padding: const EdgeInsets.all(16),
        // Mint only when something is genuinely connected; otherwise this is
        // a neutral status panel, not a reassurance.
        borderColor: peers > 0
            ? AppColors.signal.withValues(alpha: 0.24)
            : AppColors.border,
        gradient: peers > 0
            ? LinearGradient(
                begin: Alignment.topLeft,
                end: Alignment.bottomRight,
                colors: [
                  AppColors.signal.withValues(alpha: 0.13),
                  AppColors.signal.withValues(alpha: 0.03),
                ],
              )
            : null,
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    peers > 0 ? 'Connected' : 'Idle',
                    style: const TextStyle(
                        fontSize: 15, fontWeight: FontWeight.w600),
                  ),
                  const SizedBox(height: 5),
                  Text(
                    facts.join(' · '),
                    style: monoLabel(
                      size: 10.5,
                      color: peers > 0
                          ? AppColors.signalMuted
                          : AppColors.textFaint,
                      letterSpacing: 0.4,
                    ),
                  ),
                ],
              ),
            ),
            Icon(
              peers > 0 ? Icons.check_circle_outline : Icons.circle_outlined,
              size: 20,
              color: peers > 0 ? AppColors.signal : AppColors.textGhost,
            ),
          ],
        ),
      ),
    );
  }
}
