import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';

/// Lifecycle and destructive actions reserved for a collection command API.
enum CollectionCommand { restart, pause, delete }

/// How much of a collection's detail a row shows inline. Repeated taps step
/// through all three: [collapsed] (one summary line) to [mid] (transfer
/// graph and actions) to [full] (peers and files too), then back around —
/// growing a row into its own detail instead of pushing a second screen.
enum CollectionDetailLevel {
  collapsed,
  mid,
  full;

  CollectionDetailLevel get next => switch (this) {
        CollectionDetailLevel.collapsed => CollectionDetailLevel.mid,
        CollectionDetailLevel.mid => CollectionDetailLevel.full,
        CollectionDetailLevel.full => CollectionDetailLevel.collapsed,
      };
}

extension CollectionCommandPresentation on CollectionCommand {
  String get label => switch (this) {
        CollectionCommand.restart => 'Restart',
        CollectionCommand.pause => 'Pause',
        CollectionCommand.delete => 'Delete',
      };

  IconData get icon => switch (this) {
        CollectionCommand.restart => Icons.restart_alt,
        CollectionCommand.pause => Icons.pause_outlined,
        CollectionCommand.delete => Icons.delete_outline,
      };

  String get tooltip => switch (this) {
        CollectionCommand.restart => 'Restart transfer',
        CollectionCommand.pause => 'Pause transfer',
        CollectionCommand.delete => 'Delete collection',
      };
}

/// One reusable command strip for both compact and wide collection previews.
/// Every command maps to a native collection lifecycle operation.
class CollectionCommandBar extends StatelessWidget {
  const CollectionCommandBar({
    super.key,
    required this.busy,
    required this.onCommand,
    this.trailingActions = const [],
  });

  final bool busy;
  final ValueChanged<CollectionCommand> onCommand;
  final List<Widget> trailingActions;

  @override
  Widget build(BuildContext context) => Wrap(
        spacing: 10,
        runSpacing: 10,
        children: [
          for (final command in CollectionCommand.values)
            OutlineActionButton(
              key: Key('collectionCommand${command.name}'),
              label: command.label,
              icon: command.icon,
              tone: command == CollectionCommand.delete
                  ? ActionButtonTone.ember
                  : ActionButtonTone.neutral,
              tooltip: command.tooltip,
              compact: true,
              onTap: busy ? null : () => onCommand(command),
            ),
          ...trailingActions,
        ],
      );
}

/// A low-profile action dock for the expanded collection header.
///
/// Lifecycle commands are icon-only because their tooltips carry the longer
/// explanation and the header is already dense with live data. Collection
/// growth actions keep short labels because "Invite", "Add media", and
/// "Fetch" describe different workflows that an icon alone would obscure.
class CollectionActionDock extends StatelessWidget {
  const CollectionActionDock({
    super.key,
    required this.busy,
    required this.onCommand,
    this.onInvite,
    this.onAddMedia,
    this.onFetch,
    this.pendingMedia = 0,
  });

  final bool busy;
  final ValueChanged<CollectionCommand> onCommand;
  final VoidCallback? onInvite;
  final VoidCallback? onAddMedia;
  final VoidCallback? onFetch;
  final int pendingMedia;

  @override
  Widget build(BuildContext context) => Wrap(
        spacing: 6,
        runSpacing: 6,
        crossAxisAlignment: WrapCrossAlignment.center,
        children: [
          for (final command in CollectionCommand.values) ...[
            if (command == CollectionCommand.delete) const _DockDivider(),
            _DockIconButton(
              key: Key('collectionCommand${command.name}'),
              icon: command.icon,
              label: command.label,
              tooltip: command.tooltip,
              destructive: command == CollectionCommand.delete,
              onTap: busy ? null : () => onCommand(command),
            ),
          ],
          if (onInvite != null) ...[
            const _DockDivider(),
            _DockLabelButton(
              key: const Key('collectionInvite'),
              icon: Icons.people_alt_outlined,
              label: 'Invite',
              accent: true,
              onTap: busy ? null : onInvite,
            ),
          ],
          if (onAddMedia != null)
            _DockLabelButton(
              key: const Key('collectionAddMedia'),
              icon: Icons.add_photo_alternate_outlined,
              label: 'Add media',
              onTap: busy ? null : onAddMedia,
            ),
          if (pendingMedia > 0 && onFetch != null)
            _DockLabelButton(
              key: const Key('collectionFetch'),
              icon: Icons.download_outlined,
              label: 'Fetch $pendingMedia',
              accent: true,
              onTap: busy ? null : onFetch,
            ),
        ],
      );
}

class _DockIconButton extends StatelessWidget {
  const _DockIconButton({
    super.key,
    required this.icon,
    required this.label,
    required this.tooltip,
    required this.onTap,
    this.destructive = false,
  });

  final IconData icon;
  final String label;
  final String tooltip;
  final VoidCallback? onTap;
  final bool destructive;

  @override
  Widget build(BuildContext context) {
    final color = destructive ? AppColors.danger : AppColors.textDim;
    return Tooltip(
      message: tooltip,
      child: Semantics(
        button: true,
        label: label,
        enabled: onTap != null,
        child: Opacity(
          opacity: onTap == null ? 0.38 : 1,
          child: Material(
            color: destructive
                ? AppColors.danger.withValues(alpha: 0.07)
                : AppColors.surfaceRaised.withValues(alpha: 0.58),
            borderRadius: BorderRadius.circular(AppRadius.tight),
            child: InkWell(
              onTap: onTap,
              borderRadius: BorderRadius.circular(AppRadius.tight),
              child: SizedBox.square(
                dimension: 34,
                child: ExcludeSemantics(
                  child: Icon(icon, size: 16, color: color),
                ),
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DockLabelButton extends StatelessWidget {
  const _DockLabelButton({
    super.key,
    required this.icon,
    required this.label,
    required this.onTap,
    this.accent = false,
  });

  final IconData icon;
  final String label;
  final VoidCallback? onTap;
  final bool accent;

  @override
  Widget build(BuildContext context) {
    final color = accent ? AppColors.signalSoft : AppColors.textDim;
    return Opacity(
      opacity: onTap == null ? 0.38 : 1,
      child: Material(
        color: accent
            ? AppColors.signalWash
            : AppColors.surfaceRaised.withValues(alpha: 0.58),
        borderRadius: BorderRadius.circular(AppRadius.tight),
        child: InkWell(
          onTap: onTap,
          borderRadius: BorderRadius.circular(AppRadius.tight),
          child: SizedBox(
            height: 34,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(icon, size: 15, color: color),
                  const SizedBox(width: 6),
                  Text(
                    label,
                    style: monoLabel(
                      size: 10,
                      color: color,
                      weight: FontWeight.w700,
                      letterSpacing: 0,
                    ),
                  ),
                ],
              ),
            ),
          ),
        ),
      ),
    );
  }
}

class _DockDivider extends StatelessWidget {
  const _DockDivider();

  @override
  Widget build(BuildContext context) => Container(
        width: 1,
        height: 18,
        margin: const EdgeInsets.symmetric(horizontal: 2),
        color: AppColors.borderStrong,
      );
}
