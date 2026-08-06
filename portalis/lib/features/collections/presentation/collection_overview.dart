import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection.dart';
import '../domain/peer_observation.dart';
import '../domain/transfer_history.dart';
import 'collection_commands.dart';
import 'collection_import_progress.dart';
import 'collection_peers.dart';
import 'collection_presentation.dart';

/// Live collection facts and actions, independent of navigation and commands.
class CollectionOverview extends StatelessWidget {
  const CollectionOverview({
    super.key,
    required this.collection,
    required this.busy,
    required this.onCommand,
    this.history,
    this.showCommands = true,
    this.level = CollectionDetailLevel.full,
    this.showTitle = true,
    required this.onInvite,
    required this.onAddMedia,
    required this.onFetch,
    this.peerHistory = const [],
    this.onForgetPeer,
  });

  final Collection collection;
  final bool busy;
  final ValueChanged<CollectionCommand> onCommand;
  final TransferHistory? history;
  final bool showCommands;
  final CollectionDetailLevel level;
  final bool showTitle;
  final VoidCallback onInvite;
  final VoidCallback onAddMedia;
  final VoidCallback onFetch;
  final List<PeerObservation> peerHistory;
  final ValueChanged<String>? onForgetPeer;

  @override
  Widget build(BuildContext context) {
    final transferHistory = [
      for (final sample in history?.samples ?? const <TransferSample>[])
        TransferPoint(
          at: sample.at,
          downloadMbps: sample.downloadMbps,
          uploadMbps: sample.uploadMbps,
        ),
    ];
    final ingestion = collection.ingestion;
    final commandBusy = busy || (ingestion != null && !ingestion.failed);
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        _CollectionControls(
          collection: collection,
        ),
        if (showTitle) ...[
          Text(
            collection.name,
            style: displayText(size: 23, weight: FontWeight.w700),
          ),
          const SizedBox(height: 8),
        ],
        Text(
          collection.isShared
              ? 'Shared collection - ${collection.subtitle}'
              : 'Torrent - ${collection.subtitle}',
          style: monoLabel(
            size: 12,
            color: AppColors.textDim,
            letterSpacing: 0,
          ),
        ),
        const SizedBox(height: 6),
        CopiesIndicator(
          color: collection.hue,
          label: collection.copiesLabel,
          fontSize: 13,
        ),
        if (ingestion != null) ...[
          const SizedBox(height: 10),
          CollectionImportProgress(ingestion: ingestion),
        ],
        if (collection.totalBytes > 0 ||
            collection.downloadMbps > 0 ||
            collection.uploadMbps > 0 ||
            transferHistory.isNotEmpty) ...[
          const SizedBox(height: 10),
          TransferPanel(
            progress: collection.progress,
            downloadedBytes: collection.downloadedBytes,
            totalBytes: collection.totalBytes,
            downloadMbps: collection.downloadMbps,
            uploadMbps: collection.uploadMbps,
            history: transferHistory,
            startedAt: history?.startedAt,
            completedAt: history?.completedAt,
            livePeers: collection.livePeers,
            etaLabel: collection.etaLabel,
            color: collection.hue,
          ),
        ],
        if (showCommands) ...[
          const SizedBox(height: 14),
          CollectionCommandBar(
            busy: commandBusy,
            onCommand: onCommand,
            trailingActions: [
              if (collection.isShared)
                PillButton(
                  label: 'Invite',
                  icon: const Icon(
                    Icons.people_alt_outlined,
                    size: 16,
                    color: AppColors.signalSoft,
                  ),
                  onTap: commandBusy ? null : onInvite,
                ),
              if (collection.isShared)
                PillButton(
                  label: 'Add media',
                  dim: true,
                  onTap: commandBusy ? null : onAddMedia,
                ),
              if (collection.pendingMedia > 0)
                PillButton(
                  label: 'Fetch ${collection.pendingMedia}',
                  onTap: commandBusy ? null : onFetch,
                ),
            ],
          ),
          const SizedBox(height: 12),
        ],
        if (level == CollectionDetailLevel.full) ...[
          _CollectionIdentifiers(collection: collection),
          const SizedBox(height: 14),
          CollectionPeers(
            collection: collection,
            peerHistory: peerHistory,
            onForgetPeer: onForgetPeer,
          ),
          const SizedBox(height: 14),
        ],
      ],
    );
  }
}
class _CollectionControls extends StatelessWidget {
  const _CollectionControls({required this.collection});

  final Collection collection;

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
      ],
    );
  }
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
                  ? 'Shared collection - invite-based, can grow'
                  : 'Torrent - fixed contents',
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
