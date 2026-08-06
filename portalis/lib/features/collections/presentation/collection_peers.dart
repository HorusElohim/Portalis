import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../domain/collection.dart';
import '../domain/peer_observation.dart';
import 'collection_presentation.dart';

/// Presents identified collaborators and anonymous torrent observations as one
/// peer surface while keeping their identity guarantees visually distinct.
class CollectionPeers extends StatelessWidget {
  const CollectionPeers({
    super.key,
    required this.collection,
    this.peerHistory = const [],
    this.onForgetPeer,
  });

  final Collection collection;
  final List<PeerObservation> peerHistory;
  final ValueChanged<String>? onForgetPeer;

  @override
  Widget build(BuildContext context) {
    final namedPeers = collection.collaborators.take(6).toList();
    final torrentPeers = _torrentPeers;
    final total = collection.collaborators.length + torrentPeers.length;
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SectionLabel('PEERS - $total'),
        const SizedBox(height: 7),
        if (total == 0)
          Text(
            collection.livePeers == 0
                ? 'No peers connected right now'
                : '${collection.peersLabel} connected - addresses unavailable',
            style: monoLabel(size: 10, color: AppColors.textDim, letterSpacing: 0),
          )
        else
          Wrap(
            spacing: 8,
            runSpacing: 7,
            children: [
              for (final collaborator in namedPeers)
                _NamedPeer(collaborator: collaborator),
              if (collection.collaborators.length > namedPeers.length)
                _PeerLabel(
                  label: '+${collection.collaborators.length - namedPeers.length} more',
                ),
              for (final peer in torrentPeers)
                _AnonymousPeer(peer: peer, onForget: onForgetPeer),
            ],
          ),
      ],
    );
  }

  List<PeerObservation> get _torrentPeers {
    if (peerHistory.isNotEmpty) return peerHistory;
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

class _NamedPeer extends StatelessWidget {
  const _NamedPeer({required this.collaborator});

  final Collaborator collaborator;

  @override
  Widget build(BuildContext context) => _PeerLabel(
        label: collaborator.name,
        leading: Avatar(initials: collaborator.initials, size: 20),
      );
}

class _AnonymousPeer extends StatelessWidget {
  const _AnonymousPeer({required this.peer, required this.onForget});

  final PeerObservation peer;
  final ValueChanged<String>? onForget;

  @override
  Widget build(BuildContext context) => _PeerLabel(
        label: peer.address,
        detail: formatLastSeen(peer.lastSeen),
        color: AppColors.ember,
        onForget: onForget == null ? null : () => onForget!(peer.address),
      );
}

class _PeerLabel extends StatelessWidget {
  const _PeerLabel({
    required this.label,
    this.leading,
    this.detail,
    this.color = AppColors.textDim,
    this.onForget,
  });

  final String label;
  final Widget? leading;
  final String? detail;
  final Color color;
  final VoidCallback? onForget;

  @override
  Widget build(BuildContext context) => Container(
        padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 6),
        decoration: BoxDecoration(
          color: color == AppColors.ember
              ? AppColors.emberWash
              : AppColors.surface,
          borderRadius: BorderRadius.circular(8),
          border: Border.all(
            color: color == AppColors.ember
                ? AppColors.ember.withValues(alpha: 0.35)
                : AppColors.border,
          ),
        ),
        child: Row(
          mainAxisSize: MainAxisSize.min,
          children: [
            if (leading != null) ...[leading!, const SizedBox(width: 6)],
            Text(
              label,
              overflow: TextOverflow.ellipsis,
              style: monoLabel(size: 10.5, color: color, letterSpacing: 0),
            ),
            if (detail != null) ...[
              const SizedBox(width: 6),
              Text(
                detail!,
                style: monoLabel(size: 9, color: AppColors.textDim, letterSpacing: 0),
              ),
            ],
            if (onForget != null)
              IconButton(
                tooltip: 'Forget peer',
                visualDensity: VisualDensity.compact,
                padding: EdgeInsets.zero,
                constraints: const BoxConstraints(),
                icon: const Icon(Icons.close, size: 14),
                color: AppColors.textDim,
                onPressed: onForget,
              ),
          ],
        ),
      );
}
