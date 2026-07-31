// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../theme.dart';
import 'primitives.dart';

/// A titled group of rows. Settings and both detail screens each had their
/// own private copy of this; they now share one so a section header can't
/// drift between screens.
class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.label,
    required this.children,
    this.padding = const EdgeInsets.fromLTRB(20, 18, 20, 0),
  });

  final String label;
  final List<Widget> children;
  final EdgeInsets padding;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: padding,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(label),
          const SizedBox(height: 6),
          ...children,
        ],
      ),
    );
  }
}

/// Label on the left, value on the right. Read-only unless [onTap] is given,
/// in which case it gains a chevron and behaves as a disclosure row.
class ValueRow extends StatelessWidget {
  const ValueRow({
    super.key,
    required this.label,
    required this.value,
    this.subtitle,
    this.onTap,
    this.monospace = true,
    this.copyable = false,
    this.valueColor,
  });

  final String label;
  final String value;
  final String? subtitle;
  final VoidCallback? onTap;
  final bool monospace;
  final bool copyable;

  /// Only pass a colour when it means something — see [AppColors.signal].
  final Color? valueColor;

  @override
  Widget build(BuildContext context) {
    final row = Container(
      padding: const EdgeInsets.symmetric(vertical: 10),
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.border)),
      ),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(label, style: const TextStyle(fontSize: 13.5)),
                if (subtitle != null) ...[
                  const SizedBox(height: 3),
                  Text(
                    subtitle!,
                    style: const TextStyle(
                        fontSize: 11, height: 1.4, color: AppColors.textFaint),
                  ),
                ],
              ],
            ),
          ),
          const SizedBox(width: 12),
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 160),
            child: Text(
              value,
              textAlign: TextAlign.right,
              overflow: TextOverflow.ellipsis,
              style: monospace
                  ? monoLabel(
                      size: 11.5,
                      color: valueColor ?? AppColors.signalSoft,
                      letterSpacing: 0,
                    )
                  : TextStyle(
                      fontSize: 12.5,
                      color: valueColor ?? AppColors.text,
                    ),
            ),
          ),
          if (copyable)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: value));
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('$label copied')),
                );
              },
              child: const Padding(
                padding: EdgeInsets.only(left: 6),
                child: Icon(Icons.copy, size: 13, color: AppColors.textDim),
              ),
            ),
          if (onTap != null)
            const Padding(
              padding: EdgeInsets.only(left: 4),
              child: Icon(Icons.chevron_right,
                  size: 15, color: AppColors.textGhost),
            ),
        ],
      ),
    );
    if (onTap == null) return row;
    return InkWell(onTap: onTap, child: row);
  }
}

/// A labelled on/off row.
class SwitchRow extends StatelessWidget {
  const SwitchRow({
    super.key,
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
                Text(label, style: const TextStyle(fontSize: 13.5)),
                const SizedBox(height: 3),
                Text(
                  subtitle,
                  style: const TextStyle(
                      fontSize: 11, height: 1.4, color: AppColors.textFaint),
                ),
              ],
            ),
          ),
          const SizedBox(width: 12),
          Switch(
            value: value,
            onChanged: onChanged,
            activeTrackColor: AppColors.signal,
            activeThumbColor: AppColors.onSignal,
            inactiveTrackColor: AppColors.borderStrong,
            inactiveThumbColor: AppColors.text,
          ),
        ],
      ),
    );
  }
}

/// A fixed-width-label detail row, for the two "details" screens.
class InfoRow extends StatelessWidget {
  const InfoRow({
    super.key,
    required this.label,
    required this.value,
    this.monospace = false,
    this.copyable = false,
  });

  final String label;
  final String value;
  final bool monospace;
  final bool copyable;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: const TextStyle(fontSize: 12, color: AppColors.textDim),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: monospace
                  ? monoLabel(
                      size: 12, color: AppColors.text, letterSpacing: 0)
                  : const TextStyle(fontSize: 12),
            ),
          ),
          if (copyable)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: value));
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('$label copied')),
                );
              },
              child: const Padding(
                padding: EdgeInsets.only(left: 6),
                child: Icon(Icons.copy, size: 13, color: AppColors.textDim),
              ),
            ),
        ],
      ),
    );
  }
}

/// An inline notice — an error, or a "this needs a restart" note.
class InfoBanner extends StatelessWidget {
  const InfoBanner({
    super.key,
    required this.color,
    required this.icon,
    required this.text,
  });

  final Color color;
  final IconData icon;
  final String text;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 6, 20, 0),
      child: Container(
        padding: const EdgeInsets.all(11),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.06),
          border: Border.all(color: color.withValues(alpha: 0.45)),
          borderRadius: BorderRadius.circular(10),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, size: 15, color: color),
            const SizedBox(width: 9),
            Expanded(
              child: Text(
                text,
                style: TextStyle(fontSize: 11, height: 1.45, color: color),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
