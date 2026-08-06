import 'package:flutter/material.dart';

import '../domain/collection_filter.dart';
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
    required this.onJoin,
  });

  final bool wide;
  final bool showFilters;
  final CollectionFilter filter;
  final ValueChanged<String> onSearch;
  final ValueChanged<CollectionFilter> onFilterChanged;
  final ValueChanged<String> onJoin;

  @override
  Widget build(BuildContext context) {
    final commandBar = PortalisCommandBar(
      onSearch: onSearch,
      onInvite: onJoin,
    );
    if (wide) {
      return commandBar;
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
      ],
    );
  }

}
