import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/collection.dart';
import '../theme.dart';

/// Distinct collaborators across every collection, and where they appear.
/// Derived — there is no peer directory in the backend.
///
/// A first-class destination on both layouts — a bottom tab on mobile, a
/// header button in the wide layout — after
/// two rounds of being reached only indirectly. The User tab still carries the
/// same collaborator count as a shortcut, but selects this same screen rather
/// than pushing a second copy of it. Same content and the same grid either
/// way — on a phone-width window it simply resolves to one column.
class PeopleScreen extends StatelessWidget {
  const PeopleScreen({super.key, this.embedded = false});

  /// Set whenever a parent already supplies a Scaffold, SafeArea and back
  /// button — a desktop shell pane, or this screen's own mobile tab of
  /// [RootShell] — so this doesn't draw a second set, or a back button with
  /// nowhere to go.
  final bool embedded;

  @override
  Widget build(BuildContext context) {
    // Home and Settings both read Collections through a ListenableBuilder;
    // this screen sat under a const [...] tab list in RootShell without one,
    // so a background poll landing while People wasn't the visible tab could
    // leave it showing whatever was true the first time it built, right up
    // until something else forced this position in the tree to rebuild.
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final byDevice = <String, ({Collaborator who, List<String> collections})>{};
    for (final c in AppControllers.collections.collections) {
      for (final p in c.collaborators) {
        final entry = byDevice[p.deviceId];
        if (entry == null) {
          byDevice[p.deviceId] = (who: p, collections: [c.name]);
        } else {
          entry.collections.add(c.name);
        }
      }
    }
    final people = byDevice.values.toList();

    // Anonymous swarm peers across every collection. They remain separate
    // from signed collaborators: an address is a connection, not an identity.
    final byAddress = <String,
        ({String address, List<String> collections, DateTime lastSeen})>{};
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
        if (peer.lastSeen.isAfter(entry.lastSeen)) {
          byAddress[peer.address] = (
            address: entry.address,
            collections: entry.collections,
            lastSeen: peer.lastSeen,
          );
        }
      }
    }
    final torrentPeople = byAddress.values.toList();

    final cards = [
      for (final entry in people) PersonCard(entry: entry),
      for (final entry in torrentPeople) TorrentPeerCard(entry: entry),
    ];

    return AppScreen(
      title: 'People',
      subtitle: cards.isEmpty ? null : Text(_subtitle(people.length, torrentPeople.length)),
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
          // A grid, not a single stretched column: on desktop the centre
          // pane is wide enough for several across, and on a phone-width
          // window this resolves to one anyway.
          : WindowBuilder(
              builder: (context, window) => GridView.builder(
                padding: const EdgeInsets.fromLTRB(
                    kScreenGutter, 0, kScreenGutter, 28),
                gridDelegate: SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: window.columns(340),
                  mainAxisSpacing: 10,
                  crossAxisSpacing: 10,
                  mainAxisExtent: 76,
                ),
                itemCount: cards.length,
                itemBuilder: (context, i) => cards[i],
              ),
            ),
    );
  }

  String _subtitle(int collaborators, int torrentPeers) {
    final parts = [
      if (collaborators > 0) '$collaborators collaborator${collaborators == 1 ? '' : 's'}',
      if (torrentPeers > 0) '$torrentPeers torrent peer${torrentPeers == 1 ? '' : 's'}',
    ];
    return '${parts.join(', ')} across your collections';
  }
}

/// One row of a person's identity: an avatar, a title, a subtitle, and an
/// optional trailing action. A named collaborator ([PersonCard]'s own
/// factory) and an anonymous swarm peer ([TorrentPeerCard]) are both this
/// same row, just filled in differently — so the row itself lives here once
/// instead of being redrawn per kind.
class PersonCard extends StatelessWidget {
  const PersonCard._({
    super.key,
    required this.avatar,
    required this.title,
    required this.subtitle,
    this.subtitleColor = AppColors.textFaint,
    this.trailing,
  });

  /// One collaborator's card — avatar, name and role, and which collections
  /// they share with this device.
  factory PersonCard({
    Key? key,
    required ({Collaborator who, List<String> collections}) entry,
  }) {
    final who = entry.who;
    return PersonCard._(
      key: key,
      avatar: Avatar(initials: who.initials, size: 34),
      title: who.isAdmin ? '${who.name} · admin' : who.name,
      subtitle: entry.collections.join(' · '),
    );
  }

  final Widget avatar;
  final String title;
  final String subtitle;
  final Color subtitleColor;
  final Widget? trailing;

  @override
  Widget build(BuildContext context) => SurfaceCard(
        child: Row(
          children: [
            avatar,
            const SizedBox(width: 12),
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                mainAxisSize: MainAxisSize.min,
                children: [
                  Text(title, overflow: TextOverflow.ellipsis, style: AppText.cardTitle()),
                  const SizedBox(height: 3),
                  Text(
                    subtitle,
                    maxLines: 1,
                    overflow: TextOverflow.ellipsis,
                    style: monoLabel(size: 10.5, color: subtitleColor, letterSpacing: 0.2),
                  ),
                ],
              ),
            ),
            if (trailing != null) trailing!,
          ],
        ),
      );
}

/// One anonymous swarm peer's card — an `ip:port`, not a name, when it was
/// last seen, and the torrent(s) it was seen on. Ember, matching every other
/// torrent-owned surface in the app, so it never reads as a signed-in
/// collaborator.
class TorrentPeerCard extends PersonCard {
  TorrentPeerCard({
    super.key,
    required ({String address, List<String> collections, DateTime lastSeen}) entry,
  }) : super._(
          avatar: Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: AppColors.emberWash,
              borderRadius: BorderRadius.circular(34 * 0.34),
            ),
            child: const Icon(Icons.hub_outlined, size: 18, color: AppColors.ember),
          ),
          title: entry.address,
          subtitle: 'Torrent peer · ${formatLastSeen(entry.lastSeen)} · '
              '${entry.collections.join(' · ')}',
          subtitleColor: AppColors.ember,
          trailing: IconButton(
            tooltip: 'Forget peer',
            icon: const Icon(Icons.close, size: 16),
            color: AppColors.textDim,
            onPressed: () => AppControllers.collections.forgetPeer(entry.address),
          ),
        );
}
