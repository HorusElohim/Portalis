import 'package:flutter/material.dart';

import '../theme.dart';
import 'formatters.dart';
import 'transfer_graph.dart';

/// The primary transfer summary for a collection preview.
///
/// It gives the percentage, byte totals, transfer history, peers, and ETA one
/// visual priority so the user does not have to scan several small labels.
class TransferPanel extends StatelessWidget {
  const TransferPanel({
    super.key,
    required this.progress,
    required this.downloadedBytes,
    required this.totalBytes,
    this.downloadMbps = 0,
    this.uploadMbps = 0,
    this.livePeers = 0,
    this.etaLabel,
    this.history = const [],
    this.startedAt,
    this.completedAt,
    this.color = AppColors.signal,
    this.pendingLabel,
  });

  final double progress;
  final int downloadedBytes;
  final int totalBytes;
  final double downloadMbps;
  final double uploadMbps;
  final int livePeers;
  final String? etaLabel;
  final List<TransferPoint> history;
  final DateTime? startedAt;
  final DateTime? completedAt;
  final Color color;
  final String? pendingLabel;

  @override
  Widget build(BuildContext context) {
    final moving = downloadMbps > 0 || uploadMbps > 0;
    final hasTotal = totalBytes > 0;
    final facts = <String>[
      if (livePeers > 0) plural(livePeers, 'peer'),
      if (etaLabel != null) etaLabel!,
    ];

    return Container(
      width: double.infinity,
      padding: const EdgeInsets.all(16),
      decoration: BoxDecoration(
        color: color.withValues(alpha: 0.08),
        borderRadius: BorderRadius.circular(AppRadius.card),
        border: Border.all(color: color.withValues(alpha: 0.32)),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Text(
                hasTotal ? formatProgressPercent(progress) : '—',
                style: displayText(size: 30, color: color, weight: FontWeight.w700),
              ),
              const SizedBox(width: 14),
              Expanded(
                child: Padding(
                  padding: const EdgeInsets.only(bottom: 4),
                  child: Text(
                    hasTotal
                        ? '${formatBytes(downloadedBytes)} of ${formatBytes(totalBytes)}'
                        : (pendingLabel ?? 'Waiting for metadata'),
                    overflow: TextOverflow.ellipsis,
                    style: monoLabel(
                      size: 12,
                      color: AppColors.text,
                      weight: FontWeight.w700,
                      letterSpacing: 0.1,
                    ),
                  ),
                ),
              ),
            ],
          ),
          if (hasTotal || moving || history.isNotEmpty) ...[
            const SizedBox(height: 14),
            TransferGraph(
              progress: progress,
              downloadMbps: downloadMbps,
              uploadMbps: uploadMbps,
              history: history,
              startedAt: startedAt,
              completedAt: completedAt,
              color: color,
            ),
          ],
          if (facts.isNotEmpty) ...[
            SizedBox(height: moving ? 12 : 10),
            Text(
              facts.join('  ·  '),
              style: monoLabel(
                size: 12,
                color: moving ? color : AppColors.textDim,
                weight: FontWeight.w700,
                letterSpacing: 0.15,
              ),
            ),
          ],
        ],
      ),
    );
  }
}
