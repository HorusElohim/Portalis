import 'dart:async';

import 'package:flutter/material.dart';

import '../services/settings_service.dart';
import '../theme.dart';
import '../widgets/common.dart';

String _formatBytes(int bytes) {
  const gb = 1000000000;
  const mb = 1000000;
  if (bytes >= gb) return '${(bytes / gb).toStringAsFixed(1)} GB';
  return '${(bytes / mb).toStringAsFixed(0)} MB';
}

String _formatBps(int? bps) {
  if (bps == null || bps == 0) return 'Unlimited';
  if (bps < 1000000) return '${(bps / 1000).toStringAsFixed(0)} KB/s';
  return '${(bps / 1000000).toStringAsFixed(1)} MB/s';
}

/// Every setting the BitTorrent engine honours, and nothing else.
///
/// Each control maps to a real librqbit `SessionOptions` field via
/// `rust/backend/src/settings.rs` — no app-invented preferences that persist
/// a value nothing reads. Sections are ordered by how likely they are to
/// matter, with the two live-adjustable rate limits first; everything below
/// them is read once at session construction, which the UI states rather than
/// implying an immediate effect.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _settings = SettingsService.instance;
  Timer? _storagePoll;
  bool _restartPending = false;

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

  Future<void> _apply(EngineSettings next) async {
    try {
      final restartRequired = await _settings.save(next);
      if (restartRequired && mounted) setState(() => _restartPending = true);
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Couldn\'t save: $e')),
      );
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
              hintStyle: const TextStyle(color: AppColors.neutral500),
              helperText: helper,
              helperMaxLines: 3,
              helperStyle:
                  const TextStyle(fontSize: 10.5, color: AppColors.neutral400),
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
          style: TextStyle(fontSize: 12, color: AppColors.neutral400),
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
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Couldn\'t reset: $e')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
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
                    Align(
                      alignment: Alignment.centerLeft,
                      child:
                          NavBackButton(onTap: () => Navigator.of(context).pop()),
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
                      _Banner(
                        color: const Color(0xFFEB5757),
                        icon: Icons.error_outline,
                        text: _settings.lastError!,
                      ),
                    if (_restartPending)
                      const _Banner(
                        color: AppColors.accent300,
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
                              fontSize: 12, color: AppColors.neutral400),
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

  List<Widget> _sections(EngineSettings s) {
    return [
      _Section(
        label: 'TRANSFER LIMITS · APPLIES IMMEDIATELY',
        children: [
          _ValueRow(
            label: 'Upload limit',
            value: _formatBps(s.uploadLimitBps),
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
          _ValueRow(
            label: 'Download limit',
            value: _formatBps(s.downloadLimitBps),
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
      _Section(
        label: 'NETWORK · NEEDS RESTART',
        children: [
          _ValueRow(
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
          _SwitchRow(
            label: 'UPnP port forwarding',
            subtitle: 'Ask the router to forward the listen port. No effect '
                'if the router has UPnP off, or while a VPN owns the default '
                'route.',
            value: s.enableUpnpPortForwarding,
            onChanged: (v) => _apply(_copy(s, enableUpnpPortForwarding: v)),
          ),
          _ValueRow(
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
      _Section(
        label: 'PEER DISCOVERY · NEEDS RESTART',
        children: [
          _SwitchRow(
            label: 'Disable DHT',
            subtitle: 'Without the distributed hash table, peers are only '
                'found via trackers or addresses shared directly.',
            value: s.disableDht,
            onChanged: (v) => _apply(_copy(s, disableDht: v)),
          ),
          _SwitchRow(
            label: 'Disable DHT persistence',
            subtitle: 'Stop reusing the stored DHT identity and port between '
                'runs. That stored port is why two copies of Portalis can\'t '
                'run at once.',
            value: s.disableDhtPersistence,
            onChanged: (v) => _apply(_copy(s, disableDhtPersistence: v)),
          ),
          _ValueRow(
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
          _ValueRow(
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
      _Section(
        label: 'SESSION · NEEDS RESTART',
        children: [
          _SwitchRow(
            label: 'Remember torrents across restarts',
            subtitle: 'Off means the engine starts empty each launch and '
                'silently stops seeding everything you have shared.',
            value: s.persistSession,
            onChanged: (v) => _apply(_copy(s, persistSession: v)),
          ),
          _SwitchRow(
            label: 'Fast resume',
            subtitle: 'Trust the saved piece state instead of re-hashing every '
                'file at launch.',
            value: s.fastresume,
            onChanged: (v) => _apply(_copy(s, fastresume: v)),
          ),
        ],
      ),
      _Section(
        label: 'PERFORMANCE · NEEDS RESTART',
        children: [
          _ValueRow(
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
          _ValueRow(
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
      _Section(
        label: 'PEER TIMEOUTS · NEEDS RESTART',
        children: [
          _ValueRow(
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
          _ValueRow(
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
          _ValueRow(
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
      _Section(
        label: 'STORAGE',
        children: [
          _ValueRow(
            label: 'Storage used',
            value: _formatBytes(_settings.storageUsedBytes),
            subtitle: 'Reported by the engine. Not capped by anything.',
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

class _Banner extends StatelessWidget {
  const _Banner({required this.color, required this.icon, required this.text});

  final Color color;
  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 4, 20, 0),
      child: Container(
        padding: const EdgeInsets.all(10),
        decoration: BoxDecoration(
          border: Border.all(color: color.withValues(alpha: 0.5)),
          borderRadius: BorderRadius.circular(8),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, size: 15, color: color),
            const SizedBox(width: 9),
            Expanded(
              child: Text(
                text,
                style: TextStyle(fontSize: 11, height: 1.4, color: color),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.label, required this.children});

  final String label;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(label),
          const SizedBox(height: 4),
          ...children,
        ],
      ),
    );
  }
}

class _SwitchRow extends StatelessWidget {
  const _SwitchRow({
    required this.label,
    required this.subtitle,
    required this.value,
    required this.onChanged,
  });

  final String label;
  final String subtitle;
  final bool value;
  final ValueChanged<bool> onChanged;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 9),
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.border)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: const TextStyle(fontSize: 13)),
                const SizedBox(height: 2),
                Text(
                  subtitle,
                  style: const TextStyle(
                      fontSize: 10.5, height: 1.35, color: AppColors.neutral400),
                ),
              ],
            ),
          ),
          const SizedBox(width: 10),
          Switch(
            value: value,
            onChanged: onChanged,
            activeTrackColor: AppColors.accent,
            activeThumbColor: AppColors.text,
            inactiveTrackColor: AppColors.borderStrong,
            inactiveThumbColor: AppColors.text,
          ),
        ],
      ),
    );
  }
}

/// A label/value row; tappable when [onTap] is given, read-only otherwise.
class _ValueRow extends StatelessWidget {
  const _ValueRow({
    required this.label,
    required this.value,
    this.subtitle,
    this.onTap,
  });

  final String label;
  final String value;
  final String? subtitle;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    final row = Container(
      padding: const EdgeInsets.symmetric(vertical: 9),
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.border)),
      ),
      child: Row(
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: const TextStyle(fontSize: 13)),
                if (subtitle != null) ...[
                  const SizedBox(height: 2),
                  Text(
                    subtitle!,
                    style: const TextStyle(
                        fontSize: 10.5,
                        height: 1.35,
                        color: AppColors.neutral400),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 10),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 150),
            child: Text(
              value,
              textAlign: TextAlign.right,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(
                fontSize: 11.5,
                fontFamily: 'monospace',
                color: AppColors.accent300,
              ),
            ),
          ),
          if (onTap != null)
            const Padding(
              padding: EdgeInsets.only(left: 4),
              child: Icon(Icons.chevron_right,
                  size: 15, color: AppColors.neutral500),
            ),
        ],
      ),
    );
    if (onTap == null) return row;
    return InkWell(onTap: onTap, child: row);
  }
}
