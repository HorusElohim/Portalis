import 'package:flutter/material.dart';

import '../../../design/design.dart';

/// Opens the compact-layout collection action sheet.
class AddCollectionAction extends StatelessWidget {
  const AddCollectionAction({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => FilledIconActionButton(
        key: const Key('addCollectionButton'),
        icon: Icons.add,
        tooltip: 'Add a collection',
        onTap: onTap,
      );
}
