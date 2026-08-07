// Collection presentation shared by compact and wide layouts.

import 'package:flutter/material.dart';

import '../../../theme.dart';
import '../../../design/formatters.dart';
import '../../media/presentation/media_thumbnail.dart';
import '../domain/collection.dart';
import 'collection_commands.dart';
import 'collection_presentation.dart';
import '../../../design/indicators.dart';
import '../../../design/primitives.dart';

/// One collection as a list row — shared by Home and the desktop centre pane
/// so a collection reads identically in both.
///
/// Colour is meaningful here: mint only while bytes are actually moving,
/// ember for torrent-sourced content, neutral for everything settled.
class CollectionRow extends StatefulWidget {
  const CollectionRow({
    super.key,
    required this.collection,
    required this.onTap,
    this.selected = false,
    this.detail,
    this.onCommand,
  });

  final Collection collection;
  final VoidCallback onTap;
  final bool selected;

  /// Built for whichever [CollectionDetailLevel] this row has cycled to, and
  /// shown under the row from [CollectionDetailLevel.mid] on. Where a window
  /// is wide enough, opening a collection means this card growing to hold
  /// it — not a second panel beside the list describing the same thing
  /// twice. `null` where there is nowhere to grow into (the compact list,
  /// which pushes a separate screen instead) — the row then keeps its older,
  /// simpler behaviour: a plain tap, and the command bar always showing.
  final Widget Function(CollectionDetailLevel level)? detail;
  final ValueChanged<CollectionCommand>? onCommand;

  @override
  State<CollectionRow> createState() => _CollectionRowState();
}

class _CollectionRowState extends State<CollectionRow> {
  CollectionDetailLevel _level = CollectionDetailLevel.collapsed;

  @override
  void didUpdateWidget(CollectionRow oldWidget) {
    super.didUpdateWidget(oldWidget);
    // The list keeps at most one collection open at a time; when this row
    // is no longer the open one (another was tapped, or this one was), its
    // own notion of how far it had grown is stale.
    if (!widget.selected) _level = CollectionDetailLevel.collapsed;
  }

  void _handleTap() {
    if (widget.detail == null) {
      widget.onTap();
      return;
    }
    // Collapsed -> mid and full -> collapsed cross the accordion's own
    // open/closed boundary, so the parent needs to hear about those; the
    // middle step (mid -> full) is purely how much of an already-open row
    // is showing, which this row can decide entirely on its own.
    switch (_level) {
      case CollectionDetailLevel.collapsed:
        setState(() => _level = CollectionDetailLevel.mid);
        widget.onTap();
      case CollectionDetailLevel.mid:
        setState(() => _level = CollectionDetailLevel.full);
      case CollectionDetailLevel.full:
        widget.onTap();
    }
  }

  @override
  Widget build(BuildContext context) {
    final collection = widget.collection;
    final torrent = !collection.isShared;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final showsExtras = widget.detail == null || _level != CollectionDetailLevel.collapsed;

    return SurfaceCard(
      onTap: _handleTap,
      // Energy by what it is genuinely doing: transferring glows brightest,
      // shared-and-standing-by glows calmly, everything else not at all. The
      // wash that separates a live row from the settled ones comes with it —
      // see [Glow.gradient].
      glow: collection.glow,
      glowColor: accent,
      glowIntensity: collection.liveIntensity,
      borderColor: widget.selected && collection.glow == GlowLevel.none
          ? AppColors.borderStrong
          : null,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          _row(),
          if (widget.onCommand != null &&
              widget.detail == null &&
              showsExtras) ...[
            const SizedBox(height: 12),
            CollectionCommandBar(
              onCommand: widget.onCommand!,
              busy: false,
            ),
          ],
          if (widget.detail != null && _level != CollectionDetailLevel.collapsed) ...[
            Divider(height: 26, color: AppColors.border),
            widget.detail!(_level),
          ],
        ],
      ),
    );
  }

  Widget _row() {
    final collection = widget.collection;
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
                  child: Icon(Icons.download_outlined,
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
            label: formatProgressPercent(collection.progress),
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
            Icon(Icons.error_outline, size: 40, color: AppColors.danger),
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
            SizedBox(
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
