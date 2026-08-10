import 'package:flutter/material.dart';

import 'welcome.dart';

/// Empty compact library state with the Portalis mark as its share action.
class EmptyCollectionsCallToAction extends StatelessWidget {
  const EmptyCollectionsCallToAction({
    super.key,
    required this.onShare,
    required this.welcomeCycle,
  });

  final VoidCallback onShare;
  final int welcomeCycle;

  @override
  Widget build(BuildContext context) => Column(
        children: [
          Welcome(
            titleSize: 30,
            onLogoTap: onShare,
            animationCycle: welcomeCycle,
          ),
        ],
      );
}
