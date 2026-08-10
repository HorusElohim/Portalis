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
  /// shown from [CollectionDetailLevel.mid] on. The builder also receives the
  /// row identity so expanded detail can compose it into one horizontal live
  /// header rather than stacking another section underneath it. Where a
  /// window is wide enough, opening a collection means this card growing to
  /// hold it — not a second panel beside the list describing the same thing
  /// twice. `null` where there is nowhere to grow into (the compact list,
  /// which pushes a separate screen instead) — the row then keeps its older,
  /// simpler behaviour: a plain tap, and the command bar always showing.
  final Widget Function(
    CollectionDetailLevel level,
    Widget inlineHeader,
    Widget inlineStatus,
  )? detail;
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
    final showsExtras =
        widget.detail == null || _level != CollectionDetailLevel.collapsed;
    final detailsOpen =
        widget.detail != null && _level != CollectionDetailLevel.collapsed;

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
          if (!detailsOpen) _row(),
          if (widget.onCommand != null &&
              widget.detail == null &&
              showsExtras) ...[
            const SizedBox(height: 12),
            CollectionCommandBar(
              onCommand: widget.onCommand!,
              busy: false,
            ),
          ],
          if (detailsOpen)
            widget.detail!(
              _level,
              _row(showStatus: false),
              _status(),
            ),
        ],
      ),
    );
  }

  Widget _row({bool showStatus = true}) {
    final collection = widget.collection;
    final torrent = !collection.isShared;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final live = collection.downloadMbps > 0 || collection.uploadMbps > 0;
    final downloading = collection.state == 'downloading';
    // No metadata has arrived, so there is no total to measure against — an
    // indeterminate bar is the honest shape for "reaching out to a peer".
    final connecting = collection.isConnecting;
    final detailsOpen =
        widget.detail != null && _level != CollectionDetailLevel.collapsed;
    final itemCount = collection.media.length;
    final admins =
        collection.collaborators.where((item) => item.isAdmin).length;
    final subtitle = detailsOpen
        ? [
            '$itemCount item${itemCount == 1 ? '' : 's'}',
            if (admins > 0) '$admins admin${admins == 1 ? '' : 's'}',
          ].join(' · ')
        : collection.subtitle;
    return Row(
      children: [
        SizedBox(
          width: 52,
          height: 52,
          child: torrent
              ? _TorrentCollectionTile(accent: accent)
              : _SharedCollectionTile(
                  collection: collection,
                  accent: accent,
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
                subtitle,
                overflow: TextOverflow.ellipsis,
                style: monoLabel(size: 11, letterSpacing: 0.2),
              ),
              if ((downloading || connecting) && !detailsOpen) ...[
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
        if (showStatus) ...[
          const SizedBox(width: 12),
          _status(),
        ],
      ],
    );
  }

  Widget _status() {
    final collection = widget.collection;
    final accent = collection.isShared ? AppColors.signal : AppColors.ember;
    final detailsOpen =
        widget.detail != null && _level != CollectionDetailLevel.collapsed;
    if (collection.state == 'downloading') {
      return StatusBadge(
        label: detailsOpen
            ? 'DOWNLOADING'
            : formatProgressPercent(collection.progress),
        color: accent,
      );
    }
    if (collection.isSharing) {
      // Mint here is earned: this device is genuinely serving the collection.
      return StatusBadge(label: 'SHARING', color: accent);
    }
    return StatusBadge(label: collection.state.toUpperCase());
  }
}

class _TorrentCollectionTile extends StatelessWidget {
  const _TorrentCollectionTile({required this.accent});

  final Color accent;

  @override
  Widget build(BuildContext context) => DecoratedBox(
        decoration: BoxDecoration(
          gradient: LinearGradient(
            begin: Alignment.topLeft,
            end: Alignment.bottomRight,
            colors: [
              accent.withValues(alpha: 0.24),
              accent.withValues(alpha: 0.08),
            ],
          ),
          borderRadius: BorderRadius.circular(AppRadius.control),
          border: Border.all(color: accent.withValues(alpha: 0.28)),
          boxShadow: [
            BoxShadow(
              color: accent.withValues(alpha: 0.16),
              blurRadius: 14,
              spreadRadius: -4,
            ),
          ],
        ),
        child: Center(
          child: Icon(Icons.download_outlined, size: 22, color: accent),
        ),
      );
}

class _SharedCollectionTile extends StatelessWidget {
  const _SharedCollectionTile({
    required this.collection,
    required this.accent,
  });

  final Collection collection;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    final media = collection.media.firstOrNull;
    final hasPreview = media?.isReady ?? false;
    return DecoratedBox(
      decoration: BoxDecoration(
        gradient: LinearGradient(
          begin: Alignment.topLeft,
          end: Alignment.bottomRight,
          colors: [
            accent.withValues(alpha: 0.2),
            AppColors.surfaceRaised,
          ],
        ),
        borderRadius: BorderRadius.circular(AppRadius.control),
        border: Border.all(color: accent.withValues(alpha: 0.3)),
        boxShadow: [
          BoxShadow(
            color: accent.withValues(alpha: 0.14),
            blurRadius: 14,
            spreadRadius: -4,
          ),
        ],
      ),
      child: ClipRRect(
        borderRadius: BorderRadius.circular(AppRadius.control - 1),
        child: Stack(
          fit: StackFit.expand,
          children: [
            if (hasPreview)
              MediaThumbnail(media: media!, borderRadius: AppRadius.control)
            else
              _SharedCollectionPlaceholder(accent: accent),
            DecoratedBox(
              decoration: BoxDecoration(
                gradient: LinearGradient(
                  begin: Alignment.topCenter,
                  end: Alignment.bottomCenter,
                  colors: [
                    Colors.transparent,
                    Colors.black.withValues(alpha: 0.2),
                  ],
                ),
              ),
            ),
            Positioned(
              right: 5,
              top: 5,
              child: Icon(
                Icons.people_alt_outlined,
                size: 14,
                color: hasPreview ? Colors.white : accent,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _SharedCollectionPlaceholder extends StatelessWidget {
  const _SharedCollectionPlaceholder({required this.accent});

  final Color accent;

  @override
  Widget build(BuildContext context) => Center(
        child: Icon(Icons.hub_outlined, size: 23, color: accent),
      );
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
