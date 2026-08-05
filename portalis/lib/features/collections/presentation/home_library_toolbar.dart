import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection_filter.dart';
import 'add_collection_action.dart';
import 'add_torrent_action.dart';
import 'collection_filter_action.dart';
import 'command_bar.dart';

/// The compact and wide entry controls for the collection library.
class HomeLibraryToolbar extends StatelessWidget {
  const HomeLibraryToolbar({
    super.key,
    required this.wide,
    required this.showFilters,
    required this.filter,
    required this.onSearch,
    required this.onFilterChanged,
    required this.onShare,
    required this.onJoin,
    required this.onAddTorrent,
  });

  final bool wide;
  final bool showFilters;
  final CollectionFilter filter;
  final ValueChanged<String> onSearch;
  final ValueChanged<CollectionFilter> onFilterChanged;
  final VoidCallback onShare;
  final ValueChanged<String> onJoin;
  final VoidCallback onAddTorrent;

  @override
  Widget build(BuildContext context) {
    final commandBar = PortalisCommandBar(onSearch: onSearch, onInvite: onJoin);
    if (wide) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(child: commandBar),
          const SizedBox(width: 12),
          PrimaryActionButton(
            key: const Key('shareSomethingButton'),
            label: 'New share',
            tone: ActionButtonTone.signal,
            icon: Icons.add,
            trailingChevron: false,
            expand: false,
            onTap: onShare,
          ),
          const SizedBox(width: 8),
          AddTorrentAction(onTap: onAddTorrent),
        ],
      );
    }

    return Row(
      children: [
        Expanded(child: commandBar),
        if (showFilters) ...[
          const SizedBox(width: 6),
          CollectionFilterAction(
            filter: filter,
            onChanged: onFilterChanged,
          ),
        ],
        const SizedBox(width: 6),
        AddCollectionAction(
          onTap: () => _showAddSheet(context),
        ),
      ],
    );
  }

  Future<void> _showAddSheet(BuildContext context) => showModalBottomSheet<void>(
        context: context,
        backgroundColor: AppColors.surface,
        builder: (sheetContext) => SafeArea(
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              ListTile(
                key: const Key('addShareAction'),
                leading: const Icon(Icons.add_photo_alternate_outlined),
                title: const Text('Share files'),
                subtitle: const Text('Create a collection from this device'),
                onTap: () {
                  Navigator.of(sheetContext).pop();
                  onShare();
                },
              ),
              ListTile(
                key: const Key('addJoinAction'),
                leading: const Icon(Icons.group_add_outlined),
                title: const Text('Join collection'),
                subtitle: const Text('Enter an invite code'),
                onTap: () {
                  Navigator.of(sheetContext).pop();
                  onJoin('');
                },
              ),
              ListTile(
                key: const Key('addTorrentAction'),
                leading: const Icon(Icons.download_outlined),
                title: const Text('Add torrent'),
                subtitle: const Text('Paste a magnet link or choose a file'),
                onTap: () {
                  Navigator.of(sheetContext).pop();
                  onAddTorrent();
                },
              ),
            ],
          ),
        ),
      );
}
