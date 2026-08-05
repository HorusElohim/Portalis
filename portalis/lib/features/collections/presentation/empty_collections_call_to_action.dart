import 'package:flutter/material.dart';

import '../../../design/design.dart';
import 'welcome.dart';

/// Empty compact library state with a single promoted action.
class EmptyCollectionsCallToAction extends StatelessWidget {
  const EmptyCollectionsCallToAction({super.key, required this.onShare});

  final VoidCallback onShare;

  @override
  Widget build(BuildContext context) => Column(
        children: [
          const Welcome(titleSize: 30),
          const SizedBox(height: 24),
          PrimaryActionButton(
            label: 'Share files',
            icon: Icons.add,
            trailingChevron: false,
            onTap: onShare,
          ),
        ],
      );
}
