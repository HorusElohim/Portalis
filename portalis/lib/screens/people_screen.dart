import 'package:flutter/material.dart';

import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';

/// Distinct collaborators across every collection, and where they appear.
/// Derived — there is no peer directory in the backend.
///
/// Reached two ways: a sidebar destination on desktop (see `DesktopShell`),
/// and a row on the You tab on mobile. Same content and the same grid either
/// way — on a phone-width window it simply resolves to one column.
class PeopleScreen extends StatelessWidget {
  const PeopleScreen({super.key, this.embedded = false});

  /// Set when this is a pane of the desktop shell rather than a pushed
  /// screen — hides the back button, since the sidebar is the only way in
  /// or out of it there.
  final bool embedded;

  @override
  Widget build(BuildContext context) {
    final byDevice = <String, ({Collaborator who, List<String> collections})>{};
    for (final c in Collections.instance.collections) {
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

    return AdaptiveScreen(
      embedded: embedded,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Padding(
            padding: EdgeInsets.fromLTRB(28, embedded ? 26 : 12, 28, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const ImpactTitle('People'),
                if (people.isNotEmpty) ...[
                  const SizedBox(height: 8),
                  Text(
                    '${people.length} collaborator'
                    '${people.length == 1 ? '' : 's'} across your '
                    'collections',
                    style: const TextStyle(
                        fontSize: 13.5, color: AppColors.textDim),
                  ),
                ],
              ],
            ),
          ),
          Expanded(
            child: people.isEmpty
                ? const Center(
                    child: Padding(
                      padding: EdgeInsets.symmetric(horizontal: 28),
                      child: Text(
                        'Nobody yet. Collaborators appear once you share '
                        'or join.',
                        textAlign: TextAlign.center,
                        style:
                            TextStyle(fontSize: 13, color: AppColors.textDim),
                      ),
                    ),
                  )
                // A grid, not a single stretched column: on desktop the
                // centre pane is wide enough for several across, and on a
                // phone-width window this resolves to one anyway.
                : LayoutBuilder(
                    builder: (context, constraints) {
                      final cols =
                          (constraints.maxWidth / 340).floor().clamp(1, 4);
                      return GridView.builder(
                        padding: const EdgeInsets.fromLTRB(28, 20, 28, 28),
                        gridDelegate:
                            SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: cols,
                          mainAxisSpacing: 10,
                          crossAxisSpacing: 10,
                          mainAxisExtent: 76,
                        ),
                        itemCount: people.length,
                        itemBuilder: (context, i) =>
                            PersonCard(entry: people[i]),
                      );
                    },
                  ),
          ),
        ],
      ),
    );
  }
}

/// One collaborator's card — avatar, name and role, and which collections
/// they share with this device.
class PersonCard extends StatelessWidget {
  const PersonCard({super.key, required this.entry});

  final ({Collaborator who, List<String> collections}) entry;

  @override
  Widget build(BuildContext context) {
    final who = entry.who;
    return SurfaceCard(
      child: Row(
        children: [
          Avatar(initials: who.initials, size: 34),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(
                  who.isAdmin ? '${who.name} · admin' : who.name,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 14, fontWeight: FontWeight.w600),
                ),
                const SizedBox(height: 3),
                Text(
                  entry.collections.join(' · '),
                  maxLines: 1,
                  overflow: TextOverflow.ellipsis,
                  style: monoLabel(size: 10.5, letterSpacing: 0.2),
                ),
              ],
            ),
          ),
        ],
      ),
    );
  }
}
