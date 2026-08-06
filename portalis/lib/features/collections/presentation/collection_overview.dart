import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection.dart';
import '../domain/peer_observation.dart';
import '../domain/transfer_history.dart';
import 'collection_commands.dart';
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
    required this.onSync,
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
  final VoidCallback onSync;
  final VoidCallback onFetch;
  final List<PeerObservation> peerHistory;
  final ValueChanged<String>? onForgetPeer;

  @override
  Widget build(BuildContext context) {
    final shown = collection.collaborators.take(6).toList();
    final transferHistory = [
      for (final sample in history?.samples ?? const <TransferSample>[])
        TransferPoint(
          at: sample.at,
          downloadMbps: sample.downloadMbps,
          uploadMbps: sample.uploadMbps,
        ),
    ];
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
            busy: busy,
            onCommand: onCommand,
          ),
          const SizedBox(height: 16),
        ],
        // Info hash and who's connected are the deepest layer of detail —
        // present, but worth an extra tap, not shown the moment a row grows
        // at all.
        if (level == CollectionDetailLevel.full) ...[
          _CollectionIdentifiers(collection: collection),
          const SizedBox(height: 14),
          _Collaborators(
            collection: collection,
            shown: shown,
            remaining: collection.collaborators.length - shown.length,
            peerHistory: peerHistory,
            onForgetPeer: onForgetPeer,
          ),
          const SizedBox(height: 14),
        ],
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
            PillButton(label: 'Add media', dim: true, onTap: busy ? null : onAddMedia),
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

class _Collaborators extends StatelessWidget {
  const _Collaborators({
    required this.collection,
    required this.shown,
    required this.remaining,
    required this.peerHistory,
    required this.onForgetPeer,
  });

  final Collection collection;
  final List<Collaborator> shown;
  final int remaining;
  final List<PeerObservation> peerHistory;
  final ValueChanged<String>? onForgetPeer;

  @override
  Widget build(BuildContext context) {
    if (shown.isEmpty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel('PEERS - ${collection.livePeers}'),
          const SizedBox(height: 7),
          Text(
            collection.livePeers == 0
                ? 'No peers connected right now'
                : '${collection.peersLabel} connected - not identified by name',
            style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
          ),
          const SizedBox(height: 14),
          _TorrentPeers(
            collection: collection,
            history: peerHistory,
            onForgetPeer: onForgetPeer,
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SectionLabel('COLLABORATORS - ${collection.collaborators.length}'),
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
        const SizedBox(height: 14),
        _TorrentPeers(
          collection: collection,
          history: peerHistory,
          onForgetPeer: onForgetPeer,
        ),
      ],
    );
  }
}

class _TorrentPeers extends StatelessWidget {
  const _TorrentPeers({
    required this.collection,
    required this.history,
    required this.onForgetPeer,
  });

  final Collection collection;
  final List<PeerObservation> history;
  final ValueChanged<String>? onForgetPeer;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel('TORRENT PEERS - ${_peers.length}'),
          const SizedBox(height: 7),
          if (history.isEmpty && collection.torrentPeers.isEmpty)
            Text(
              collection.livePeers == 0
                  ? 'No torrent peers connected right now'
                  : '${collection.peersLabel} connected - addresses unavailable',
              style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
            )
          else
            Wrap(
              spacing: 8,
              runSpacing: 6,
              children: [
                for (final peer in _peers)
                  Container(
                    padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
                    decoration: BoxDecoration(
                      color: AppColors.emberWash,
                      borderRadius: BorderRadius.circular(8),
                      border: Border.all(
                        color: AppColors.ember.withValues(alpha: 0.35),
                      ),
                    ),
                    child: Row(
                      mainAxisSize: MainAxisSize.min,
                      children: [
                        Flexible(
                          child: Text(
                            peer.address,
                            overflow: TextOverflow.ellipsis,
                            style: monoLabel(
                              size: 10.5,
                              color: AppColors.ember,
                              letterSpacing: 0,
                            ),
                          ),
                        ),
                        const SizedBox(width: 6),
                        Text(
                          formatLastSeen(peer.lastSeen),
                          style: monoLabel(
                            size: 9,
                            color: AppColors.textDim,
                            letterSpacing: 0,
                          ),
                        ),
                        if (onForgetPeer != null)
                          IconButton(
                            tooltip: 'Forget peer',
                            visualDensity: VisualDensity.compact,
                            padding: EdgeInsets.zero,
                            constraints: const BoxConstraints(),
                            icon: const Icon(Icons.close, size: 14),
                            color: AppColors.textDim,
                            onPressed: () => onForgetPeer!(peer.address),
                          ),
                      ],
                    ),
                  ),
              ],
            ),
        ],
      );

  List<PeerObservation> get _peers {
    if (history.isNotEmpty) return history;
    final now = DateTime.now();
    return [
      for (final address in collection.torrentPeers)
        PeerObservation(
          collectionId: collection.id,
          collectionName: collection.name,
          address: address,
          lastSeen: now,
        ),
    ];
  }
}
