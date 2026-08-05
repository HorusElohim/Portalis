// Cross-feature row design primitive.

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../theme.dart';
import 'primitives.dart';
import 'toast.dart';

/// A titled group of rows. Settings and both detail screens each had their
/// own private copy of this; they now share one so a section header can't
/// drift between screens.
class SettingsSection extends StatelessWidget {
  const SettingsSection({
    super.key,
    required this.label,
    required this.children,
    this.padding =
        const EdgeInsets.fromLTRB(kScreenGutter, 18, kScreenGutter, 0),
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

/// An icon, a title, an optional subtitle, and — when [onTap] is given — a
/// trailing chevron. The shape behind every "go here" row on You and
/// Settings: File formats, People, Settings, Network & engine, and (without
/// [onTap]) the plain identity notice — five copies of the same Row before
/// this existed.
class DestinationRow extends StatelessWidget {
  const DestinationRow({
    super.key,
    required this.icon,
    required this.title,
    this.subtitle,
    this.iconColor = AppColors.textDim,
    this.onTap,
  });

  final IconData icon;
  final String title;
  final String? subtitle;
  final Color iconColor;

  /// Omit for a plain notice: no chevron, and nothing to tap.
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      padding: const EdgeInsets.all(16),
      onTap: onTap,
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 19, color: iconColor),
          const SizedBox(width: 13),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(title, style: AppText.cardTitle()),
                if (subtitle != null) ...[
                  const SizedBox(height: 3),
                  Text(subtitle!, style: AppText.secondary()),
                ],
              ],
            ),
          ),
          if (onTap != null)
            const Icon(Icons.chevron_right,
                size: 16, color: AppColors.textGhost),
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
                Text(label, style: AppText.body()),
                if (subtitle != null) ...[
                  const SizedBox(height: 3),
                  Text(
                    subtitle!,
                    style: AppText.caption(
                        color: AppColors.textFaint, height: 1.4),
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
                  : AppText.secondary(color: valueColor ?? AppColors.text),
            ),
          ),
          if (copyable)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: value));
                showToast(context, '$label copied',
                    severity: ToastSeverity.success);
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
                Text(label, style: AppText.body()),
                const SizedBox(height: 3),
                Text(
                  subtitle,
                  style:
                      AppText.caption(color: AppColors.textFaint, height: 1.4),
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
              style: AppText.secondary(color: AppColors.textDim),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: monospace
                  ? monoLabel(size: 12, color: AppColors.text, letterSpacing: 0)
                  : AppText.secondary(),
            ),
          ),
          if (copyable)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: value));
                showToast(context, '$label copied',
                    severity: ToastSeverity.success);
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
      padding: const EdgeInsets.fromLTRB(kScreenGutter, 6, kScreenGutter, 0),
      child: Container(
        padding: const EdgeInsets.all(11),
        decoration: BoxDecoration(
          color: color.withValues(alpha: 0.06),
          border: Border.all(color: color.withValues(alpha: 0.45)),
          borderRadius: BorderRadius.circular(AppRadius.inner),
        ),
        child: Row(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Icon(icon, size: 15, color: color),
            const SizedBox(width: 9),
            Expanded(
              child: Text(
                text,
                style: AppText.caption(color: color, height: 1.45),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
