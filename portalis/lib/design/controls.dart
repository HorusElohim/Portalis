// Cross-feature control design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';

/// The semantic accent applied to an [ActionButton].
enum ActionButtonTone { signal, ember, neutral }

/// Shared contract for Portalis action buttons.
///
/// Concrete subclasses own their geometry and content shape. This keeps
/// feature code explicit while centralising tone, tooltip, and disabled
/// behaviour in one reusable design primitive.
abstract class ActionButton extends StatelessWidget {
  const ActionButton({
    super.key,
    required this.onTap,
    this.tone = ActionButtonTone.signal,
    this.tooltip,
  });

  final ActionButtonTone tone;
  final String? tooltip;
  final VoidCallback? onTap;

  Color get _accent => switch (tone) {
        ActionButtonTone.signal => AppColors.signal,
        ActionButtonTone.ember => AppColors.ember,
        ActionButtonTone.neutral => AppColors.textDim,
      };

  Color get _foreground => switch (tone) {
        ActionButtonTone.signal => AppColors.onSignal,
        ActionButtonTone.ember => AppColors.onEmber,
        ActionButtonTone.neutral => AppColors.surfaceDeep,
      };

  Widget _surface({
    required BorderRadius radius,
    required Widget child,
    required bool filled,
    Border? border,
  }) {
    final enabled = onTap != null;
    final button = Opacity(
      opacity: enabled ? 1 : 0.38,
      child: Material(
        color: filled ? _accent : AppColors.surface,
        borderRadius: radius,
        child: InkWell(
          borderRadius: radius,
          onTap: onTap,
          child: Container(
            decoration: border == null
                ? null
                : BoxDecoration(borderRadius: radius, border: border),
            child: child,
          ),
        ),
      ),
    );
    return tooltip == null ? button : Tooltip(message: tooltip!, child: button);
  }

  Widget _labelContent({
    required String label,
    required IconData? icon,
    required bool expand,
    required bool trailingChevron,
    required bool compact,
    required Color color,
  }) {
    return Container(
      constraints: BoxConstraints(minHeight: compact ? 34 : 52),
      padding: EdgeInsets.symmetric(
        horizontal: compact ? 10 : 20,
        vertical: compact ? 7 : 14,
      ),
      child: Row(
        mainAxisAlignment: MainAxisAlignment.center,
        mainAxisSize: expand ? MainAxisSize.max : MainAxisSize.min,
        children: [
          if (icon != null) ...[
            Icon(icon, size: compact ? 15 : 20, color: color),
            SizedBox(width: compact ? 5 : 10),
          ],
          Flexible(
            child: Text(
              label,
              overflow: TextOverflow.ellipsis,
              style: displayText(size: compact ? 11.5 : 16.5, color: color),
            ),
          ),
          if (trailingChevron) ...[
            SizedBox(width: compact ? 5 : 10),
            Icon(Icons.chevron_right, size: compact ? 15 : 19, color: color),
          ],
        ],
      ),
    );
  }

  Widget _iconContent(Color color, IconData icon) => SizedBox(
        width: 46,
        height: 46,
        child: Icon(icon, size: 20, color: color),
      );
}

/// Shared implementation for text actions with optional leading/trailing
/// content.
abstract class LabelActionButton extends ActionButton {
  const LabelActionButton({
    super.key,
    required this.label,
    required super.onTap,
    super.tone,
    super.tooltip,
    this.icon,
    this.expand = false,
    this.trailingChevron = false,
    this.compact = false,
  });

  final String label;
  final IconData? icon;
  final bool expand;
  final bool trailingChevron;
  final bool compact;

  Widget _buildLabel({required bool filled}) => _surface(
        radius: BorderRadius.circular(AppRadius.card),
        filled: filled,
        border: filled ? null : Border.all(color: AppColors.border),
        child: _labelContent(
          label: label,
          icon: icon,
          expand: expand,
          trailingChevron: trailingChevron,
          compact: compact,
          color: filled ? _foreground : _accent,
        ),
      );
}

/// Filled text action used for the primary action in a surface.
class PrimaryActionButton extends LabelActionButton {
  const PrimaryActionButton({
    super.key,
    required super.label,
    required super.onTap,
    super.tone,
    super.tooltip,
    super.icon,
    super.expand,
    super.trailingChevron,
    super.compact,
  });

  @override
  Widget build(BuildContext context) => _buildLabel(filled: true);
}

/// Outlined text action used for secondary actions.
class OutlineActionButton extends LabelActionButton {
  const OutlineActionButton({
    super.key,
    required super.label,
    required super.onTap,
    super.tone,
    super.tooltip,
    super.icon,
    super.expand,
    super.trailingChevron,
    super.compact,
  });

  @override
  Widget build(BuildContext context) => _buildLabel(filled: false);
}

/// Shared icon-button implementation for compact toolbar actions.
abstract class IconActionButton extends ActionButton {
  const IconActionButton({
    super.key,
    required this.icon,
    required super.onTap,
    super.tone,
    super.tooltip,
  });

  final IconData icon;

  Widget _buildIcon({required bool filled}) => _surface(
        radius: BorderRadius.circular(AppRadius.control),
        filled: filled,
        border: filled ? null : Border.all(color: AppColors.border),
        child: _iconContent(filled ? _foreground : _accent, icon),
      );
}

/// Filled icon action, used for the compact primary add action.
class FilledIconActionButton extends IconActionButton {
  const FilledIconActionButton({
    super.key,
    required super.icon,
    required super.onTap,
    super.tone,
    super.tooltip,
  });

  @override
  Widget build(BuildContext context) => _buildIcon(filled: true);
}

/// Outlined icon action, used for compact secondary actions.
class OutlinedIconActionButton extends IconActionButton {
  const OutlinedIconActionButton({
    super.key,
    required super.icon,
    required super.onTap,
    super.tone,
    super.tooltip,
  });

  @override
  Widget build(BuildContext context) => _buildIcon(filled: false);
}

/// The call-to-action that closes a flow, with its one-line consequence.
///
/// It is deliberately distinct from a toolbar [ActionButton]: this one is
/// full-width and belongs in [AppScreen.footer].
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
