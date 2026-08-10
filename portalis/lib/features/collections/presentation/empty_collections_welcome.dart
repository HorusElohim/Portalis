import 'package:flutter/material.dart';

import 'welcome.dart';

/// The wide-layout empty state with the same logo share action as compact Home.
class EmptyCollectionsWelcome extends StatelessWidget {
  const EmptyCollectionsWelcome({
    super.key,
    required this.onShare,
    required this.welcomeCycle,
  });

  final VoidCallback onShare;
  final int welcomeCycle;

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40),
          child: Welcome(
            titleSize: 34,
            onLogoTap: onShare,
            animationCycle: welcomeCycle,
            footer: 'NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED',
          ),
        ),
      );
}
