import 'package:flutter/material.dart';

import '../bridge_generated/device.dart' as bridge;
import '../models.dart';
import '../services/collections.dart';
import '../services/navigation.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'add_torrent_screen.dart';
import 'collection_screen.dart';
import 'join_collection_screen.dart';
import 'share_screen.dart';

/// Home — the welcome.
///
/// Always the same shape, whether you own nothing or fifty collections: what
/// Portalis is, and the three ways to start something. The list lives on its
/// own destination now, so this screen never has to be two things at once.
///
/// The one thing that does appear conditionally is a live transfer card,
/// because "something is moving right now" is the single fact worth
/// interrupting a welcome for.
class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  void _push(BuildContext context, Widget screen) {
    Navigator.of(context).push(MaterialPageRoute(builder: (_) => screen));
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) {
        final all = Collections.instance.collections;
        final error = Collections.instance.lastError;
        final moving = all
            .where((c) => c.downloadMbps > 0 || c.uploadMbps > 0)
            .toList()
          ..sort((a, b) => (b.downloadMbps + b.uploadMbps)
              .compareTo(a.downloadMbps + a.uploadMbps));
        final hero = moving.isEmpty ? null : moving.first;

        return PageBody(
          child: CustomScrollView(
            slivers: [
              const SliverToBoxAdapter(child: _Header()),
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(34, 30, 34, 0),
                  child: Column(
                    children: [
                      PulseRings(
                        size: 168,
                        child: ClipRRect(
                          borderRadius: BorderRadius.circular(20),
                          child: Image.asset(
                            'assets/PortalisNature.png',
                            width: 72,
                            height: 72,
                            // Decoded at roughly the size it is drawn rather
                            // than at the source's 1254², so the
                            // full-resolution bitmap never enters the image
                            // cache for a 72pt slot.
                            cacheWidth: 216,
                            cacheHeight: 216,
                            filterQuality: FilterQuality.medium,
                          ),
                        ),
                      ),
                      const SizedBox(height: 22),
                      Text(
                        'Send anything,\nstraight to a friend',
                        textAlign: TextAlign.center,
                        style: displayText(size: 28, height: 1.15),
                      ),
                      const SizedBox(height: 10),
                      const Text(
                        'No uploads, no size limits. Files move device to '
                        'device — and stay on yours.',
                        textAlign: TextAlign.center,
                        style: TextStyle(
                            fontSize: 14.5,
                            height: 1.5,
                            color: AppColors.textDim),
                      ),
                    ],
                  ),
                ),
              ),
              if (error != null)
                SliverToBoxAdapter(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(22, 22, 22, 0),
                    child: InfoBanner(
                      color: AppColors.danger,
                      icon: Icons.error_outline,
                      text: error,
                    ),
                  ),
                )
              else if (!Collections.instance.engineReady)
                const SliverToBoxAdapter(child: EngineStartingNotice()),
              if (hero != null)
                SliverToBoxAdapter(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                    child: LiveTransferCard(
                      collection: hero,
                      onTap: () =>
                          _push(context, CollectionScreen(collection: hero)),
                    ),
                  ),
                ),
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(22, 26, 22, 0),
                  child: Column(
                    children: [
                      PrimaryAction(
                        key: const Key('shareSomethingButton'),
                        label: 'Share something',
                        icon: Icons.arrow_upward,
                        onTap: () => _push(context, const ShareScreen()),
                      ),
                      const SizedBox(height: 10),
                      Row(
                        children: [
                          Expanded(
                            child: _SecondaryCard(
                              icon: Icons.link,
                              iconColor: AppColors.signal,
                              label: 'Join with a key',
                              onTap: () => _push(
                                  context, const JoinCollectionScreen()),
                            ),
                          ),
                          const SizedBox(width: 10),
                          Expanded(
                            child: _SecondaryCard(
                              icon: Icons.download_outlined,
                              iconColor: AppColors.ember,
                              label: 'Add a torrent',
                              onTap: () =>
                                  _push(context, const AddTorrentScreen()),
                            ),
                          ),
                        ],
                      ),
                    ],
                  ),
                ),
              ),
              if (all.isNotEmpty)
                SliverToBoxAdapter(
                  child: Padding(
                    padding: const EdgeInsets.fromLTRB(22, 14, 22, 0),
                    child: SurfaceCard(
                      padding: const EdgeInsets.symmetric(
                          horizontal: 16, vertical: 13),
                      // Switches destination rather than pushing a route:
                      // Collections is a peer of Home, not a child of it.
                      onTap: () => AppNavigation.tab.value = 1,
                      child: Row(
                        children: [
                          const Icon(Icons.dashboard_outlined,
                              size: 18, color: AppColors.textDim),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Text(
                              plural(all.length, 'collection'),
                              style: const TextStyle(
                                  fontSize: 14, fontWeight: FontWeight.w600),
                            ),
                          ),
                          const Icon(Icons.chevron_right,
                              size: 16, color: AppColors.textGhost),
                        ],
                      ),
                    ),
                  ),
                ),
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(22, 18, 22, 26),
                  child: Text(
                    'NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED',
                    textAlign: TextAlign.center,
                    style: monoLabel(size: 10.5, color: AppColors.textGhost),
                  ),
                ),
              ),
            ],
          ),
        );
      },
    );
  }
}

class _Header extends StatefulWidget {
  const _Header();

  @override
  State<_Header> createState() => _HeaderState();
}

class _HeaderState extends State<_Header> {
  String _initials = '·';

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final identity = await bridge.deviceIdentity();
      if (mounted && identity.nickname.isNotEmpty) {
        setState(() => _initials = identity.nickname[0].toUpperCase());
      }
    } catch (_) {
      // Backend unavailable — keep the neutral placeholder rather than
      // inventing a name.
    }
  }

  @override
  Widget build(BuildContext context) {
    final peers = Collections.instance.collections
        .fold<int>(0, (s, c) => s + c.livePeers);
    return Padding(
      padding: const EdgeInsets.fromLTRB(22, 14, 22, 0),
      child: Row(
        children: [
          Avatar(initials: _initials, size: 34, primary: true),
          const SizedBox(width: 10),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text('Portalis', style: displayText(size: 16)),
                // Only claimed when there genuinely are peers — there is no
                // device-discovery layer to report "nearby" from.
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
  }
}

/// The hero card for whatever is moving right now.
///
/// Only ever rendered when there is live throughput to report — see
/// [HomeScreen]. Every figure on it comes from the engine.
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
    // Direction is whichever side is actually carrying bytes.
    final receiving = collection.downloadMbps >= collection.uploadMbps;
    final rate = receiving ? collection.downloadMbps : collection.uploadMbps;

    return SurfaceCard(
      onTap: onTap,
      radius: 24,
      padding: const EdgeInsets.all(18),
      borderColor: accent.withValues(alpha: 0.28),
      gradient: LinearGradient(
        begin: Alignment.topLeft,
        end: Alignment.bottomRight,
        colors: [
          accent.withValues(alpha: 0.16),
          accent.withValues(alpha: 0.04),
        ],
      ),
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
                  Text('MB/S',
                      style:
                          monoLabel(size: 10, color: AppColors.signalMuted)),
                ],
              ),
            ],
          ),
          const SizedBox(height: 16),
          ClipRRect(
            borderRadius: BorderRadius.circular(99),
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
                      size: 11, color: AppColors.textDim, letterSpacing: 0.2),
                ),
              ),
              if (collection.collaborators.isNotEmpty) ...[
                _AvatarStack(collaborators: collection.collaborators),
                const SizedBox(width: 7),
              ],
              Text(
                collection.peersLabel,
                style: monoLabel(
                    size: 11, color: AppColors.textDim, letterSpacing: 0.2),
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
          for (var i = 0; i < shown.length; i++)
            Positioned(
              left: i * 11.0,
              child: Container(
                decoration: BoxDecoration(
                  shape: BoxShape.circle,
                  border: Border.all(color: AppColors.surfaceDeep, width: 1.5),
                ),
                child: Avatar(initials: shown[i].initials, size: 16),
              ),
            ),
        ],
      ),
    );
  }
}

class _SecondaryCard extends StatelessWidget {
  const _SecondaryCard({
    required this.icon,
    required this.iconColor,
    required this.label,
    required this.onTap,
  });

  final IconData icon;
  final Color iconColor;
  final String label;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      onTap: onTap,
      radius: 18,
      padding: const EdgeInsets.all(16),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Icon(icon, size: 19, color: iconColor),
          const SizedBox(height: 8),
          Text(
            label,
            style: const TextStyle(fontSize: 14.5, fontWeight: FontWeight.w600),
          ),
        ],
      ),
    );
  }
}
