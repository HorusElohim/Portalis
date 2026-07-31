import 'package:flutter/material.dart';

import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'collection_screen.dart';

/// Everything currently in flight.
///
/// This screen is *not* in the design doc — only the nav item is — so it's
/// derived from the model rather than invented: a collection appears here when
/// it has bytes left to fetch or is actively moving them. Nothing is shown
/// that the engine isn't reporting.
class TransfersScreen extends StatelessWidget {
  const TransfersScreen({super.key});

  /// "In flight" means the engine is doing work for it right now, or still
  /// owes work: unfinished bytes, or a live rate in either direction.
  /// Seeding-but-idle collections belong on Collections, not here.
  static bool isMoving(Collection c) =>
      c.downloadMbps > 0 ||
      c.uploadMbps > 0 ||
      (c.state == 'downloading') ||
      c.pendingMedia > 0;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) {
        final moving =
            Collections.instance.collections.where(isMoving).toList();
        final down = moving.fold<double>(0, (s, c) => s + c.downloadMbps);
        final up = moving.fold<double>(0, (s, c) => s + c.uploadMbps);

        return Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(22, 18, 22, 0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text('Transfers', style: displayText(size: 30, height: 1.1)),
                  const SizedBox(height: 5),
                  Text(
                    moving.isEmpty
                        ? 'Nothing moving right now'
                        : '${moving.length} in flight · '
                            '↓ ${down.toStringAsFixed(1)} · '
                            '↑ ${up.toStringAsFixed(1)} MB/s',
                    style: const TextStyle(
                        fontSize: 14, color: AppColors.textDim),
                  ),
                ],
              ),
            ),
            Expanded(
              child: moving.isEmpty
                  ? const _NothingMoving()
                  : ListView.separated(
                      padding: const EdgeInsets.fromLTRB(22, 20, 22, 22),
                      itemCount: moving.length,
                      separatorBuilder: (_, __) => const SizedBox(height: 10),
                      itemBuilder: (context, i) =>
                          _TransferRow(collection: moving[i]),
                    ),
            ),
          ],
        );
      },
    );
  }
}

class _NothingMoving extends StatelessWidget {
  const _NothingMoving();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 44),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            // Deliberately not mint: nothing is moving, so the signal colour
            // would be lying.
            const Icon(Icons.swap_horiz, size: 40, color: AppColors.textGhost),
            const SizedBox(height: 14),
            Text(
              'No transfers in flight.',
              textAlign: TextAlign.center,
              style: displayText(size: 17, color: AppColors.textDim),
            ),
            const SizedBox(height: 6),
            const Text(
              'Anything downloading or still to fetch shows up here.',
              textAlign: TextAlign.center,
              style: TextStyle(
                  fontSize: 13, height: 1.5, color: AppColors.textGhost),
            ),
          ],
        ),
      ),
    );
  }
}

class _TransferRow extends StatelessWidget {
  const _TransferRow({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    final torrent = !collection.isShared;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final live = collection.downloadMbps > 0 || collection.uploadMbps > 0;

    return SurfaceCard(
      padding: const EdgeInsets.all(16),
      onTap: () => Navigator.of(context).push(
        MaterialPageRoute(
          builder: (_) => CollectionScreen(collection: collection),
        ),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              // The pulsing dot is the live indicator; a queued-but-idle
              // transfer gets a static one so the motion means something.
              if (live)
                LiveDot(color: accent, size: 7)
              else
                Container(
                  width: 7,
                  height: 7,
                  decoration: const BoxDecoration(
                    color: AppColors.textGhost,
                    shape: BoxShape.circle,
                  ),
                ),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  collection.name,
                  overflow: TextOverflow.ellipsis,
                  style: displayText(size: 15.5),
                ),
              ),
              const Spacer(),
              if (live) ...[
                const SizedBox(width: 8),
                Text(
                  formatRate(collection.downloadMbps + collection.uploadMbps),
                  style: monoLabel(size: 11.5, color: accent, letterSpacing: 0),
                ),
              ],
            ],
          ),
          const SizedBox(height: 11),
          ClipRRect(
            borderRadius: BorderRadius.circular(99),
            child: LinearProgressIndicator(
              value: collection.progress.clamp(0.0, 1.0),
              minHeight: 6,
              backgroundColor: AppColors.borderStrong,
              valueColor: AlwaysStoppedAnimation(accent),
            ),
          ),
          const SizedBox(height: 8),
          Text(
            collection.pendingMedia > 0 && collection.totalBytes == 0
                // Nothing fetched yet means no metadata, so no total exists
                // to report — say that instead of "0 B of 0 B".
                ? '${plural(collection.pendingMedia, 'item')} to fetch'
                : '${formatBytes(collection.downloadedBytes)} / '
                    '${formatBytes(collection.totalBytes)} · '
                    '${collection.peersLabel}',
            style: monoLabel(size: 10.5, color: AppColors.textDim,
                letterSpacing: 0.2),
          ),
        ],
      ),
    );
  }
}
