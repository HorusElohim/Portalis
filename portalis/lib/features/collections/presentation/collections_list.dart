import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../domain/collection.dart';
import 'collection_commands.dart';
import 'collection_views.dart';

/// The wide-layout collection list. The caller supplies optional detail so
/// this component does not depend on a navigation strategy.
class CollectionsList extends StatelessWidget {
  const CollectionsList({
    super.key,
    required this.collections,
    required this.openId,
    required this.onOpen,
    required this.detailFor,
    required this.onCommand,
  });

  final List<Collection> collections;
  final String? openId;
  final ValueChanged<Collection> onOpen;
  final Widget Function(Collection collection, CollectionDetailLevel level) detailFor;
  final ValueChanged<(Collection, CollectionCommand)> onCommand;

  @override
  Widget build(BuildContext context) => ListView.separated(
        padding: const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 28),
        itemCount: collections.length,
        separatorBuilder: (_, __) => const SizedBox(height: 10),
        itemBuilder: (context, index) {
          final collection = collections[index];
          final isOpen = collection.id == openId;
          return CollectionRow(
            collection: collection,
            selected: isOpen,
            onTap: () => onOpen(collection),
            detail: (level) => detailFor(collection, level),
            onCommand: (command) => onCommand((collection, command)),
          );
        },
      );
}
