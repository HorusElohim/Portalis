import 'package:flutter/material.dart';

import '../theme.dart';
import 'formatters.dart';
import 'transfer_graph.dart';

/// The primary transfer summary for a collection preview.
///
/// It gives the percentage, byte totals, transfer history, peers, and ETA one
/// visual priority so the user does not have to scan several small labels.
class TransferPanel extends StatelessWidget {
  TransferPanel({
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
    Color? color,
    this.pendingLabel,
    this.leading,
    this.status,
    this.actions,
  }) : color = color ?? AppColors.signal;

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
  final Widget? leading;
  final Widget? status;
  final Widget? actions;

  @override
  Widget build(BuildContext context) {
    final moving = downloadMbps > 0 || uploadMbps > 0;
    final hasTotal = totalBytes > 0;
    final metrics = <Widget>[
      if (livePeers > 0)
        _PanelMetric(label: 'PEERS', value: livePeers.toString()),
      if (etaLabel != null)
        _PanelMetric(
          label: 'REMAINING',
          value: etaLabel!.replaceFirst(' left', ''),
        ),
    ];
    final graphHeader = TransferGraphHeader(
      progress: progress,
      downloadMbps: downloadMbps,
      uploadMbps: uploadMbps,
      history: history,
      startedAt: startedAt,
      completedAt: completedAt,
      color: color,
    );

    return SizedBox(
      width: double.infinity,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          if (leading != null)
            _LoftHeader(
              identity: leading!,
              progress: _ProgressBlock(
                progress: progress,
                hasTotal: hasTotal,
                downloadedBytes: downloadedBytes,
                totalBytes: totalBytes,
                pendingLabel: pendingLabel,
                color: color,
                metrics: metrics,
              ),
              transfer: moving || history.isNotEmpty ? graphHeader : null,
              actions: actions,
              status: status,
            )
          else
            _StandaloneSummary(
              progress: progress,
              hasTotal: hasTotal,
              downloadedBytes: downloadedBytes,
              totalBytes: totalBytes,
              pendingLabel: pendingLabel,
              color: color,
              metrics: metrics,
            ),
          if (moving || history.isNotEmpty) ...[
            const SizedBox(height: 14),
            TransferGraph(
              progress: progress,
              downloadMbps: downloadMbps,
              uploadMbps: uploadMbps,
              history: history,
              startedAt: startedAt,
              completedAt: completedAt,
              color: color,
              showHeader: leading == null,
            ),
          ],
          if (hasTotal && progress < 1) ...[
            const SizedBox(height: 12),
            ClipRRect(
              key: const Key('transferProgressBar'),
              borderRadius: BorderRadius.circular(AppRadius.pill),
              child: LinearProgressIndicator(
                value: progress.clamp(0.0, 1.0),
                minHeight: 4,
                backgroundColor: AppColors.borderStrong,
                valueColor: AlwaysStoppedAnimation(color),
              ),
            ),
          ],
        ],
      ),
    );
  }
}

class _LoftHeader extends StatelessWidget {
  const _LoftHeader({
    required this.identity,
    required this.progress,
    this.transfer,
    this.actions,
    this.status,
  });

  final Widget identity;
  final Widget progress;
  final Widget? transfer;
  final Widget? actions;
  final Widget? status;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
        builder: (context, constraints) {
          // The full shared-collection dock needs roughly 400 logical pixels.
          // Below this width, wrapping whole information groups is calmer than
          // forcing the dock itself back into the button grid it replaced.
          if (constraints.maxWidth >= 1380) {
            return Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                SizedBox(width: 280, child: identity),
                const SizedBox(width: 28),
                SizedBox(width: 210, child: progress),
                if (transfer != null) ...[
                  const SizedBox(width: 28),
                  SizedBox(width: 270, child: transfer),
                ],
                if (actions != null) ...[
                  const SizedBox(width: 28),
                  Expanded(
                    child: Align(
                      alignment: Alignment.centerRight,
                      child: actions,
                    ),
                  ),
                ],
                if (status != null) ...[
                  const SizedBox(width: 18),
                  status!,
                ],
              ],
            );
          }

          return Wrap(
            spacing: 28,
            runSpacing: 16,
            crossAxisAlignment: WrapCrossAlignment.center,
            children: [
              SizedBox(width: 280, child: identity),
              SizedBox(width: 210, child: progress),
              if (transfer != null) SizedBox(width: 270, child: transfer),
              if (actions != null)
                ConstrainedBox(
                  constraints: BoxConstraints(maxWidth: constraints.maxWidth),
                  child: actions,
                ),
              if (status != null) status!,
            ],
          );
        },
      );
}

class _StandaloneSummary extends StatelessWidget {
  const _StandaloneSummary({
    required this.progress,
    required this.hasTotal,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.color,
    required this.metrics,
    this.pendingLabel,
  });

  final double progress;
  final bool hasTotal;
  final int downloadedBytes;
  final int totalBytes;
  final String? pendingLabel;
  final Color color;
  final List<Widget> metrics;

  @override
  Widget build(BuildContext context) => LayoutBuilder(
        builder: (context, constraints) {
          final summary = _ProgressSummary(
            progress: progress,
            hasTotal: hasTotal,
            downloadedBytes: downloadedBytes,
            totalBytes: totalBytes,
            pendingLabel: pendingLabel,
            color: color,
          );
          if (constraints.maxWidth < 560) {
            return Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                summary,
                if (metrics.isNotEmpty) ...[
                  const SizedBox(height: 10),
                  Wrap(spacing: 22, runSpacing: 8, children: metrics),
                ],
              ],
            );
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.end,
            children: [
              Expanded(child: summary),
              if (metrics.isNotEmpty) ...[
                const SizedBox(width: 24),
                Row(
                  mainAxisSize: MainAxisSize.min,
                  crossAxisAlignment: CrossAxisAlignment.end,
                  children: [
                    for (var index = 0; index < metrics.length; index++) ...[
                      if (index > 0) const SizedBox(width: 24),
                      metrics[index],
                    ],
                  ],
                ),
              ],
            ],
          );
        },
      );
}

class _ProgressBlock extends StatelessWidget {
  const _ProgressBlock({
    required this.progress,
    required this.hasTotal,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.color,
    required this.metrics,
    this.pendingLabel,
  });

  final double progress;
  final bool hasTotal;
  final int downloadedBytes;
  final int totalBytes;
  final String? pendingLabel;
  final Color color;
  final List<Widget> metrics;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _ProgressSummary(
            progress: progress,
            hasTotal: hasTotal,
            downloadedBytes: downloadedBytes,
            totalBytes: totalBytes,
            pendingLabel: pendingLabel,
            color: color,
          ),
          if (metrics.isNotEmpty) ...[
            const SizedBox(height: 7),
            Wrap(spacing: 16, runSpacing: 6, children: metrics),
          ],
        ],
      );
}

class _ProgressSummary extends StatelessWidget {
  const _ProgressSummary({
    required this.progress,
    required this.hasTotal,
    required this.downloadedBytes,
    required this.totalBytes,
    required this.color,
    this.pendingLabel,
  });

  final double progress;
  final bool hasTotal;
  final int downloadedBytes;
  final int totalBytes;
  final String? pendingLabel;
  final Color color;

  @override
  Widget build(BuildContext context) => Row(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            hasTotal ? formatProgressPercent(progress) : '—',
            style: displayText(
              size: 30,
              color: color,
              weight: FontWeight.w700,
            ),
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
      );
}

class _PanelMetric extends StatelessWidget {
  const _PanelMetric({required this.label, required this.value});

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            label,
            style: monoLabel(size: 8, color: AppColors.textGhost),
          ),
          const SizedBox(height: 3),
          Text(
            value,
            style: monoLabel(
              size: 12,
              color: AppColors.text,
              weight: FontWeight.w700,
              letterSpacing: 0,
            ),
          ),
        ],
      );
}
