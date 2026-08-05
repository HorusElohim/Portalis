import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../theme.dart';

/// A quiet rail that appears only while a collection is actively transferring.
class DesktopEventRail extends StatelessWidget {
  const DesktopEventRail({super.key});

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: AppControllers.collections,
        builder: (context, _) {
          final collections = AppControllers.collections.collections;
          final active = collections.where((collection) => collection.isMoving).toList();
          if (active.isEmpty) return const SizedBox.shrink();

          final down = active.fold<double>(
            0,
            (sum, collection) => sum + collection.downloadMbps,
          );
          final up = active.fold<double>(
            0,
            (sum, collection) => sum + collection.uploadMbps,
          );

          return Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              LiveDot(color: AppColors.signal, size: 7),
              const SizedBox(width: 8),
              Text(
                plural(active.length, 'active transfer').toUpperCase(),
                style: monoLabel(size: 10, color: AppColors.signal),
              ),
              if (down > 0) ...[
                const SizedBox(width: 12),
                Text('↓ ${formatRate(down)}', style: monoLabel(size: 10)),
              ],
              if (up > 0) ...[
                const SizedBox(width: 12),
                Text('↑ ${formatRate(up)}', style: monoLabel(size: 10)),
              ],
            ],
          );
        },
      );
}
