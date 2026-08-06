import 'package:flutter/material.dart';

import '../../../design/design.dart';

/// Lifecycle and destructive actions reserved for a collection command API.
enum CollectionCommand { restart, pause, forget, delete, deleteFiles }

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
        CollectionCommand.forget => 'Forget',
        CollectionCommand.delete => 'Delete',
        CollectionCommand.deleteFiles => 'Delete files',
      };

  IconData get icon => switch (this) {
        CollectionCommand.restart => Icons.restart_alt,
        CollectionCommand.pause => Icons.pause_outlined,
        CollectionCommand.forget => Icons.link_off,
        CollectionCommand.delete => Icons.delete_outline,
        CollectionCommand.deleteFiles => Icons.delete_sweep_outlined,
      };

  String get tooltip => switch (this) {
        CollectionCommand.restart => 'Restart transfer',
        CollectionCommand.pause => 'Pause transfer',
        // Matches the backend's own term for this (`forget_torrent`): removes
        // it from the active session — stops tracking and seeding it — but
        // never touches the bytes already on disk. "Stop" read as a milder
        // pause and hid that distinction.
        CollectionCommand.forget => 'Forget this torrent, keeping its downloaded files',
        CollectionCommand.delete => 'Remove collection from this device',
        CollectionCommand.deleteFiles => 'Delete downloaded files',
      };
}

/// One reusable command strip for both compact and wide collection previews.
/// Every command maps to a native collection lifecycle operation.
class CollectionCommandBar extends StatelessWidget {
  const CollectionCommandBar({
    super.key,
    required this.busy,
    required this.onCommand,
  });

  final bool busy;
  final ValueChanged<CollectionCommand> onCommand;

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
              tone: command == CollectionCommand.delete ||
                      command == CollectionCommand.deleteFiles
                  ? ActionButtonTone.ember
                  : ActionButtonTone.neutral,
              tooltip: command.tooltip,
              compact: true,
              onTap: busy ? null : () => onCommand(command),
            ),
        ],
      );
}
