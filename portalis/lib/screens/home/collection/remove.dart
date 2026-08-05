import 'package:flutter/material.dart';

import '../../../models.dart';
import '../../../services/collections.dart';
import '../../../theme.dart';
import '../../../ui/ui.dart';

/// Confirms, then removes a collection from this device.
///
/// Split out of `collection.dart` so viewing a collection and destroying it
/// are two different files, not two responsibilities in one — the same
/// reasoning that already split `home.dart` from the screens it launches.
///
/// [setBusy] drives the caller's own busy indicator: this doesn't own a
/// spinner of its own, since [collection.dart]'s `_busy` flag already gates
/// every other action on the same screen.
Future<void> confirmAndRemoveCollection(
  BuildContext context,
  Collection collection, {
  required ValueChanged<bool> setBusy,
}) async {
  final confirmed = await showDialog<bool>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      backgroundColor: AppColors.surface,
      title: Text('Remove "${collection.name}"?'),
      content: Text(
        'This only removes it from this device. Downloaded files stay on '
        'disk, and other collaborators keep their own copies.',
        style: AppText.secondary(color: AppColors.textDim),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(false),
          child: const Text('Cancel'),
        ),
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(true),
          child: const Text('Remove'),
        ),
      ],
    ),
  );
  if (confirmed != true || !context.mounted) return;
  // Not fire-and-forget: deleting genuinely fails (a torrent that isn't in
  // the session, a store write that can't land), and without this the
  // dialog would just close with nothing happening and no error shown.
  setBusy(true);
  try {
    await Collections.instance.delete(collection.id);
    // Embedded, the list beside us simply drops it and the selection moves
    // on; there is no route to leave.
    if (context.mounted && Navigator.of(context).canPop()) {
      Navigator.of(context).pop();
    }
  } catch (e) {
    if (!context.mounted) return;
    showToast(context, 'Couldn\'t remove this collection: $e');
    setBusy(false);
  }
}
