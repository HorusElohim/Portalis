import 'dart:math' as math;

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
    final activePieceProgress = _activePieceProgress(ingestion);
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
          if (!failed) ...[
            const SizedBox(height: 10),
            if (ingestion.stage == 'hashing' && ingestion.totalPieces > 0)
              Padding(
                padding: const EdgeInsets.only(bottom: 5),
                child: Text(
                  '${ingestion.completedPieces} / ${ingestion.totalPieces} PIECES VERIFIED',
                  style:
                      monoLabel(color: AppColors.textDim, letterSpacing: 0.45),
                ),
              ),
            SizedBox(
              height: 52,
              width: double.infinity,
              child: CustomPaint(
                painter: _PieceMatrixPainter(
                  completedPieces: ingestion.completedPieces,
                  totalPieces: ingestion.totalPieces,
                  activePieceProgress: activePieceProgress,
                  color: color,
                ),
              ),
            ),
          ],
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
        'linking' => 'LINKING SOURCES',
        'hashing' => 'HASHING TORRENT PIECES',
        'seeding' => 'STARTING SEED',
        _ => ingestion.stage.toUpperCase(),
      };

  double _activePieceProgress(CollectionImport ingestion) {
    const pieceLength = 2 * 1024 * 1024;
    if (ingestion.stage != 'hashing' ||
        ingestion.completedPieces >= ingestion.totalPieces ||
        ingestion.totalPieces == 0) {
      return 0;
    }
    final completedBytes = ingestion.completedPieces * pieceLength;
    final activePieceLength = math.min(
      pieceLength,
      ingestion.totalBytes - completedBytes,
    );
    if (activePieceLength <= 0) return 0;
    return ((ingestion.processedBytes - completedBytes) / activePieceLength)
        .clamp(0.0, 1.0)
        .toDouble();
  }
}

class _PieceMatrixPainter extends CustomPainter {
  const _PieceMatrixPainter({
    required this.completedPieces,
    required this.totalPieces,
    required this.activePieceProgress,
    required this.color,
  });

  final int completedPieces;
  final int totalPieces;
  final double activePieceProgress;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    const columns = 18;
    const rows = 5;
    const gap = 3.0;
    final tileWidth = (size.width - (columns - 1) * gap) / columns;
    final tileHeight = (size.height - (rows - 1) * gap) / rows;
    final background = Paint()..color = AppColors.borderStrong;
    final fill = Paint()..color = color;
    final activeFill = Paint()..color = color.withValues(alpha: 0.65);
    final tileCount = columns * rows;
    final visibleTiles = totalPieces.clamp(0, tileCount).toInt();
    final piecePosition = completedPieces + activePieceProgress;
    for (var index = 0; index < columns * rows; index++) {
      final column = index % columns;
      final row = index ~/ columns;
      final rect = RRect.fromRectAndRadius(
        Rect.fromLTWH(
          column * (tileWidth + gap),
          row * (tileHeight + gap),
          tileWidth,
          tileHeight,
        ),
        const Radius.circular(1.5),
      );
      canvas.drawRRect(rect, background);
      if (index >= visibleTiles || totalPieces == 0) continue;
      final pieceStart = index * totalPieces / visibleTiles;
      final pieceEnd = (index + 1) * totalPieces / visibleTiles;
      final amount = ((piecePosition - pieceStart) / (pieceEnd - pieceStart))
          .clamp(0.0, 1.0)
          .toDouble();
      if (amount > 0) {
        canvas.drawRRect(
          RRect.fromRectAndRadius(
            Rect.fromLTWH(
                rect.left, rect.top, rect.width * amount, rect.height),
            const Radius.circular(1.5),
          ),
          amount >= 1 ? fill : activeFill,
        );
      }
    }
  }

  @override
  bool shouldRepaint(covariant _PieceMatrixPainter oldDelegate) =>
      oldDelegate.completedPieces != completedPieces ||
      oldDelegate.totalPieces != totalPieces ||
      oldDelegate.activePieceProgress != activePieceProgress ||
      oldDelegate.color != color;
}
