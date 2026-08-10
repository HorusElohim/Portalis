import 'package:flutter/material.dart';

import '../../../app/app_controllers.dart';
import '../../../design/design.dart';
import '../domain/collection.dart';
import '../../../theme.dart';

enum _CollectionDeletion { collectionOnly, withFiles }

/// Confirms whether local files should go too, then deletes the collection.
///
/// Split out of `collection.dart` so viewing a collection and destroying it
/// are two different files, not two responsibilities in one — the same
/// reasoning that already split `home.dart` from the screens it launches.
///
/// [setBusy] drives the caller's own busy indicator: this doesn't own a
/// spinner of its own, since [collection.dart]'s `_busy` flag already gates
/// every other action on the same screen.
Future<void> confirmAndDeleteCollection(
  BuildContext context,
  Collection collection, {
  required ValueChanged<bool> setBusy,
}) async {
  final choice = await showDialog<_CollectionDeletion>(
    context: context,
    builder: (dialogContext) => AlertDialog(
      backgroundColor: AppColors.surface,
      title: Text('Delete "${collection.name}"?'),
      content: Text(
        'The collection will be deleted from this device. Choose whether its '
        'downloaded files stay on disk or are deleted too. Other collaborators '
        'keep their own copies.',
        style: AppText.secondary(color: AppColors.textDim),
      ),
      actions: [
        TextButton(
          onPressed: () => Navigator.of(dialogContext).pop(),
          child: const Text('Cancel'),
        ),
        TextButton(
          key: const Key('deleteCollectionOnly'),
          onPressed: () => Navigator.of(dialogContext)
              .pop(_CollectionDeletion.collectionOnly),
          child: Text('Delete', style: TextStyle(color: AppColors.danger)),
        ),
        TextButton(
          key: const Key('deleteCollectionWithFiles'),
          onPressed: () =>
              Navigator.of(dialogContext).pop(_CollectionDeletion.withFiles),
          child: Text(
            'Delete with files',
            style: TextStyle(color: AppColors.danger),
          ),
        ),
      ],
    ),
  );
  if (choice == null || !context.mounted) return;
  // Not fire-and-forget: deleting genuinely fails (a torrent that isn't in
  // the session, a store write that can't land), and without this the
  // dialog would just close with nothing happening and no error shown.
  setBusy(true);
  try {
    switch (choice) {
      case _CollectionDeletion.collectionOnly:
        await AppControllers.collections.delete(collection.id);
      case _CollectionDeletion.withFiles:
        await AppControllers.collections.deleteWithFiles(collection.id);
    }
    // Embedded, the list beside us simply drops it and the selection moves
    // on; there is no route to leave.
    if (context.mounted && Navigator.of(context).canPop()) {
      Navigator.of(context).pop();
    }
  } catch (e) {
    if (!context.mounted) return;
    showToast(context, 'Couldn\'t delete this collection: $e');
    setBusy(false);
  }
}
