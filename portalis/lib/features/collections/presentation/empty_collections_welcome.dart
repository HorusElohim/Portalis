import 'package:flutter/material.dart';

import '../../../theme.dart';
import 'welcome.dart';

/// The wide-layout empty state. Compact Home adds its contextual action.
class EmptyCollectionsWelcome extends StatelessWidget {
  const EmptyCollectionsWelcome({super.key});

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Welcome(titleSize: 34),
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
