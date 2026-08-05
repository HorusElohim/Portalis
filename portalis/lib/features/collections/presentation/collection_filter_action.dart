import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection_filter.dart';

/// Compact filter entry point. The selected filter remains a library concern;
/// this widget only presents the same choices on a phone-sized screen.
class CollectionFilterAction extends StatelessWidget {
  const CollectionFilterAction({
    super.key,
    required this.filter,
    required this.onChanged,
  });

  final CollectionFilter filter;
  final ValueChanged<CollectionFilter> onChanged;

  @override
  Widget build(BuildContext context) => OutlinedIconActionButton(
        key: const Key('collectionFilterButton'),
        tone: filter == CollectionFilter.all
            ? ActionButtonTone.neutral
            : ActionButtonTone.signal,
        icon: filter == CollectionFilter.all ? Icons.tune_outlined : Icons.tune,
        tooltip: 'Filter collections',
        onTap: () => _showSheet(context),
      );

  Future<void> _showSheet(BuildContext context) => showModalBottomSheet<void>(
        context: context,
        backgroundColor: AppColors.surface,
        builder: (sheetContext) => SafeArea(
          child: RadioGroup<CollectionFilter>(
            groupValue: filter,
            onChanged: (value) {
              if (value == null) return;
              onChanged(value);
              Navigator.of(sheetContext).pop();
            },
            child: Column(
              mainAxisSize: MainAxisSize.min,
              children: [
                for (final option in CollectionFilter.values)
                  RadioListTile<CollectionFilter>(
                    value: option,
                    title: Text(option.label),
                  ),
              ],
            ),
          ),
        ),
      );
}
