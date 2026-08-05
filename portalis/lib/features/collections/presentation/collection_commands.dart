import 'package:flutter/material.dart';

import '../../../design/design.dart';

/// Lifecycle and destructive actions reserved for a collection command API.
enum CollectionCommand { restart, pause, stop, delete, deleteFiles }

extension CollectionCommandPresentation on CollectionCommand {
  String get label => switch (this) {
        CollectionCommand.restart => 'Restart',
        CollectionCommand.pause => 'Pause',
        CollectionCommand.stop => 'Stop',
        CollectionCommand.delete => 'Delete',
        CollectionCommand.deleteFiles => 'Delete files',
      };

  IconData get icon => switch (this) {
        CollectionCommand.restart => Icons.restart_alt,
        CollectionCommand.pause => Icons.pause_outlined,
        CollectionCommand.stop => Icons.stop_outlined,
        CollectionCommand.delete => Icons.delete_outline,
        CollectionCommand.deleteFiles => Icons.delete_sweep_outlined,
      };

  String get tooltip => switch (this) {
        CollectionCommand.restart => 'Restart transfer',
        CollectionCommand.pause => 'Pause transfer',
        CollectionCommand.stop => 'Stop transfer',
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
        spacing: 8,
        runSpacing: 8,
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
              onTap: busy ? null : () => onCommand(command),
            ),
        ],
      );
}
