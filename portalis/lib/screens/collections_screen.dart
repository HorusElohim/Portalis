import 'package:flutter/material.dart';

import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'add_torrent_screen.dart';
import 'collection_screen.dart';
import 'join_collection_screen.dart';
import 'share_screen.dart';

/// Collections — the full list, with filters.
///
/// Split out of Home so the first destination can be a welcome rather than a
/// list: the two answer different questions ("what can I do?" versus "what do
/// I have?") and were previously crammed into one screen that changed shape
/// depending on whether you owned anything.
class CollectionsScreen extends StatefulWidget {
  const CollectionsScreen({super.key});

  @override
  State<CollectionsScreen> createState() => _CollectionsScreenState();
}

/// All / Sharing / Receiving, derived from `state`.
enum _Filter { all, sharing, receiving }

class _CollectionsScreenState extends State<CollectionsScreen> {
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
              ? CollectionsErrorState(message: error)
              : const _NoCollectionsYet();
        }

        final shown = _apply(all);
        final down = all.fold<double>(0, (s, c) => s + c.downloadMbps);
        final up = all.fold<double>(0, (s, c) => s + c.uploadMbps);
        return Stack(
          children: [
            PageBody(
              child: CustomScrollView(
                slivers: [
                  SliverToBoxAdapter(
                    child: Padding(
                      padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          const CanvasTitle('Collections', size: 34),
                          const SizedBox(height: 5),
                          // The aggregate Transfers used to carry. Stating it
                          // here is what let that destination go: it was the
                          // one fact this screen didn't already have.
                          if (Collections.instance.liveRate > 0)
                            Text(
                              '${plural(all.where((c) => c.isMoving).length, 'transfer')}'
                              ' · ↓ ${formatRate(down)} · ↑ ${formatRate(up)}',
                              overflow: TextOverflow.ellipsis,
                              style: monoLabel(
                                  size: 12,
                                  color: AppColors.signal,
                                  letterSpacing: 0.2),
                            )
                          else
                            Text(
                              plural(all.length, 'collection'),
                              style: const TextStyle(
                                  fontSize: 14, color: AppColors.textDim),
                            ),
                        ],
                      ),
                    ),
                  ),
                  if (!Collections.instance.engineReady)
                    const SliverToBoxAdapter(child: EngineStartingNotice()),
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
            Positioned(right: 20, bottom: 20, child: AddFab(onPush: _push)),
          ],
        );
      },
    );
  }
}

/// Collections tab with nothing in it. Deliberately terse — the welcome copy
/// and the primary actions live on Home, and repeating them here would make
/// the two destinations look like the same screen.
class _NoCollectionsYet extends StatelessWidget {
  const _NoCollectionsYet();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 44),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.dashboard_outlined,
                size: 40, color: AppColors.textGhost),
            const SizedBox(height: 14),
            Text('No collections yet.',
                style: displayText(size: 17, color: AppColors.textDim)),
            const SizedBox(height: 6),
            const Text(
              'Share something or join with a key from Home.',
              textAlign: TextAlign.center,
              style: TextStyle(
                  fontSize: 13, height: 1.5, color: AppColors.textGhost),
            ),
          ],
        ),
      ),
    );
  }
}

/// The one unmistakable primary action on this screen.
class AddFab extends StatelessWidget {
  const AddFab({super.key, required this.onPush});

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