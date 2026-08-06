import 'package:flutter/material.dart';

import '../../../theme.dart';
import 'welcome.dart';

/// The wide-layout empty state with the same logo share action as compact Home.
class EmptyCollectionsWelcome extends StatelessWidget {
  const EmptyCollectionsWelcome({super.key, required this.onShare});

  final VoidCallback onShare;

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              Welcome(titleSize: 34, onLogoTap: onShare),
              const SizedBox(height: 26),
              Text(
                'NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED',
                textAlign: TextAlign.center,
                style: monoLabel(size: 10.5, color: AppColors.textGhost),
              ),
            ],
          ),
        ),
      );
}
