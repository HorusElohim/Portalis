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
  });

  final Collection collection;
  final VoidCallback onTap;
  final bool selected;

  @override
  Widget build(BuildContext context) {
    final torrent = !collection.isShared;
    final live = collection.downloadMbps > 0 || collection.uploadMbps > 0;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final downloading = collection.state == 'downloading';

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
      borderColor:
          selected && collection.glow == GlowLevel.none
              ? AppColors.borderStrong
              : null,
      child: Row(
        children: [
          SizedBox(
            width: 52,
            height: 52,
            child: torrent
                ? Container(
                    decoration: BoxDecoration(
                      color: AppColors.emberWash,
                      borderRadius: BorderRadius.circular(14),
                    ),
                    child: const Icon(Icons.download_outlined,
                        size: 20, color: AppColors.ember),
                  )
                : ClipRRect(
                    borderRadius: BorderRadius.circular(14),
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
                if (downloading) ...[
                  const SizedBox(height: 9),
                  ClipRRect(
                    borderRadius: BorderRadius.circular(99),
                    child: LinearProgressIndicator(
                      value: collection.progress.clamp(0.0, 1.0),
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
      ),
    );
  }
}
