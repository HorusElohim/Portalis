import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/collection.dart';
import '../theme.dart';

/// Distinct collaborators across every collection, and where they appear.
///
/// The transfer figures on a collaborator card are pooled from the
/// collections that collaborator belongs to. The native engine reports
/// transfer activity per collection, not per signed identity, so the card
/// deliberately describes the aggregate rather than pretending to know which
/// individual connection consumed which bytes.
class PeopleScreen extends StatelessWidget {
  const PeopleScreen({super.key, this.embedded = false});

  final bool embedded;

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final byDevice = <String,
        ({
          Collaborator who,
          List<String> collections,
          int totalBytes,
          double downloadMbps,
          double uploadMbps,
          bool isSharing,
        })>{};
    for (final collection in AppControllers.collections.collections) {
      for (final collaborator in collection.collaborators) {
        final entry = byDevice[collaborator.deviceId];
        if (entry == null) {
          byDevice[collaborator.deviceId] = (
            who: collaborator,
            collections: [collection.name],
            totalBytes: collection.totalBytes,
            downloadMbps: collection.downloadMbps,
            uploadMbps: collection.uploadMbps,
            isSharing: collection.isSharing,
          );
        } else {
          entry.collections.add(collection.name);
          byDevice[collaborator.deviceId] = (
            who: entry.who,
            collections: entry.collections,
            totalBytes: entry.totalBytes + collection.totalBytes,
            downloadMbps: entry.downloadMbps + collection.downloadMbps,
            uploadMbps: entry.uploadMbps + collection.uploadMbps,
            isSharing: entry.isSharing || collection.isSharing,
          );
        }
      }
    }
    final people = byDevice.values.toList();

    // Anonymous swarm peers across every collection. An address is a
    // connection, not a signed identity. Only its collection names and last
    // seen time are known at this level; per-peer speed and bytes are not.
    final byAddress = <String,
        ({
          String address,
          List<String> collections,
          DateTime lastSeen,
        })>{};
    for (final peer in AppControllers.collections.peerHistory) {
      final entry = byAddress[peer.address];
      if (entry == null) {
        byAddress[peer.address] = (
          address: peer.address,
          collections: [peer.collectionName],
          lastSeen: peer.lastSeen,
        );
      } else {
        if (!entry.collections.contains(peer.collectionName)) {
          entry.collections.add(peer.collectionName);
        }
        byAddress[peer.address] = (
          address: entry.address,
          collections: entry.collections,
          lastSeen: peer.lastSeen.isAfter(entry.lastSeen)
              ? peer.lastSeen
              : entry.lastSeen,
        );
      }
    }
    final torrentPeople = byAddress.values.toList();

    final cards = [
      for (final entry in people) PersonCard(entry: entry),
      for (final entry in torrentPeople) TorrentPeerCard(entry: entry),
    ];

    return AppScreen(
      title: 'People',
      subtitle: cards.isEmpty
          ? null
          : Text(_subtitle(people.length, torrentPeople.length)),
      embedded: embedded,
      width: ScreenWidth.full,
      body: cards.isEmpty
          ? Center(
              child: Padding(
                padding: EdgeInsets.symmetric(horizontal: kScreenGutter),
                child: Text(
                  'Nobody yet. Collaborators appear once you share or join, '
                  'and torrent peers once something is downloading.',
                  textAlign: TextAlign.center,
                  style: AppText.body(color: AppColors.textDim),
                ),
              ),
            )
          : WindowBuilder(
              builder: (context, window) => GridView.builder(
                padding: const EdgeInsets.fromLTRB(
                    kScreenGutter, 0, kScreenGutter, 28),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: window.columns(340),
                  mainAxisSpacing: 10,
                  crossAxisSpacing: 10,
                  mainAxisExtent: 180,
                ),
                itemCount: cards.length,
                itemBuilder: (context, i) => cards[i],
              ),
            ),
    );
  }

  String _subtitle(int collaborators, int torrentPeers) {
    final parts = [
      if (collaborators > 0)
        '$collaborators collaborator${collaborators == 1 ? '' : 's'}',
      if (torrentPeers > 0)
        '$torrentPeers torrent peer${torrentPeers == 1 ? '' : 's'}',
    ];
    return '${parts.join(', ')} across your collections';
  }
}

/// A collaborator summary card. Named collaborators and anonymous swarm
/// peers share the same visual language while their identity colors remain
/// distinct.
class PersonCard extends StatelessWidget {
  const PersonCard._({
    super.key,
    required this.avatar,
    required this.title,
    required this.subtitle,
    this.totalBytes = 0,
    this.sharedCount = 0,
    this.rateMbps = 0,
    this.status = 'IDLE',
    this.statusColor = AppColors.textFaint,
    this.active = false,
    this.metricColor = AppColors.signal,
    this.subtitleColor = AppColors.textFaint,
    this.trailing,
  });

  factory PersonCard({
    Key? key,
    required ({
      Collaborator who,
      List<String> collections,
      int totalBytes,
      double downloadMbps,
      double uploadMbps,
      bool isSharing,
    }) entry,
  }) {
    final rateMbps = entry.uploadMbps > 0
        ? entry.uploadMbps
        : entry.downloadMbps;
    return PersonCard._(
      key: key,
      avatar: Avatar(initials: entry.who.initials, size: 44),
      title: entry.who.isAdmin ? '${entry.who.name} · admin' : entry.who.name,
      subtitle: entry.collections.join(' · '),
      totalBytes: entry.totalBytes,
      sharedCount: entry.collections.length,
      rateMbps: rateMbps,
      status: _statusLabel(
        uploadMbps: entry.uploadMbps,
        downloadMbps: entry.downloadMbps,
        isSharing: entry.isSharing,
      ),
      statusColor: rateMbps > 0 ? AppColors.signal : AppColors.textFaint,
      active: rateMbps > 0,
    );
  }

  final Widget avatar;
  final String title;
  final String subtitle;
  final int? totalBytes;
  final int sharedCount;
  final double? rateMbps;
  final String status;
  final Color statusColor;
  final bool active;
  final Color metricColor;
  final Color subtitleColor;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      padding: const EdgeInsets.all(16),
      glow: active ? GlowLevel.active : GlowLevel.none,
      glowIntensity: Glow.intensityForRate(rateMbps ?? 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              avatar,
              const SizedBox(width: 12),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Text(
                      title,
                      overflow: TextOverflow.ellipsis,
                      style: AppText.cardTitle(),
                    ),
                    const SizedBox(height: 4),
                    Row(
                      children: [
                        if (active) ...[
                          Container(
                            width: 6,
                            height: 6,
                            decoration: const BoxDecoration(
                              color: AppColors.signal,
                              shape: BoxShape.circle,
                            ),
                          ),
                          const SizedBox(width: 6),
                        ],
                        Flexible(
                          child: Text(
                            status,
                            overflow: TextOverflow.ellipsis,
                            style: monoLabel(
                              size: 9.5,
                              color: statusColor,
                              letterSpacing: 0.6,
                            ),
                          ),
                        ),
                      ],
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 8),
              _RateValue(
                rateMbps: rateMbps,
                color: rateMbps == null ? AppColors.textFaint : metricColor,
              ),
              if (trailing != null) trailing!,
            ],
          ),
          const SizedBox(height: 14),
          Row(
            children: [
              Expanded(
                child: _Metric(
                  value: rateMbps == null ? '—' : rateMbps!.toStringAsFixed(1),
                  label: 'MB/S',
                  color: rateMbps == null ? AppColors.textFaint : metricColor,
                ),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: _Metric(value: '$sharedCount', label: 'SHARED'),
              ),
              const SizedBox(width: 8),
              Expanded(
                child: _Metric(
                  value: _gigabytes(totalBytes),
                  label: 'GB TOTAL',
                  color: totalBytes == null ? AppColors.textFaint : metricColor,
                ),
              ),
            ],
          ),
          const SizedBox(height: 11),
          Text(
            subtitle,
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            style: AppText.secondary(color: subtitleColor),
          ),
        ],
      ),
    );
  }
}

/// One anonymous swarm peer's card. Ember marks it as network-only: the
/// address has no signed identity behind it.
class TorrentPeerCard extends PersonCard {
  TorrentPeerCard({
    super.key,
    required ({
      String address,
      List<String> collections,
      DateTime lastSeen,
    }) entry,
  }) : super._(
          avatar: Container(
            width: 44,
            height: 44,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: AppColors.emberWash,
              borderRadius: BorderRadius.circular(44 * 0.34),
            ),
            child: const Icon(Icons.hub_outlined,
                size: 20, color: AppColors.ember),
          ),
          title: entry.address,
          subtitle: '${entry.collections.join(' · ')} · '
              '${formatLastSeen(entry.lastSeen)}',
          totalBytes: null,
          sharedCount: entry.collections.length,
          rateMbps: null,
          status: 'Torrent peer',
          statusColor: AppColors.ember,
          metricColor: AppColors.ember,
          subtitleColor: AppColors.ember,
          trailing: IconButton(
            tooltip: 'Forget peer',
            icon: const Icon(Icons.close, size: 16),
            color: AppColors.textDim,
            onPressed: () => AppControllers.collections.forgetPeer(entry.address),
          ),
        );
}

class _RateValue extends StatelessWidget {
  const _RateValue({required this.rateMbps, required this.color});

  final double? rateMbps;
  final Color color;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.end,
        mainAxisSize: MainAxisSize.min,
        children: [
          Text(
            rateMbps == null ? '—' : rateMbps!.toStringAsFixed(1),
            style: TextStyle(
              fontFamily: AppFonts.display,
              fontSize: 17,
              fontWeight: FontWeight.w600,
              color: color,
            ),
          ),
          Text(
            'MB/S',
            style: monoLabel(size: 9, color: AppColors.signalMuted),
          ),
        ],
      );
}

class _Metric extends StatelessWidget {
  const _Metric({
    required this.value,
    required this.label,
    this.color = AppColors.signal,
  });

  final String value;
  final String label;
  final Color color;

  @override
  Widget build(BuildContext context) => Container(
        height: 50,
        alignment: Alignment.center,
        decoration: BoxDecoration(
          color: AppColors.surfaceRaised,
          borderRadius: BorderRadius.circular(AppRadius.inner),
        ),
        child: Column(
          mainAxisAlignment: MainAxisAlignment.center,
          children: [
            Text(
              value,
              style: TextStyle(
                fontFamily: AppFonts.display,
                fontSize: 16,
                fontWeight: FontWeight.w600,
                color: color,
              ),
            ),
            const SizedBox(height: 2),
            Text(
              label,
              style: monoLabel(
                size: 9,
                color: AppColors.signalMuted,
                letterSpacing: 0.2,
              ),
            ),
          ],
        ),
      );
}

String _gigabytes(int? bytes) =>
    bytes == null ? '—' : (bytes / 1000000000).toStringAsFixed(1);

String _statusLabel({
  required double uploadMbps,
  required double downloadMbps,
  required bool isSharing,
}) {
  if (uploadMbps > 0 && downloadMbps > 0) return 'SENDING · RECEIVING';
  if (uploadMbps > 0) return 'SENDING';
  if (downloadMbps > 0) return 'RECEIVING';
  if (isSharing) return 'SHARING';
  return 'IDLE';
}
