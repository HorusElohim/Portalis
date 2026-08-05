import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/identity/domain/device_profile.dart';
import '../features/settings/domain/engine_settings.dart';
import '../features/settings/domain/listen_port_range.dart';
import '../features/settings/presentation/device_profile_section.dart';
import '../features/settings/presentation/settings_sections.dart';
import '../services/navigation.dart';
import '../theme.dart';
import 'settings/formats.dart';
import 'settings/storage.dart';

/// Your identity and this device's engine, in one place.
///
/// Absorbs what used to be the separate You/profile destination
/// (`user_screen.dart`) as a leading profile section: avatar, nickname,
/// session stats, sync address, and the "identity lives on this device"
/// note. Below that, every setting the BitTorrent engine honours, and
/// nothing else — each control maps to a real librqbit `SessionOptions`
/// field via `rust/backend/src/settings.rs`, no app-invented preferences
/// that persist a value nothing reads. Engine sections are ordered by how
/// likely they are to matter, with the two live-adjustable rate limits
/// first; everything below them is read once at session construction, which
/// the UI states rather than implying an immediate effect.
class SettingsScreen extends StatefulWidget {
  const SettingsScreen({
    super.key,
    this.embedded = false,
    this.advanced = false,
  });

  /// Rendered inside the desktop shell's centre pane rather than pushed —
  /// see [AppScreen]. Still shows a back button while showing Advanced,
  /// since collapsing that has nowhere else to happen; otherwise none,
  /// because the sidebar is the navigation.
  final bool embedded;

  /// The engine internals, reached from "Network & engine". Same state class
  /// and the same editors — only which sections are shown differs.
  final bool advanced;

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _settings = AppControllers.settings;
  Timer? _storagePoll;
  bool _restartPending = false;

  /// Which sections show. Starts from [SettingsScreen.advanced] but, when
  /// embedded, toggles in place rather than through a second pushed
  /// instance — see [_openAdvanced].
  late bool _advanced = widget.advanced;

  /// Embedded only: shows [StorageScreen] in place of Settings instead of
  /// pushing it over the shell's sidebar and list — see [_openStorage].
  bool _showStorage = false;

  /// Embedded only: shows [FormatsScreen] in place of Settings instead of
  /// pushing it over the shell's sidebar and list — see [_openFormats].
  bool _showFormats = false;

  String? _syncAddress;

  DeviceProfile? get _identity => AppControllers.identity.info;

  @override
  void initState() {
    super.initState();
    if (!_settings.loaded) _settings.load();
    // Storage grows as downloads finish in the background.
    _storagePoll = Timer.periodic(
      const Duration(seconds: 2),
      (_) => _settings.refreshStorageUsage(),
    );
    AppControllers.identity.load();
    _loadSyncAddress();
  }

  @override
  void dispose() {
    _storagePoll?.cancel();
    super.dispose();
  }

  Future<void> _loadSyncAddress() async {
    // Separate from identity: fetching the sync address is what starts the
    // listener, making this device reachable for as long as the app runs.
    try {
      final addr = await AppControllers.collections.syncAddress();
      if (mounted) setState(() => _syncAddress = addr);
    } catch (_) {
      // Backend unavailable — the row stays hidden rather than showing a
      // made-up address.
    }
  }

  Future<void> _rename() async {
    final result = await promptForText(
      context,
      title: 'Your name',
      initialValue: _identity?.nickname,
      helper: 'This is how you appear to collaborators.',
    );
    if (result == null || result.isEmpty || !mounted) return;
    try {
      await AppControllers.identity.rename(result);
    } catch (e) {
      if (mounted) showToast(context, 'Couldn\'t rename: $e');
    }
  }

  /// "Network & engine" used to always push a second [SettingsScreen] with
  /// `advanced: true` — a full-screen route over the desktop shell's sidebar
  /// and collection list even when this instance was already embedded in its
  /// centre pane. [openNestedScreen] toggles this instance's own sections in
  /// place instead when embedded; pushed (mobile), the drill-down keeps its
  /// own back-stack entry as before.
  void _openAdvanced() => openNestedScreen(
        context,
        embedded: widget.embedded,
        showInPlace: () => setState(() => _advanced = true),
        push: (_) => const SettingsScreen(advanced: true),
      );

  void _openStorage() => openNestedScreen(
        context,
        embedded: widget.embedded,
        showInPlace: () => setState(() => _showStorage = true),
        push: (_) => StorageScreen(embedded: widget.embedded),
      );

  void _openFormats() => openNestedScreen(
        context,
        embedded: widget.embedded,
        showInPlace: () => setState(() => _showFormats = true),
        push: (_) => const FormatsScreen(),
      );

  /// Collapses Advanced back to Basic in place when embedded — there is no
  /// pushed route here to pop. Not embedded, this instance only exists
  /// because it was pushed (either as Advanced, or from Home's search-bar
  /// join/torrent flow reaching Settings some other way), so it always
  /// pops.
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
      showToast(context, 'Couldn\'t save: $e', severity: ToastSeverity.error);
    }
  }

  Future<void> _pickDownloadFolder(EngineSettings settings) async {
    try {
      final folder = await FilePicker.getDirectoryPath();
      if (folder == null) return;

      final path = folder.trim();
      // Some Android document providers report the filesystem root for a
      // location the app cannot write to. Reject it here with a useful error
      // rather than failing only when a torrent starts.
      if (path.isEmpty || path == '/' || path == r'\') {
        if (mounted) {
          showToast(context, 'Choose a writable folder, not the device root.',
              severity: ToastSeverity.error);
        }
        return;
      }
      await _apply(settings.copyWith(downloadDir: path));
    } catch (e) {
      if (!mounted) return;
      showToast(context, 'Couldn\'t choose that folder: $e',
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
            style: monoLabel(size: 13, color: AppColors.text, letterSpacing: 0),
            decoration: InputDecoration(
              hintText: hint,
              hintStyle: const TextStyle(color: AppColors.textGhost),
              helperText: helper,
              helperMaxLines: 3,
              helperStyle: AppText.caption(color: AppColors.textDim),
            ),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () =>
                Navigator.of(dialogContext).pop(controller.text.trim()),
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
        content: Text(
          'Restores every value below to the built-in default. Your '
          'collections and identity are untouched.',
          style: AppText.secondary(color: AppColors.textDim),
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
      showToast(context, 'Couldn\'t reset: $e', severity: ToastSeverity.error);
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_showStorage) {
      return StorageScreen(
        embedded: widget.embedded,
        onBack: () => setState(() => _showStorage = false),
      );
    }
    if (_showFormats) {
      return FormatsScreen(
        embedded: widget.embedded,
        onBack: () => setState(() => _showFormats = false),
      );
    }
    return AppScreen(
      // Advanced is a screen in its own right, so it says so rather than
      // sitting under the title of the one it was reached from.
      title: _advanced ? 'Network & engine' : 'Settings',
      embedded: widget.embedded,
      forceShowBack: _advanced,
      onBack: _handleBack,
      // Wider than the shared reading cap once there's room: this is the one
      // screen with something to do with it (see SettingsSectionsLayout).
      wideMaxWidth: 1100,
      body: ListenableBuilder(
        listenable: Listenable.merge(
          [_settings, AppControllers.collections, AppControllers.identity],
        ),
        builder: (context, _) {
          final s = _settings.settings;
          return SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                if (!_advanced)
                  DeviceProfileSection(
                    profile: _identity,
                    identityError: AppControllers.identity.lastError,
                    collections: AppControllers.collections.collections,
                    syncAddress: _syncAddress,
                    onRename: _identity == null ? null : _rename,
                    onOpenPeople: () => AppNavigation.tab.value = 1,
                    onOpenFormats: _openFormats,
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
                  Padding(
                    padding: EdgeInsets.all(kScreenGutter),
                    child: Text(
                      'Loading engine settings…',
                      style: AppText.secondary(color: AppColors.textDim),
                    ),
                  )
                else
                  SettingsSectionsLayout(sections: _sections(s)),
                const SizedBox(height: 24),
              ],
            ),
          );
        },
      ),
    );
  }

  List<Widget> _sections(EngineSettings s) =>
      _advanced ? _advancedSections(s) : _basicSections(s);

  /// What most people ever need: how fast, and whether to keep sharing.
  List<Widget> _basicSections(EngineSettings s) {
    return [
      SettingsHealthCard(settings: s, collections: AppControllers.collections),
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
              await _apply(
                s.copyWith(uploadLimitBps: raw.isEmpty ? null : _parseInt(raw)),
              );
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
              await _apply(
                s.copyWith(
                  downloadLimitBps: raw.isEmpty ? null : _parseInt(raw),
                ),
              );
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
            onChanged: (v) => _apply(s.copyWith(persistSession: v)),
          ),
        ],
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(kScreenGutter, 18, kScreenGutter, 0),
        child: DestinationRow(
          icon: Icons.tune,
          title: 'Network & engine',
          subtitle: 'Ports, DHT, proxy, trackers, disk',
          onTap: _openAdvanced,
        ),
      ),
      SettingsSection(
        label: 'STORAGE · DOWNLOAD FOLDER NEEDS RESTART',
        children: [
          ValueRow(
            label: 'Download folder',
            value: s.downloadDir ?? 'Portalis default',
            subtitle: s.downloadDir == null
                ? 'Downloads/Portalis-TorrentDebug on desktop; Documents on mobile.'
                : 'New torrents use this folder after restarting Portalis.',
            onTap: () => _pickDownloadFolder(s),
          ),
          if (s.downloadDir != null)
            ValueRow(
              label: 'Use Portalis default folder',
              value: 'Reset',
              subtitle: 'Does not move torrents already added.',
              onTap: () => _apply(s.copyWith(downloadDir: null)),
            ),
          ValueRow(
            label: 'Storage used',
            value: formatBytes(_settings.storageUsedBytes),
            subtitle: 'Reported by the engine. Not capped by anything.',
            onTap: _openStorage,
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
              final range = parseListenPortRange(raw);
              if (range == null) {
                if (mounted) {
                  showToast(
                    context,
                    'Enter ports from 1 to 65535, for example 6881-6999.',
                    severity: ToastSeverity.error,
                  );
                }
                return;
              }
              await _apply(
                s.copyWith(
                  listenPortStart: range.start,
                  listenPortEnd: range.end,
                ),
              );
            },
          ),
          SwitchRow(
            label: 'UPnP port forwarding',
            subtitle: 'Ask the router to forward the listen port. No effect '
                'if the router has UPnP off, or while a VPN owns the default '
                'route.',
            value: s.enableUpnpPortForwarding,
            onChanged: (v) => _apply(s.copyWith(enableUpnpPortForwarding: v)),
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
              await _apply(
                s.copyWith(socksProxyUrl: raw.isEmpty ? null : raw),
              );
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
            onChanged: (v) => _apply(s.copyWith(disableDht: v)),
          ),
          SwitchRow(
            label: 'Disable DHT persistence',
            subtitle: 'Stop reusing the stored DHT identity and port between '
                'runs. That stored port is why two copies of Portalis can\'t '
                'run at once.',
            value: s.disableDhtPersistence,
            onChanged: (v) => _apply(s.copyWith(disableDhtPersistence: v)),
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
              await _apply(s.copyWith(trackers: trackers));
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
              await _apply(
                s.copyWith(blocklistUrl: raw.isEmpty ? null : raw),
              );
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
            onChanged: (v) => _apply(s.copyWith(fastresume: v)),
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
              await _apply(
                s.copyWith(
                  deferWritesUpToMb: raw.isEmpty ? null : _parseInt(raw),
                ),
              );
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
              await _apply(
                s.copyWith(
                  concurrentInitLimit: raw.isEmpty ? null : _parseInt(raw),
                ),
              );
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
              await _apply(
                s.copyWith(
                  peerConnectTimeoutSecs: raw.isEmpty ? null : _parseInt(raw),
                ),
              );
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
              await _apply(
                s.copyWith(
                  peerReadWriteTimeoutSecs:
                      raw.isEmpty ? null : _parseInt(raw),
                ),
              );
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
              await _apply(
                s.copyWith(
                  peerKeepAliveIntervalSecs:
                      raw.isEmpty ? null : _parseInt(raw),
                ),
              );
            },
          ),
        ],
      ),
      Padding(
        padding: const EdgeInsets.fromLTRB(kScreenGutter, 18, kScreenGutter, 0),
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

}
