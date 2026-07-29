import 'package:flutter/material.dart';
import '../theme.dart';
import '../widgets/common.dart';

class SettingsScreen extends StatefulWidget {
  const SettingsScreen({super.key});

  @override
  State<SettingsScreen> createState() => _SettingsScreenState();
}

class _SettingsScreenState extends State<SettingsScreen> {
  final _rows = [
    ('Auto-seed on Wi-Fi only', 'Pause uploads on cellular', true),
    ('Background sharing', 'Keep seeding when app is closed', true),
    ('Discoverable to collaborators', 'Show online status', false),
    ('Metered connection warning', 'Ask before large downloads', false),
  ];

  @override
  Widget build(BuildContext context) {
    return SingleChildScrollView(
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          const Padding(
            padding: EdgeInsets.fromLTRB(20, 66, 20, 6),
            child: Text(
              'Settings',
              style: TextStyle(fontSize: 20, fontWeight: FontWeight.w500),
            ),
          ),
          Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20),
            child: Column(
              children: [
                for (var i = 0; i < _rows.length; i++)
                  _SettingsRow(
                    label: _rows[i].$1,
                    subtitle: _rows[i].$2,
                    value: _rows[i].$3,
                    onChanged: (v) => setState(() {
                      _rows[i] = (_rows[i].$1, _rows[i].$2, v);
                    }),
                    showDivider: i != _rows.length - 1,
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
                _MeterRow(label: 'Upload limit', fraction: 0.6, value: '2 MB/s'),
                const SizedBox(height: 10),
                _MeterRow(
                    label: 'Storage cap', fraction: 0.34, value: '3.4 / 10 GB'),
              ],
            ),
          ),
          const SizedBox(height: 24),
        ],
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
