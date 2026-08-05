import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection.dart';
import 'collection_presentation.dart';

/// Live collection facts and actions, independent of navigation and commands.
class CollectionOverview extends StatelessWidget {
  const CollectionOverview({
    super.key,
    required this.collection,
    required this.showHeading,
    required this.showDetails,
    required this.busy,
    required this.onToggleDetails,
    required this.onDelete,
    required this.onInvite,
    required this.onAddMedia,
    required this.onSync,
    required this.onFetch,
  });

  final Collection collection;
  final bool showHeading;
  final bool showDetails;
  final bool busy;
  final VoidCallback onToggleDetails;
  final VoidCallback onDelete;
  final VoidCallback onInvite;
  final VoidCallback onAddMedia;
  final VoidCallback onSync;
  final VoidCallback onFetch;

  @override
  Widget build(BuildContext context) {
    final shown = collection.collaborators.take(6).toList();
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _CollectionControls(
          collection: collection,
          showDetails: showDetails,
          busy: busy,
          onToggleDetails: onToggleDetails,
          onDelete: onDelete,
        ),
        if (showHeading) ...[
          Text(collection.name, style: AppText.action()),
          const SizedBox(height: 6),
          Text(
            collection.isShared
                ? 'Shared collection · ${collection.subtitle}'
                : 'Torrent · ${collection.subtitle}',
            style:
                monoLabel(size: 10.5, color: AppColors.textDim, letterSpacing: 0),
          ),
          const SizedBox(height: 6),
        ],
        CopiesIndicator(
          color: collection.hue,
          label: collection.copiesLabel,
          fontSize: 12,
        ),
        if (collection.totalBytes > 0 ||
            collection.downloadMbps > 0 ||
            collection.uploadMbps > 0) ...[
          const SizedBox(height: 10),
          TransferFacts(
            progress: collection.progress,
            downloadedBytes: collection.downloadedBytes,
            totalBytes: collection.totalBytes,
            downloadMbps: collection.downloadMbps,
            uploadMbps: collection.uploadMbps,
            livePeers: collection.livePeers,
            etaLabel: collection.etaLabel,
            color: collection.hue,
          ),
        ],
        AnimatedSize(
          duration: const Duration(milliseconds: 180),
          curve: Curves.easeOutCubic,
          alignment: Alignment.topCenter,
          child: showDetails
              ? _CollectionIdentifiers(collection: collection)
              : const SizedBox(width: double.infinity),
        ),
        const SizedBox(height: 14),
        _Collaborators(
          collection: collection,
          shown: shown,
          remaining: collection.collaborators.length - shown.length,
        ),
        const SizedBox(height: 14),
        _CollectionActions(
          collection: collection,
          busy: busy,
          onInvite: onInvite,
          onAddMedia: onAddMedia,
          onSync: onSync,
          onFetch: onFetch,
        ),
      ],
    );
  }
}

class _CollectionControls extends StatelessWidget {
  const _CollectionControls({
    required this.collection,
    required this.showDetails,
    required this.busy,
    required this.onToggleDetails,
    required this.onDelete,
  });

  final Collection collection;
  final bool showDetails;
  final bool busy;
  final VoidCallback onToggleDetails;
  final VoidCallback onDelete;

  @override
  Widget build(BuildContext context) {
    final admins = collection.collaborators.where((item) => item.isAdmin).length;
    return Row(
      children: [
        if (admins > 0)
          Text(
            '$admins admin${admins == 1 ? '' : 's'}',
            style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
          ),
        const Spacer(),
        IconButton(
          tooltip: showDetails ? 'Hide details' : 'Details',
          icon: Icon(
            showDetails ? Icons.info_rounded : Icons.info_outline,
            size: 18,
            color: showDetails ? AppColors.signalSoft : AppColors.textDim,
          ),
          onPressed: onToggleDetails,
        ),
        IconButton(
          tooltip: 'Remove from this device',
          icon: const Icon(Icons.delete_outline, size: 18, color: AppColors.textDim),
          onPressed: busy ? null : onDelete,
        ),
      ],
    );
  }
}

class _CollectionActions extends StatelessWidget {
  const _CollectionActions({
    required this.collection,
    required this.busy,
    required this.onInvite,
    required this.onAddMedia,
    required this.onSync,
    required this.onFetch,
  });

  final Collection collection;
  final bool busy;
  final VoidCallback onInvite;
  final VoidCallback onAddMedia;
  final VoidCallback onSync;
  final VoidCallback onFetch;

  @override
  Widget build(BuildContext context) => Wrap(
        spacing: 10,
        runSpacing: 8,
        children: [
          if (collection.isShared) ...[
            PillButton(
              label: 'Invite',
              icon: const Icon(
                Icons.people_alt_outlined,
                size: 16,
                color: AppColors.signalSoft,
              ),
              onTap: busy ? null : onInvite,
            ),
            PillButton(label: '＋ Add media', dim: true, onTap: busy ? null : onAddMedia),
            PillButton(label: 'Sync', dim: true, onTap: busy ? null : onSync),
          ],
          if (collection.pendingMedia > 0)
            PillButton(
              label: 'Fetch ${collection.pendingMedia}',
              onTap: busy ? null : onFetch,
            ),
        ],
      );
}

class _CollectionIdentifiers extends StatelessWidget {
  const _CollectionIdentifiers({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.only(top: 12),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            InfoRow(
              label: 'Type',
              value: collection.isShared
                  ? 'Shared — invite-based, can grow'
                  : 'Torrent — fixed contents',
            ),
            InfoRow(
              label: 'State',
              value: collection.state.isEmpty ? 'Unknown' : collection.state,
            ),
            if (collection.uploadedBytes > 0)
              InfoRow(
                label: 'Uploaded',
                value: formatBytesPrecise(collection.uploadedBytes),
              ),
            InfoRow(
              label: collection.isShared ? 'Collection id' : 'Info hash',
              value: collection.id,
              monospace: true,
              copyable: true,
            ),
          ],
        ),
      );
}

class _Collaborators extends StatelessWidget {
  const _Collaborators({
    required this.collection,
    required this.shown,
    required this.remaining,
  });

  final Collection collection;
  final List<Collaborator> shown;
  final int remaining;

  @override
  Widget build(BuildContext context) {
    if (shown.isEmpty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel('PEERS · ${collection.livePeers}'),
          const SizedBox(height: 7),
          Text(
            collection.livePeers == 0
                ? 'No peers connected right now'
                : '${collection.peersLabel} connected · not identified by name',
            style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SectionLabel('COLLABORATORS · ${collection.collaborators.length}'),
        const SizedBox(height: 7),
        SizedBox(
          height: 27,
          child: Row(
            children: [
              SizedBox(
                width: 27.0 + (shown.length - 1) * 19,
                child: Stack(
                  children: [
                    for (var index = 0; index < shown.length; index++)
                      Positioned(
                        left: index * 19.0,
                        child: Container(
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            border: Border.all(
                              color: AppColors.surfaceDeep,
                              width: 2,
                            ),
                          ),
                          child: Avatar(initials: shown[index].initials, size: 27),
                        ),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  remaining > 0 ? '+$remaining more' : shown.map((item) => item.name).join(', '),
                  overflow: TextOverflow.ellipsis,
                  style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}
