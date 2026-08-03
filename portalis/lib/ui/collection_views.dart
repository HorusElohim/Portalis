// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../models.dart';
import '../theme.dart';
import 'indicators.dart';
import 'media.dart';
import 'primitives.dart';

/// One collection as a list row — shared by Home and the desktop centre pane
/// so a collection reads identically in both.
///
/// Colour is meaningful here: mint only while bytes are actually moving,
/// ember for torrent-sourced content, neutral for everything settled.
class CollectionRow extends StatelessWidget {
  const CollectionRow({
    super.key,
    required this.collection,
    required this.onTap,
    this.selected = false,
    this.detail,
  });

  final Collection collection;
  final VoidCallback onTap;
  final bool selected;

  /// Shown inside this card, under the row. Where a window is wide enough,
  /// opening a collection means this card growing to hold it — not a second
  /// panel beside the list describing the same thing twice.
  final Widget? detail;

  @override
  Widget build(BuildContext context) {
    final torrent = !collection.isShared;
    final live = collection.downloadMbps > 0 || collection.uploadMbps > 0;
    final accent = torrent ? AppColors.ember : AppColors.signal;

    return SurfaceCard(
      onTap: onTap,
      // A live row gets a tinted wash so it separates from the settled ones
      // at a glance; selection is a plain stronger border, not a colour.
      gradient: live
          ? LinearGradient(
              begin: Alignment.topLeft,
              end: Alignment.bottomRight,
              colors: [
                accent.withValues(alpha: 0.13),
                accent.withValues(alpha: 0.03),
              ],
            )
          : null,
      // Energy by what it is genuinely doing: transferring glows brightest,
      // shared-and-standing-by glows calmly, everything else not at all.
      glow: collection.glow,
      glowColor: accent,
      borderColor: selected && collection.glow == GlowLevel.none
          ? AppColors.borderStrong
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _row(),
          if (detail != null) ...[
            const Divider(height: 26, color: AppColors.border),
            detail!,
          ],
        ],
      ),
    );
  }

  Widget _row() {
    final torrent = !collection.isShared;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final live = collection.downloadMbps > 0 || collection.uploadMbps > 0;
    final downloading = collection.state == 'downloading';
    // No metadata has arrived, so there is no total to measure against — an
    // indeterminate bar is the honest shape for "reaching out to a peer".
    final connecting = collection.isConnecting;
    return Row(
      children: [
        SizedBox(
          width: 52,
          height: 52,
          child: torrent
              ? Container(
                  decoration: BoxDecoration(
                    color: AppColors.emberWash,
                    borderRadius: BorderRadius.circular(AppRadius.control),
                  ),
                  child: const Icon(Icons.download_outlined,
                      size: 20, color: AppColors.ember),
                )
              : ClipRRect(
                  borderRadius: BorderRadius.circular(AppRadius.control),
                  child: collection.media.isEmpty
                      ? const PlaceholderTile(borderRadius: 14)
                      : MediaThumbnail(
                          media: collection.media.first, borderRadius: 14),
                ),
        ),
        const SizedBox(width: 14),
        Expanded(
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  if (live) ...[
                    LiveDot(color: accent, size: 6),
                    const SizedBox(width: 7),
                  ],
                  Flexible(
                    child: Text(
                      collection.name,
                      overflow: TextOverflow.ellipsis,
                      style: displayText(size: 15),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 4),
              Text(
                collection.subtitle,
                overflow: TextOverflow.ellipsis,
                style: monoLabel(size: 11, letterSpacing: 0.2),
              ),
              if (downloading || connecting) ...[
                const SizedBox(height: 9),
                ClipRRect(
                  borderRadius: BorderRadius.circular(AppRadius.pill),
                  child: LinearProgressIndicator(
                    value:
                        connecting ? null : collection.progress.clamp(0.0, 1.0),
                    minHeight: 5,
                    backgroundColor: AppColors.borderStrong,
                    valueColor: AlwaysStoppedAnimation(accent),
                  ),
                ),
              ],
            ],
          ),
        ),
        const SizedBox(width: 12),
        if (downloading)
          StatusBadge(
            label: '${(collection.progress * 100).round()}%',
            color: accent,
          )
        else if (collection.isSharing)
          // Mint here is earned: this device is genuinely serving the
          // collection right now.
          StatusBadge(label: 'SHARING', color: accent)
        else
          StatusBadge(label: collection.state.toUpperCase()),
      ],
    );
  }
}

/// Shown when the backend itself failed, so it is never mistaken for an empty
/// list. The raw message is included deliberately — it is the only place a
/// Rust-side error reaches the user.
class CollectionsErrorState extends StatelessWidget {
  const CollectionsErrorState({super.key, required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.error_outline, size: 40, color: AppColors.danger),
            const SizedBox(height: 14),
            Text(
              'Couldn\'t load your collections.',
              textAlign: TextAlign.center,
              style: displayText(size: 17),
            ),
            const SizedBox(height: 8),
            Text(
              message,
              textAlign: TextAlign.center,
              style: monoLabel(
                  size: 10.5, color: AppColors.textDim, letterSpacing: 0.1),
            ),
          ],
        ),
      ),
    );
  }
}

/// "The engine is still starting, so nothing is live yet."
///
/// Its own widget because both Home and Collections have to say it: a
/// collection that exists on disk but has no torrent running is not being
/// shared, and silence there is what made a fresh launch look broken.
class EngineStartingNotice extends StatelessWidget {
  const EngineStartingNotice({super.key});

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(22, 16, 22, 0),
      child: SurfaceCard(
        padding: const EdgeInsets.symmetric(horizontal: 14, vertical: 11),
        child: Row(
          children: [
            const SizedBox(
              width: 13,
              height: 13,
              child: CircularProgressIndicator(
                strokeWidth: 1.8,
                valueColor: AlwaysStoppedAnimation(AppColors.textDim),
              ),
            ),
            const SizedBox(width: 11),
            Expanded(
              child: Text(
                'Starting the transfer engine — nothing is being shared yet.',
                style: AppText.secondary(color: AppColors.textDim, height: 1.4),
              ),
            ),
          ],
        ),
      ),
    );
  }
}
