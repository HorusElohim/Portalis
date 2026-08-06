import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection_import.dart';

/// Real-time local publication progress reported by the Rust backend.
class CollectionImportProgress extends StatelessWidget {
  const CollectionImportProgress({super.key, required this.ingestion});

  final CollectionImport ingestion;

  @override
  Widget build(BuildContext context) {
    final failed = ingestion.failed;
    final color = failed ? AppColors.danger : AppColors.signal;
    final progress = ingestion.progress.clamp(0.0, 1.0).toDouble();
    final percent = formatProgressPercent(progress);
    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(14),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.08),
        border: Border.all(color: color.withValues(alpha: 0.35)),
        borderRadius: BorderRadius.circular(AppRadius.inner),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Expanded(
                child: Text(
                  failed ? 'IMPORT FAILED' : _stageLabel,
                  style: monoLabel(
                    color: color,
                    weight: FontWeight.w700,
                    letterSpacing: 0.6,
                  ),
                ),
              ),
              if (!failed)
                Text(
                  percent,
                  style: monoLabel(color: color, weight: FontWeight.w700),
                ),
            ],
          ),
          const SizedBox(height: 9),
          LinearProgressIndicator(
            value: failed ? 0 : progress,
            minHeight: 4,
            borderRadius: BorderRadius.circular(AppRadius.pill),
            backgroundColor: AppColors.borderStrong,
            valueColor: AlwaysStoppedAnimation(color),
          ),
          const SizedBox(height: 7),
          Text(
            failed
                ? ingestion.error!
                : '${formatBytes(ingestion.processedBytes)} of '
                    '${formatBytes(ingestion.totalBytes)} processed by Rust',
            maxLines: 2,
            overflow: TextOverflow.ellipsis,
            style: AppText.caption(color: AppColors.textDim),
          ),
        ],
      ),
    );
  }

  String get _stageLabel => switch (ingestion.stage) {
        'preparing' => 'PREPARING SOURCES',
        'copying' => 'IMPORTING FILES',
        'hashing' => 'HASHING TORRENT PIECES',
        'seeding' => 'STARTING SEED',
        _ => ingestion.stage.toUpperCase(),
      };
}
