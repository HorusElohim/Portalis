import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart' show AppColors, AppRadius, monoLabel;

/// The single entry point for creating a collection from the library.
class ShareCollectionAction extends StatelessWidget {
  const ShareCollectionAction({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Center(
        child: Tooltip(
          message: 'Share files',
          child: Semantics(
            button: true,
            label: 'Share files',
            child: InkWell(
              key: const Key('shareCollectionAction'),
              borderRadius: BorderRadius.circular(AppRadius.card),
              onTap: onTap,
              child: Padding(
                padding: const EdgeInsets.all(10),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    const PortalisLogo(size: 48, energized: true),
                    const SizedBox(height: 7),
                    Text(
                      'Share files',
                      style: monoLabel(
                        size: 10,
                        color: AppColors.textDim,
                        letterSpacing: 0.6,
                      ),
                    ),
                  ],
                ),
              ),
            ),
          ),
        ),
      );
}
