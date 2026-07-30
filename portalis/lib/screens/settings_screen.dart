import 'dart:async';

import 'package:flutter/material.dart';
import '../services/settings_service.dart';
import '../theme.dart';
import '../widgets/common.dart';

const _uploadLimitPresetsBps = [0, 256000, 512000, 1000000, 2000000, 5000000];

String _formatBps(int bps) {
  if (bps == 0) return 'Unlimited';
  if (bps < 1000000) return '${(bps / 1000).toStringAsFixed(0)} KB/s';
  return '${(bps / 1000000).toStringAsFixed(1)} MB/s';
}

String _formatBytes(int bytes) {
  const gb = 1000000000;
  const mb = 1000000;
  if (bytes >= gb) return '${(bytes / gb).toStringAsFixed(1)} GB';
  return '${(bytes / mb).toStringAsFixed(0)} MB';
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

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: PageBody(
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
                    // Only settings the Rust engine actually honours live here.
                    // The mockup's four toggles (auto-seed on Wi-Fi, background
                    // sharing, discoverable, metered warning) and the storage
                    // *cap* were removed rather than kept as switches that
                    // persisted a preference nothing ever read — the app has no
                    // enforcement for any of them yet.
                    Padding(
                      padding: const EdgeInsets.fromLTRB(20, 6, 20, 0),
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
                          const Padding(
                            padding: EdgeInsets.only(top: 6),
                            child: Text(
                              'Tap to cycle presets. Applied to the running '
                              'transfer engine immediately.',
                              style: TextStyle(
                                  fontSize: 10, color: AppColors.neutral500),
                            ),
                          ),
                          const SizedBox(height: 14),
                          // Reported by the engine (torrent.rs::storage_usage_bytes),
                          // not capped by anything — so it's shown as a plain
                          // figure rather than a meter against an invented limit.
                          Row(
                            children: [
                              const SizedBox(
                                width: 80,
                                child: Text(
                                  'Storage used',
                                  style: TextStyle(
                                      fontSize: 12, color: AppColors.neutral400),
                                ),
                              ),
                              Text(
                                _formatBytes(_settings.storageUsedBytes),
                                style: const TextStyle(
                                    fontSize: 12, fontFamily: 'monospace'),
                              ),
                            ],
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
