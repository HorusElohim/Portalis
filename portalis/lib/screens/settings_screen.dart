import 'dart:async';

import 'package:flutter/material.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import '../widgets/common.dart';

const _uploadLimitPresetsBps = [0, 256000, 512000, 1000000, 2000000, 5000000];
const _storageCapPresetsBytes = [
  5000000000,
  10000000000,
  20000000000,
  50000000000,
];

String _formatBps(int bps) {
  if (bps == 0) return 'Unlimited';
  if (bps < 1000000) return '${(bps / 1000).toStringAsFixed(0)} KB/s';
  return '${(bps / 1000000).toStringAsFixed(1)} MB/s';
}

String _formatBytes(int bytes) {
  const gb = 1000000000;
  return '${(bytes / gb).toStringAsFixed(1)} GB';
}

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _settings = SettingsService.instance;
  Timer? _storagePoll;

  @override
  void initState() {
    super.initState();
    if (!_settings.loaded) _settings.load();
    // Storage usage changes as downloads finish in the background — refresh
    // the meter for as long as this screen stays open, same polling
    // approach as TorrentCollections uses for Home.
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

  void _cycleUploadLimit() {
    final i = _uploadLimitPresetsBps.indexOf(_settings.uploadLimitBps);
    final next = _uploadLimitPresetsBps[(i + 1) % _uploadLimitPresetsBps.length];
    _settings.setUploadLimitBps(next);
  }

  void _cycleStorageCap() {
    final i = _storageCapPresetsBytes.indexOf(_settings.storageCapBytes);
    final next = _storageCapPresetsBytes[(i + 1) % _storageCapPresetsBytes.length];
    _settings.setStorageCapBytes(next);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: ListenableBuilder(
          listenable: _settings,
          builder: (context, _) {
            return SingleChildScrollView(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Align(
                    alignment: Alignment.centerLeft,
                    child: NavBackButton(onTap: () => Navigator.of(context).pop()),
                  ),
                  const Padding(
                    padding: EdgeInsets.fromLTRB(20, 6, 20, 6),
                    child: Text(
                      'Settings',
                      style: TextStyle(fontSize: 20, fontWeight: FontWeight.w500),
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.symmetric(horizontal: 20),
                    child: Column(
                      children: [
                        _SettingsRow(
                          label: 'Auto-seed on Wi-Fi only',
                          subtitle: 'Pause uploads on cellular',
                          value: _settings.autoSeedWifiOnly,
                          onChanged: _settings.setAutoSeedWifiOnly,
                          showDivider: true,
                        ),
                        _SettingsRow(
                          label: 'Background sharing',
                          subtitle: 'Keep seeding when app is closed',
                          value: _settings.backgroundSharing,
                          onChanged: _settings.setBackgroundSharing,
                          showDivider: true,
                        ),
                        _SettingsRow(
                          label: 'Discoverable to collaborators',
                          subtitle: 'Show online status',
                          value: _settings.discoverable,
                          onChanged: _settings.setDiscoverable,
                          showDivider: true,
                        ),
                        _SettingsRow(
                          label: 'Metered connection warning',
                          subtitle: 'Ask before large downloads',
                          value: _settings.meteredWarning,
                          onChanged: _settings.setMeteredWarning,
                          showDivider: false,
                        ),
                      ],
                    ),
                  ),
                  Padding(
                    padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        const SectionLabel('BANDWIDTH & STORAGE'),
                        const SizedBox(height: 10),
                        GestureDetector(
                          onTap: _cycleUploadLimit,
                          child: _MeterRow(
                            label: 'Upload limit',
                            fraction: _settings.uploadLimitBps == 0
                                ? 1.0
                                : (_settings.uploadLimitBps /
                                        _uploadLimitPresetsBps.last)
                                    .clamp(0.0, 1.0),
                            value: _formatBps(_settings.uploadLimitBps),
                          ),
                        ),
                        const SizedBox(height: 10),
                        GestureDetector(
                          onTap: _cycleStorageCap,
                          child: _MeterRow(
                            label: 'Storage cap',
                            fraction: (_settings.storageUsedBytes /
                                    _settings.storageCapBytes)
                                .clamp(0.0, 1.0),
                            value:
                                '${_formatBytes(_settings.storageUsedBytes)} / ${_formatBytes(_settings.storageCapBytes)}',
                          ),
                        ),
                        const Padding(
                          padding: EdgeInsets.only(top: 6),
                          child: Text(
                            'Tap a meter to cycle presets.',
                            style: TextStyle(
                                fontSize: 10, color: AppColors.neutral500),
                          ),
                        ),
                      ],
                    ),
                  ),
                  const SizedBox(height: 24),
                ],
              ),
            );
          },
        ),
      ),
    );
  }
}

class _SettingsRow extends StatelessWidget {
  const _SettingsRow({
    required this.label,
    required this.subtitle,
    required this.value,
    required this.onChanged,
    required this.showDivider,
  });

  final String label;
  final String subtitle;
  final bool value;
  final ValueChanged<bool> onChanged;
  final bool showDivider;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 11),
      decoration: BoxDecoration(
        border: showDivider
            ? const Border(bottom: BorderSide(color: AppColors.border))
            : null,
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
                      fontSize: 10.5, color: AppColors.neutral400),
                ),
              ],
            ),
          ),
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

class _MeterRow extends StatelessWidget {
  const _MeterRow({
    required this.label,
    required this.fraction,
    required this.value,
  });

  final String label;
  final double fraction;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        SizedBox(
          width: 80,
          child: Text(
            label,
            style: const TextStyle(fontSize: 12, color: AppColors.neutral400),
          ),
        ),
        Expanded(
          child: ClipRRect(
            borderRadius: BorderRadius.circular(99),
            child: LinearProgressIndicator(
              value: fraction,
              minHeight: 4,
              backgroundColor: AppColors.borderStrong,
              valueColor: const AlwaysStoppedAnimation(AppColors.accent),
            ),
          ),
        ),
        const SizedBox(width: 10),
        Text(
          value,
          style: const TextStyle(fontSize: 10, fontFamily: 'monospace'),
        ),
      ],
    );
  }
}
