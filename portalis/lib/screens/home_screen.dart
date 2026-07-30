import 'package:flutter/material.dart';
import '../bridge_generated/device.dart' as bridge;
import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'add_torrent_screen.dart';
import 'collection_screen.dart';
import 'join_collection_screen.dart';
import 'settings_screen.dart';
import 'share_screen.dart';
import 'user_screen.dart';

/// Home, per the "Portalis Add Flow" design: header, the collections list
/// (or an empty state), and three full-width actions — Share something /
/// Join a collection / Torrent — each its own screen instead of the old
/// single combined "Add" screen.
class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  void _push(BuildContext context, Widget screen) {
    Navigator.of(context).push(MaterialPageRoute(builder: (_) => screen));
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 14),
          child: Row(
            children: [
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const Text(
                      'Portalis',
                      style: TextStyle(
                        fontSize: 26,
                        fontWeight: FontWeight.w500,
                        letterSpacing: -0.3,
                      ),
                    ),
                    Text(
                      'Your collections',
                      style: TextStyle(
                        fontSize: 12.5,
                        color: AppColors.neutral400,
                      ),
                    ),
                  ],
                ),
              ),
              InkWell(
                customBorder: const CircleBorder(),
                onTap: () => _push(context, const SettingsScreen()),
                child: const Padding(
                  padding: EdgeInsets.all(8),
                  child: Icon(Icons.settings_outlined,
                      size: 21, color: AppColors.neutral400),
                ),
              ),
              const SizedBox(width: 6),
              InkWell(
                key: const Key('userAvatarButton'),
                customBorder: const CircleBorder(),
                onTap: () => _push(context, const UserScreen()),
                child: const _UserAvatar(),
              ),
            ],
          ),
        ),
        Expanded(
          child: ListenableBuilder(
            listenable: Collections.instance,
            builder: (context, _) {
              final collections = Collections.instance.collections;
              final error = Collections.instance.lastError;
              if (collections.isEmpty) {
                // A backend that failed to answer must not look identical to
                // a backend that answered "nothing" — that ambiguity is what
                // made earlier failures so hard to spot.
                return error != null
                    ? _CollectionsError(message: error)
                    : const _EmptyCollections();
              }
              return ListView.separated(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                itemCount: collections.length,
                separatorBuilder: (_, __) => const SizedBox(height: 10),
                itemBuilder: (context, index) {
                  final c = collections[index];
                  return _CollectionCard(
                    collection: c,
                    onTap: () => _push(context, CollectionScreen(collection: c)),
                  );
                },
              );
            },
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 18),
          child: Column(
            children: [
              _ActionButton(
                label: 'Share something',
                icon: Icons.upload_outlined,
                primary: true,
                onTap: () => _push(context, const ShareScreen()),
              ),
              const SizedBox(height: 10),
              _ActionButton(
                label: 'Join a collection',
                icon: Icons.link,
                onTap: () => _push(context, const JoinCollectionScreen()),
              ),
              const SizedBox(height: 10),
              _ActionButton(
                label: 'Torrent',
                icon: Icons.download_outlined,
                onTap: () => _push(context, const AddTorrentScreen()),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

/// Home's avatar button, showing the initial of the device's *real*
/// persisted nickname (`device.rs`'s identity) rather than a fixed letter.
/// Falls back to a neutral glyph until it loads, or if the backend isn't
/// available (widget tests) — never to an invented name.
class _UserAvatar extends StatefulWidget {
  const _UserAvatar();

  @override
  State<_UserAvatar> createState() => _UserAvatarState();
}

class _UserAvatarState extends State<_UserAvatar> {
  String _initials = '·';

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final identity = await bridge.deviceIdentity();
      final nickname = identity.nickname;
      if (mounted && nickname.isNotEmpty) {
        setState(() => _initials = nickname[0].toUpperCase());
      }
    } catch (_) {
      // Backend unavailable — keep the neutral placeholder.
    }
  }

  @override
  Widget build(BuildContext context) => Avatar(initials: _initials, size: 36);
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
            const Icon(Icons.error_outline, size: 40, color: Color(0xFFEB5757)),
            const SizedBox(height: 14),
            const Text(
              'Couldn\'t load your collections.',
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 13.5, height: 1.5),
            ),
            const SizedBox(height: 8),
            Text(
              message,
              textAlign: TextAlign.center,
              style: const TextStyle(
                fontSize: 10.5,
                height: 1.4,
                fontFamily: 'monospace',
                color: AppColors.neutral400,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _EmptyCollections extends StatelessWidget {
  const _EmptyCollections();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 48),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.hub_outlined, size: 44, color: AppColors.neutral500),
            const SizedBox(height: 14),
            Text(
              'Nothing here yet. Share files of your own, join a collection, or add a torrent.',
              textAlign: TextAlign.center,
              style: TextStyle(
                fontSize: 13.5,
                height: 1.5,
                color: AppColors.neutral400,
              ),
            ),
          ],
        ),
      ),
    );
  }
}

/// List-style collection card: kicker (what this is) + percent, name,
/// meta line, and a thin progress bar — replaces the old thumbnail grid
/// per the Add Flow design.
class _CollectionCard extends StatelessWidget {
  const _CollectionCard({required this.collection, required this.onTap});

  final Collection collection;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    final pct = (collection.progress * 100).clamp(0, 100).round();
    // State comes from Rust so both kinds of collection describe themselves
    // the same way — and so "seeding" accounts for unfetched manifest entries,
    // not just downloaded bytes.
    final kicker = collection.state.toUpperCase();
    final meta = collection.isShared
        ? '${collection.subtitle} · ${collection.collaborators.length} collaborator'
            '${collection.collaborators.length == 1 ? '' : 's'} · ${collection.peersLabel}'
        : '${collection.subtitle} · ${collection.peersLabel}';

    return Material(
      color: AppColors.surface,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onTap,
        child: Container(
          padding: const EdgeInsets.all(14),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(12),
            border: Border.all(color: AppColors.border),
          ),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Row(
                children: [
                  Text(
                    kicker,
                    style: const TextStyle(
                      fontSize: 10,
                      fontFamily: 'monospace',
                      letterSpacing: 1.1,
                      color: AppColors.accent300,
                    ),
                  ),
                  const Spacer(),
                  Text(
                    '$pct%',
                    style: const TextStyle(
                      fontSize: 11,
                      color: AppColors.neutral400,
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 5),
              Text(
                collection.name,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
              ),
              const SizedBox(height: 3),
              Text(
                meta,
                overflow: TextOverflow.ellipsis,
                style: const TextStyle(fontSize: 11.5, color: AppColors.neutral400),
              ),
              const SizedBox(height: 9),
              ClipRRect(
                borderRadius: BorderRadius.circular(2),
                child: LinearProgressIndicator(
                  value: collection.progress.clamp(0.0, 1.0),
                  minHeight: 3,
                  backgroundColor: AppColors.borderStrong,
                  valueColor: AlwaysStoppedAnimation(collection.hue),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// Full-width Add-flow action button: leading icon, label, trailing
/// chevron. [primary] gets the filled accent treatment.
class _ActionButton extends StatelessWidget {
  const _ActionButton({
    required this.label,
    required this.icon,
    required this.onTap,
    this.primary = false,
  });

  final String label;
  final IconData icon;
  final VoidCallback onTap;
  final bool primary;

  @override
  Widget build(BuildContext context) {
    final fg = primary ? AppColors.bg : AppColors.text;
    return Material(
      color: primary ? AppColors.accent : AppColors.surface,
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onTap,
        child: Container(
          height: 52,
          padding: const EdgeInsets.symmetric(horizontal: 18),
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(14),
            border: primary ? null : Border.all(color: AppColors.borderStrong),
          ),
          child: Row(
            children: [
              Icon(icon, size: 19, color: fg),
              const SizedBox(width: 12),
              Expanded(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    fontSize: 15.5,
                    fontWeight: FontWeight.w500,
                    color: fg,
                  ),
                ),
              ),
              Icon(Icons.chevron_right, size: 18, color: fg.withValues(alpha: 0.65)),
            ],
          ),
        ),
      ),
    );
  }
}
