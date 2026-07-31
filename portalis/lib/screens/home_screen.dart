import 'package:flutter/material.dart';

import '../bridge_generated/device.dart' as bridge;
import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'add_torrent_screen.dart';
import 'collection_screen.dart';
import 'join_collection_screen.dart';
import 'share_screen.dart';

/// Collections — the app's first destination.
///
/// The transfer itself is promoted to the top: when something is genuinely
/// moving, a live card sits above the list. When nothing is, that card is
/// *absent* rather than dormant, so mint on this screen always means motion.
class HomeScreen extends StatefulWidget {
  const HomeScreen({super.key});

  @override
  State<HomeScreen> createState() => _HomeScreenState();
}

/// All / Sharing / Receiving, derived from `state`.
enum _Filter { all, sharing, receiving }

class _HomeScreenState extends State<HomeScreen> {
  _Filter _filter = _Filter.all;

  void _push(Widget screen) {
    Navigator.of(context).push(MaterialPageRoute(builder: (_) => screen));
  }

  List<Collection> _apply(List<Collection> all) {
    switch (_filter) {
      case _Filter.all:
        return all;
      case _Filter.sharing:
        return all.where((c) => c.state == 'seeding').toList();
      case _Filter.receiving:
        return all.where((c) => c.state == 'downloading').toList();
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) {
        final all = Collections.instance.collections;
        final error = Collections.instance.lastError;

        if (all.isEmpty) {
          // A backend that failed to answer must not look identical to one
          // that answered "nothing" — that ambiguity is what made earlier
          // failures so hard to spot on device.
          return error != null
              ? _CollectionsError(message: error)
              : _FirstRun(onPush: _push);
        }

        // The single most active transfer gets the hero treatment. Picking
        // one rather than listing all keeps the top of the screen answering
        // "what is happening right now" in one glance.
        final moving = all
            .where((c) => c.downloadMbps > 0 || c.uploadMbps > 0)
            .toList()
          ..sort((a, b) => (b.downloadMbps + b.uploadMbps)
              .compareTo(a.downloadMbps + a.uploadMbps));
        final hero = moving.isEmpty ? null : moving.first;
        final shown = _apply(all);

        return Stack(
          children: [
            PageBody(
              child: CustomScrollView(
                slivers: [
                  SliverToBoxAdapter(child: const _Header()),
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            hero == null ? 'Your collections' : 'Moving now',
                            style: displayText(size: 30, height: 1.1),
                          ),
                          const SizedBox(height: 5),
                          Text(
                            _summary(all, moving.length),
                            style: const TextStyle(
                                fontSize: 14, color: AppColors.textDim),
                          ),
                        ],
                      ),
                    ),
                  ),
                  if (hero != null)
                    SliverToBoxAdapter(
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                        child: LiveTransferCard(
                          collection: hero,
                          onTap: () =>
                              _push(CollectionScreen(collection: hero)),
                        ),
                      ),
                    ),
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(22, 22, 22, 0),
                      child: FilterChips(
                        labels: const ['All', 'Sharing', 'Receiving'],
                        selected: _Filter.values.indexOf(_filter),
                        onSelected: (i) =>
                            setState(() => _filter = _Filter.values[i]),
                      ),
                    ),
                  ),
                  if (shown.isEmpty)
                    SliverToBoxAdapter(
                      child: Padding(
                        padding: const EdgeInsets.fromLTRB(22, 40, 22, 0),
                        child: Center(
                          child: Text(
                            _filter == _Filter.sharing
                                ? 'Nothing is being shared right now.'
                                : 'Nothing is being received right now.',
                            style: const TextStyle(
                                fontSize: 13, color: AppColors.textDim),
                          ),
                        ),
                      ),
                    )
                  else
                    SliverPadding(
                      padding: const EdgeInsets.fromLTRB(22, 16, 22, 0),
                      sliver: SliverList.separated(
                        itemCount: shown.length,
                        separatorBuilder: (_, __) => const SizedBox(height: 10),
                        itemBuilder: (context, i) => CollectionRow(
                          collection: shown[i],
                          onTap: () =>
                              _push(CollectionScreen(collection: shown[i])),
                        ),
                      ),
                    ),
                  // Clearance so the FAB never covers the last row.
                  const SliverToBoxAdapter(child: SizedBox(height: 96)),
                ],
              ),
            ),
            Positioned(
              right: 20,
              bottom: 20,
              child: _AddFab(onPush: _push),
            ),
          ],
        );
      },
    );
  }

  String _summary(List<Collection> all, int movingCount) {
    final c = '${all.length} collection${all.length == 1 ? '' : 's'}';
    if (movingCount == 0) return c;
    return '$c · $movingCount transfer${movingCount == 1 ? '' : 's'} in flight';
  }
}

/// The one unmistakable primary action on this screen.
class _AddFab extends StatelessWidget {
  const _AddFab({required this.onPush});

  final void Function(Widget) onPush;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.signal,
      borderRadius: BorderRadius.circular(22),
      child: InkWell(
        key: const Key('addFab'),
        borderRadius: BorderRadius.circular(22),
        onTap: () => _showSheet(context),
        child: const SizedBox(
          width: 62,
          height: 62,
          child: Icon(Icons.add, size: 26, color: AppColors.onSignal),
        ),
      ),
    );
  }

  void _showSheet(BuildContext context) {
    showModalBottomSheet<void>(
      context: context,
      backgroundColor: AppColors.surface,
      shape: const RoundedRectangleBorder(
        borderRadius: BorderRadius.vertical(top: Radius.circular(24)),
      ),
      builder: (sheetContext) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const SizedBox(height: 8),
            _sheetItem(
              sheetContext,
              Icons.arrow_upward,
              AppColors.signal,
              'Share something',
              'Seed files from this device',
              const ShareScreen(),
            ),
            _sheetItem(
              sheetContext,
              Icons.link,
              AppColors.signal,
              'Join with a key',
              'Paste an invite you were sent',
              const JoinCollectionScreen(),
            ),
            _sheetItem(
              sheetContext,
              Icons.download_outlined,
              AppColors.ember,
              'Add a torrent',
              'Magnet link or .torrent file',
              const AddTorrentScreen(),
            ),
            const SizedBox(height: 8),
          ],
        ),
      ),
    );
  }

  Widget _sheetItem(BuildContext sheetContext, IconData icon, Color color,
      String title, String subtitle, Widget screen) {
    return ListTile(
      leading: Icon(icon, color: color),
      title: Text(title,
          style: const TextStyle(fontSize: 15, fontWeight: FontWeight.w600)),
      subtitle: Text(subtitle,
          style: const TextStyle(fontSize: 12, color: AppColors.textDim)),
      onTap: () {
        Navigator.of(sheetContext).pop();
        onPush(screen);
      },
    );
  }
}

/// Home's top bar: identity, app name, and a live peer count when there is
/// one to report.
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
                  '${_size(collection.totalBytes)}',
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

/// First run — no collections at all.
class _FirstRun extends StatelessWidget {
  const _FirstRun({required this.onPush});

  final void Function(Widget) onPush;

  @override
  Widget build(BuildContext context) {
    return PageBody(
      child: Column(
        children: [
          Expanded(
            child: Center(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 34),
                child: Column(
                  mainAxisSize: MainAxisSize.min,
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
                          // than at the source's 1254², so the full-resolution
                          // bitmap never enters the image cache for a 72pt
                          // slot. 3x covers the densest screen we target.
                          cacheWidth: 216,
                          cacheHeight: 216,
                          filterQuality: FilterQuality.medium,
                        ),
                      ),
                    ),
                    const SizedBox(height: 26),
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
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(22, 0, 22, 26),
            child: Column(
              children: [
                PrimaryAction(
                  key: const Key('shareSomethingButton'),
                  label: 'Share something',
                  icon: Icons.arrow_upward,
                  onTap: () => onPush(const ShareScreen()),
                ),
                const SizedBox(height: 10),
                Row(
                  children: [
                    Expanded(
                      child: _SecondaryCard(
                        icon: Icons.link,
                        iconColor: AppColors.signal,
                        label: 'Join with a key',
                        onTap: () => onPush(const JoinCollectionScreen()),
                      ),
                    ),
                    const SizedBox(width: 10),
                    Expanded(
                      child: _SecondaryCard(
                        icon: Icons.download_outlined,
                        iconColor: AppColors.ember,
                        label: 'Add a torrent',
                        onTap: () => onPush(const AddTorrentScreen()),
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 14),
                Text(
                  'NO ACCOUNT · NOTHING LEAVES THIS DEVICE UNASKED',
                  textAlign: TextAlign.center,
                  style: monoLabel(size: 10.5, color: AppColors.textGhost),
                ),
              ],
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

/// Shown instead of the empty state when the backend itself failed, so the
/// two are distinguishable. The raw message is included deliberately — it's
/// the only place a Rust-side error reaches the user.
class _CollectionsError extends StatelessWidget {
  const _CollectionsError({required this.message});

  final String message;

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 32),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.error_outline, size: 40, color: AppColors.danger),
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
