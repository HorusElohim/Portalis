import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../media/domain/media_item.dart';
import '../../media/presentation/media_thumbnail.dart';
import '../domain/collection.dart';
import 'collection_presentation.dart';

/// Manifest-entry grouping and media tiles for a collection.
class CollectionContents extends StatelessWidget {
  const CollectionContents({
    super.key,
    required this.collection,
    required this.onOpenMedia,
  });

  final Collection collection;
  final ValueChanged<MediaItem> onOpenMedia;

  @override
  Widget build(BuildContext context) {
    final entries = collection.entries;
    if (!collection.isShared || entries.length <= 1) {
      return _MediaGrid(
        collection: collection,
        media: collection.media,
        onOpenMedia: onOpenMedia,
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        for (final entry in entries) ...[
          _EntryHeader(collection: collection, entry: entry),
          const SizedBox(height: 8),
          _MediaGrid(
            collection: collection,
            media: entry.media,
            onOpenMedia: onOpenMedia,
          ),
          const SizedBox(height: 18),
        ],
      ],
    );
  }
}

class _EntryHeader extends StatelessWidget {
  const _EntryHeader({required this.collection, required this.entry});

  final Collection collection;
  final CollectionEntry entry;

  @override
  Widget build(BuildContext context) {
    final author = entry.addedBy == null
        ? null
        : collection.collaborators
            .where((item) => item.deviceId == entry.addedBy)
            .firstOrNull;
    final facts = <String>[
      if (entry.fetched) plural(entry.media.length, 'file'),
      if (entry.totalBytes > 0) formatBytes(entry.totalBytes),
      if (author != null) 'from ${author.name}',
    ];

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Expanded(
              child: Text(
                entry.label,
                overflow: TextOverflow.ellipsis,
                style: displayText(size: 13.5),
              ),
            ),
            if (!entry.fetched) ...[
              const SizedBox(width: 8),
              StatusBadge(label: 'NOT FETCHED'),
            ],
          ],
        ),
        if (facts.isNotEmpty) ...[
          const SizedBox(height: 2),
          Text(
            facts.join(' · '),
            overflow: TextOverflow.ellipsis,
            style: monoLabel(size: 10, letterSpacing: 0.2),
          ),
        ],
        if (entry.fetched && entry.totalBytes > 0 && entry.progress < 1.0) ...[
          const SizedBox(height: 7),
          ClipRRect(
            borderRadius: BorderRadius.circular(AppRadius.pill),
            child: LinearProgressIndicator(
              value: entry.progress,
              minHeight: 3,
              backgroundColor: AppColors.borderStrong,
              valueColor: AlwaysStoppedAnimation(collection.hue),
            ),
          ),
        ],
      ],
    );
  }
}

class _MediaGrid extends StatelessWidget {
  const _MediaGrid({
    required this.collection,
    required this.media,
    required this.onOpenMedia,
  });

  final Collection collection;
  final List<MediaItem> media;
  final ValueChanged<MediaItem> onOpenMedia;

  @override
  Widget build(BuildContext context) => GridView.builder(
        shrinkWrap: true,
        physics: const NeverScrollableScrollPhysics(),
        gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
          crossAxisCount: 3,
          mainAxisSpacing: 10,
          crossAxisSpacing: 8,
          childAspectRatio: 0.76,
        ),
        itemCount: media.length,
        itemBuilder: (context, index) {
          final item = media[index];
          return Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: PerimeterProgress(
                  progress: item.progress,
                  color: collection.hue,
                  borderRadius: BorderRadius.circular(AppRadius.tight),
                  child: Container(
                    decoration: BoxDecoration(
                      borderRadius: BorderRadius.circular(AppRadius.tight),
                      border: Border.all(color: AppColors.border),
                    ),
                    clipBehavior: Clip.antiAlias,
                    child: Material(
                      color: Colors.transparent,
                      child: InkWell(
                        onTap: () => onOpenMedia(item),
                        child: Stack(
                          fit: StackFit.expand,
                          children: [
                            MediaThumbnail(media: item, borderRadius: 6),
                            if (!item.fetched)
                              Container(
                                color: AppColors.surfaceDeep.withValues(alpha: 0.55),
                                alignment: Alignment.center,
                                child: const Icon(
                                  Icons.cloud_download_outlined,
                                  size: 22,
                                  color: AppColors.signalSoft,
                                ),
                              ),
                          ],
                        ),
                      ),
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 5),
              Text(
                item.label,
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: AppText.caption(color: AppColors.text, height: 1.1),
              ),
              Text(
                !item.fetched
                    ? 'not fetched'
                    : item.progress < 1.0
                        ? '${(item.progress * 100).toStringAsFixed(0)}%'
                        : item.sizeBytes > 0
                            ? formatBytes(item.sizeBytes)
                            : '',
                maxLines: 1,
                overflow: TextOverflow.ellipsis,
                style: monoLabel(size: 9.5, letterSpacing: 0.2),
              ),
            ],
          );
        },
      );
}
