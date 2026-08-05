import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../application/collections_controller.dart';
import '../domain/collection.dart';
import 'collection_presentation.dart';
import 'collection_views.dart';
import 'welcome.dart';
import '../../identity/application/identity_controller.dart';

/// Identity and peer summary for the compact Home layout.
class HomeHeader extends StatelessWidget {
  const HomeHeader({
    super.key,
    required this.identity,
    required this.collections,
  });

  final IdentityController identity;
  final CollectionsController collections;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: identity,
        builder: (context, _) {
          final nickname = identity.info?.nickname;
          final initials = nickname == null || nickname.isEmpty
              ? '·'
              : nickname[0].toUpperCase();
          final peers = collections.collections.fold<int>(
            0,
            (sum, collection) => sum + collection.livePeers,
          );
          return Padding(
            padding: const EdgeInsets.fromLTRB(22, 14, 22, 0),
            child: Row(
              children: [
                Avatar(initials: initials, size: 34, primary: true),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Portalis', style: displayText(size: 16)),
                      if (peers > 0)
                        Text(
                          '$peers PEER${peers == 1 ? '' : 'S'} CONNECTED',
                          style: monoLabel(size: 10, color: AppColors.signal),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          );
        },
      );
}

/// Secondary action beside the primary share action.
class AddTorrentAction extends StatelessWidget {
  const AddTorrentAction({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => Tooltip(
        message: 'Add a torrent',
        child: Material(
          color: AppColors.surface,
          borderRadius: BorderRadius.circular(AppRadius.control),
          child: InkWell(
            key: const Key('addTorrentButton'),
            borderRadius: BorderRadius.circular(AppRadius.control),
            onTap: onTap,
            child: Container(
              width: 46,
              height: 46,
              alignment: Alignment.center,
              decoration: BoxDecoration(
                borderRadius: BorderRadius.circular(AppRadius.control),
                border: Border.all(color: AppColors.border),
              ),
              child: const Icon(
                Icons.download_outlined,
                size: 18,
                color: AppColors.ember,
              ),
            ),
          ),
        ),
      );
}

/// The wide-layout collection list. The caller supplies the optional detail
/// so this component does not depend on a navigation strategy.
class CollectionsList extends StatelessWidget {
  const CollectionsList({
    super.key,
    required this.collections,
    required this.openId,
    required this.onOpen,
    required this.detailFor,
  });

  final List<Collection> collections;
  final String? openId;
  final ValueChanged<Collection> onOpen;
  final Widget? Function(Collection collection) detailFor;

  @override
  Widget build(BuildContext context) => ListView.separated(
        padding: const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 28),
        itemCount: collections.length,
        separatorBuilder: (_, __) => const SizedBox(height: 10),
        itemBuilder: (context, index) {
          final collection = collections[index];
          final isOpen = collection.id == openId;
          return CollectionRow(
            collection: collection,
            selected: isOpen,
            onTap: () => onOpen(collection),
            detail: isOpen ? detailFor(collection) : null,
          );
        },
      );
}

/// The wide-layout empty state. Compact Home embeds the same [Welcome].
class EmptyCollectionsWelcome extends StatelessWidget {
  const EmptyCollectionsWelcome({super.key});

  @override
  Widget build(BuildContext context) => Center(
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 40),
          child: Column(
            mainAxisSize: MainAxisSize.min,
            children: [
              const Welcome(titleSize: 34),
              const SizedBox(height: 26),
              Text(
                'NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED',
                textAlign: TextAlign.center,
                style: monoLabel(size: 10.5, color: AppColors.textGhost),
              ),
            ],
          ),
        ),
      );
}

/// The compact-layout hero for the collection currently moving bytes.
class LiveTransferCard extends StatelessWidget {
  const LiveTransferCard({
    super.key,
    required this.collection,
    required this.onTap,
  });

  final Collection collection;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final torrent = !collection.isShared;
    final accent = torrent ? AppColors.ember : AppColors.signal;
    final receiving = collection.downloadMbps >= collection.uploadMbps;
    final rate = receiving ? collection.downloadMbps : collection.uploadMbps;

    return SurfaceCard(
      onTap: onTap,
      radius: AppRadius.card,
      padding: const EdgeInsets.all(18),
      glow: collection.glow,
      glowColor: accent,
      glowIntensity: collection.liveIntensity,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        LiveDot(color: accent, size: 7),
                        const SizedBox(width: 7),
                        Text(
                          receiving ? 'RECEIVING' : 'SENDING',
                          style: monoLabel(size: 10, color: accent),
                        ),
                      ],
                    ),
                    const SizedBox(height: 5),
                    Text(
                      collection.name,
                      overflow: TextOverflow.ellipsis,
                      style: displayText(size: 19),
                    ),
                  ],
                ),
              ),
              const SizedBox(width: 12),
              Column(
                crossAxisAlignment: CrossAxisAlignment.end,
                children: [
                  Text(
                    rate.toStringAsFixed(1),
                    style: displayText(size: 20, color: accent),
                  ),
                  Text(
                    'MB/S',
                    style: monoLabel(size: 10, color: AppColors.signalMuted),
                  ),
                ],
              ),
            ],
          ),
          const SizedBox(height: 16),
          ClipRRect(
            borderRadius: BorderRadius.circular(AppRadius.pill),
            child: LinearProgressIndicator(
              value: collection.progress.clamp(0.0, 1.0),
              minHeight: 8,
              backgroundColor: AppColors.borderStrong,
              valueColor: AlwaysStoppedAnimation(accent),
            ),
          ),
          const SizedBox(height: 11),
          Row(
            children: [
              Expanded(
                child: Text(
                  '${_size(collection.downloadedBytes)} / '
                  '${_size(collection.totalBytes)}'
                  '${collection.etaLabel == null ? '' : ' · ${collection.etaLabel}'}',
                  overflow: TextOverflow.ellipsis,
                  style: monoLabel(
                    size: 11,
                    color: AppColors.textDim,
                    letterSpacing: 0.2,
                  ),
                ),
              ),
              if (collection.collaborators.isNotEmpty) ...[
                _AvatarStack(collaborators: collection.collaborators),
                const SizedBox(width: 7),
              ],
              Text(
                collection.peersLabel,
                style: monoLabel(
                  size: 11,
                  color: AppColors.textDim,
                  letterSpacing: 0.2,
                ),
              ),
            ],
          ),
        ],
      ),
    );
  }

  static String _size(int bytes) {
    const gb = 1000000000;
    const mb = 1000000;
    if (bytes >= gb) return '${(bytes / gb).toStringAsFixed(1)} GB';
    return '${(bytes / mb).toStringAsFixed(0)} MB';
  }
}

class _AvatarStack extends StatelessWidget {
  const _AvatarStack({required this.collaborators});

  final List<Collaborator> collaborators;

  @override
  Widget build(BuildContext context) {
    final shown = collaborators.take(3).toList();
    return SizedBox(
      width: 16.0 + (shown.length - 1) * 11,
      height: 16,
      child: Stack(
        children: [
          for (var index = 0; index < shown.length; index++)
            Positioned(
              left: index * 11.0,
              child: Container(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(
                    color: AppColors.surfaceDeep,
                    width: 1.5,
                  ),
                ),
                child: Avatar(initials: shown[index].initials, size: 16),
              ),
            ),
        ],
      ),
    );
  }
}
