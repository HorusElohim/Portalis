// Cross-feature control design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';

/// The big filled call-to-action at the foot of a flow. [ember] switches it
/// to the torrent accent — the one place a primary action is not mint,
/// because the thing it starts is a torrent, not a Portalis transfer.
class PrimaryAction extends StatelessWidget {
  const PrimaryAction({
    super.key,
    required this.label,
    required this.onTap,
    this.icon,
    this.trailingChevron = true,
    this.ember = false,
    this.expand = true,
  });

  final String label;
  final VoidCallback? onTap;
  final IconData? icon;
  final bool trailingChevron;
  final bool ember;

  /// Fills the width it is given, which is what a stacked action wants. Set
  /// false to size to the label instead — beside a field, rather than under
  /// one.
  final bool expand;

  @override
  Widget build(BuildContext context) {
    final enabled = onTap != null;
    final fill = ember ? AppColors.ember : AppColors.signal;
    final ink = ember ? AppColors.onEmber : AppColors.onSignal;
    return Opacity(
      // Disabled reads as dimmed rather than grey: the button keeps its
      // identity, it just isn't ready yet.
      opacity: enabled ? 1 : 0.38,
      child: Material(
        color: fill,
        borderRadius: BorderRadius.circular(AppRadius.card),
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.card),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 18),
            child: Row(
              mainAxisAlignment: MainAxisAlignment.center,
              mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
              children: [
                if (icon != null) ...[
                  Icon(icon, size: 20, color: ink),
                  const SizedBox(width: 10),
                ],
                Flexible(
                  child: Text(
                    label,
                    overflow: TextOverflow.ellipsis,
                    style: displayText(size: 16.5, color: ink),
                  ),
                ),
                if (trailingChevron) ...[
                  const SizedBox(width: 10),
                  Icon(Icons.chevron_right, size: 19, color: ink),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// The call-to-action that closes a flow — "Create & share", "Join", "Add &
/// start" — with the one line of consequence under it.
///
/// Goes in [AppScreen.footer], which supplies the rule above it and the
/// gutter around it. Share, Join and Add torrent each carried their own
/// copy of this button: the same 52pt filled rectangle, the same 14pt
/// radius, the same 20pt spinner swapped in while busy, written out three
/// times.
///
/// Distinct from [PrimaryAction], which is the pill that *starts* a flow
/// from a list or a sidebar. This one ends one, so it is full width and
/// square-shouldered rather than a pill.
class ScreenAction extends StatelessWidget {
  const ScreenAction({
    super.key,
    required this.label,
    required this.onPressed,
    this.hint,
    this.busy = false,
    this.buttonKey,
  });

  final String label;

  /// Null disables the action. [busy] disables it too, so a caller only has
  /// to express "not ready yet" here.
  final VoidCallback? onPressed;

  /// What happens when this is pressed, in one line.
  final String? hint;

  final bool busy;

  /// Sits on the button itself rather than on this widget, so a tap in a
  /// test lands on the button and not on the hint beneath it.
  final Key? buttonKey;

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        SizedBox(
          width: double.infinity,
          height: 52,
          child: FilledButton(
            key: buttonKey,
            onPressed: busy ? null : onPressed,
            style: FilledButton.styleFrom(
              backgroundColor: AppColors.signal,
              disabledBackgroundColor: AppColors.borderStrong,
              foregroundColor: AppColors.surfaceDeep,
              shape: RoundedRectangleBorder(
                borderRadius: BorderRadius.circular(AppRadius.control),
              ),
            ),
            child: busy
                ? const SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      valueColor: AlwaysStoppedAnimation(AppColors.surfaceDeep),
                    ),
                  )
                : Text(label, style: AppText.action()),
          ),
        ),
        if (hint != null) ...[
          const SizedBox(height: 8),
          Text(
            hint!,
            textAlign: TextAlign.center,
            style: AppText.caption(),
          ),
        ],
      ],
    );
  }
}

/// Outlined accent pill button, e.g. "＋ Share something".
class PillButton extends StatelessWidget {
  const PillButton({
    super.key,
    required this.label,
    required this.onTap,
    this.icon,
    this.filled = false,
    this.dim = false,
  });

  final String label;
  final VoidCallback? onTap;
  final Widget? icon;
  final bool filled;

  /// Use the dimmer neutral outline instead of the accent outline.
  final bool dim;

  @override
  Widget build(BuildContext context) {
    final color = dim ? AppColors.textDim : AppColors.signalSoft;
    final borderColor = dim ? AppColors.borderStrong : AppColors.signal;
    return Material(
      color: filled ? AppColors.signal : Colors.transparent,
      shape: StadiumBorder(
        side: BorderSide(color: filled ? AppColors.signal : borderColor),
      ),
      child: InkWell(
        customBorder: const StadiumBorder(),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[icon!, const SizedBox(width: 7)],
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: AppText.cardTitle(
                      color: filled ? AppColors.onSignal : color),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Horizontal segmented filter, e.g. All / Sharing / Receiving.
class FilterChips extends StatelessWidget {
  const FilterChips({
    super.key,
    required this.labels,
    required this.selected,
    required this.onSelected,
  });

  final List<String> labels;
  final int selected;
  final ValueChanged<int> onSelected;

  @override
  Widget build(BuildContext context) {
    // Scrollable rather than wrapping: the chips are a single axis of choice,
    // and a second row of them would read as a different control. Also keeps
    // them from overflowing a narrow window.
    return SingleChildScrollView(
      scrollDirection: Axis.horizontal,
      child: Row(
        children: [
          for (var i = 0; i < labels.length; i++) ...[
            if (i > 0) const SizedBox(width: 8),
            // The selected chip is white-on-dark, never mint: selection is not
            // movement, and mint has to keep meaning only one thing.
            Material(
              color: i == selected ? AppColors.text : Colors.transparent,
              shape: StadiumBorder(
                side: BorderSide(
                  color:
                      i == selected ? AppColors.text : AppColors.borderStrong,
                ),
              ),
              child: InkWell(
                customBorder: const StadiumBorder(),
                onTap: () => onSelected(i),
                child: Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 14, vertical: 8),
                  child: Text(
                    labels[i],
                    style: AppText.body(
                        color: i == selected
                            ? AppColors.surfaceDeep
                            : AppColors.textDim,
                        weight:
                            i == selected ? FontWeight.w600 : FontWeight.w400),
                  ),
                ),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

/// A back-chevron text button, e.g. "‹ Back".
class NavBackButton extends StatelessWidget {
  const NavBackButton({super.key, this.onTap});

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return TextButton(
      onPressed: onTap ?? () => Navigator.of(context).maybePop(),
      style: TextButton.styleFrom(
        foregroundColor: AppColors.signalSoft,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      ),
      child: Text(
        '‹ Back',
        style: AppText.cardTitle(),
      ),
    );
  }
}
