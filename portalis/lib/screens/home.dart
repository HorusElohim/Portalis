import 'dart:typed_data';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/collection.dart';
import '../features/collections/presentation/components.dart';
import '../theme.dart';
import '../features/collections/presentation/collection_detail.dart';
import '../features/collections/presentation/collection_join.dart';
import '../features/collections/presentation/collection_share.dart';
import '../features/collections/presentation/torrent_add.dart';

export '../features/collections/presentation/home_sections.dart'
    show LiveTransferCard;

/// Home — the one place to find, filter, and add a collection.
///
/// Replaces three screens that had drifted apart by platform:
/// `home_screen.dart` (mobile welcome + three buttons), `collections_screen.dart`
/// (mobile list + filter chips + FAB), and `desktop_collections_pane.dart`
/// (desktop omnibar + list). All three answered the same two questions —
/// "what do I have" and "how do I add something" — with different widgets
/// and, in places, different capabilities: only desktop had paste
/// recognition, only mobile had filter chips. This is the one answer to both
/// questions, on either layout.
///
/// [embedded] follows the convention every other screen in this app uses:
/// true when this is a pane of [DesktopShell] (which supplies its own
/// Scaffold/SafeArea and, on this screen only, a file-drop target), false
/// when it's a tab of the mobile shell.
///
/// [onOpen]/[onShare]/[onJoin] let the desktop shell keep driving its own
/// pane selection exactly as it did through `desktop_collections_pane.dart`;
/// left null (the mobile case) each falls back to pushing the destination
/// screen instead.
class Home extends StatefulWidget {
  const Home({
    super.key,
    this.embedded = false,
    this.openId,
    this.onOpen,
    this.onShare,
    this.onJoin,
  });

  final bool embedded;

  /// The collection whose card is showing its contents, if any. Desktop
  /// only — mobile pushes a route instead of expanding in place.
  final String? openId;

  final ValueChanged<String>? onOpen;

  /// Optionally carries files a drop already picked, so the desktop shell
  /// can show them on the share pane it selects rather than this widget
  /// pushing a route of its own.
  final void Function([List<PickedFile>? initialFiles])? onShare;

  /// An invite code the omnibar recognised.
  final ValueChanged<String>? onJoin;

  @override
  State<Home> createState() => _HomeState();
}

enum _Filter { all, sharing, receiving }

class _HomeState extends State<Home> {
  String _query = '';
  _Filter _filter = _Filter.all;
  bool _dropBusy = false;

  void _push(Widget screen) {
    Navigator.of(context).push(MaterialPageRoute(builder: (_) => screen));
  }

  void _openCollection(Collection c) {
    if (widget.onOpen != null) {
      widget.onOpen!(c.id);
    } else {
      _push(CollectionScreen(collection: c));
    }
  }

  void _openShare([List<PickedFile>? initialFiles]) {
    if (widget.onShare != null) {
      widget.onShare!(initialFiles);
    } else {
      _push(ShareScreen(initialFiles: initialFiles));
    }
  }

  void _openJoin(String code) {
    if (widget.onJoin != null) {
      widget.onJoin!(code);
    } else {
      _push(JoinCollectionScreen(initialCode: code));
    }
  }

  void _openAddTorrent() => _push(const AddTorrentScreen());

  /// Matches on the collection's name and on the files inside it, because
  /// "where is that photo" is the question a filter over a list of albums is
  /// actually being asked.
  bool _matchesQuery(Collection c) {
    if (_query.isEmpty) return true;
    final q = _query.toLowerCase();
    return c.name.toLowerCase().contains(q) ||
        c.media.any((m) => m.label.toLowerCase().contains(q));
  }

  bool _matchesFilter(Collection c) {
    switch (_filter) {
      case _Filter.all:
        return true;
      case _Filter.sharing:
        return c.state == 'seeding';
      case _Filter.receiving:
        return c.state == 'downloading';
    }
  }

  /// One dropped `.torrent` file starts a download immediately — the same
  /// call the omnibar's own file picker and [AddTorrentScreen]'s picker both
  /// use, so a drop and a pick behave identically. Anything else opens
  /// [ShareScreen] pre-filled, since a new collection still needs a name
  /// nobody but the person dropping the files can supply.
  Future<void> _handleDrop(DropDoneDetails details) async {
    final files = details.files;
    if (files.isEmpty) return;
    if (files.length == 1 &&
        files.single.name.toLowerCase().endsWith('.torrent')) {
      final bytes = await files.single.readAsBytes();
      setState(() => _dropBusy = true);
      try {
        await AppControllers.collections.addFromFileBytes(bytes);
        if (mounted) {
          showToast(context, 'Torrent added — joining swarm',
              severity: ToastSeverity.success);
        }
      } catch (e) {
        if (mounted) {
          showToast(context, 'Couldn\'t add .torrent file: $e',
              severity: ToastSeverity.error);
        }
      } finally {
        if (mounted) setState(() => _dropBusy = false);
      }
      return;
    }
    final picked = await Future.wait(files.map((f) async {
      final Uint8List bytes = await f.readAsBytes();
      return (name: f.name, bytes: bytes);
    }));
    if (mounted) _openShare(picked);
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) =>
          widget.embedded ? _buildDesktop(context) : _buildMobile(context),
    );
  }

  List<Collection> get _shown => AppControllers.collections.collections
      .where(_matchesQuery)
      .where(_matchesFilter)
      .toList(growable: false);

  Widget _omnibar() =>
      Omnibar(onSearch: (q) => setState(() => _query = q), onInvite: _openJoin);

  Widget _shareButton({required bool expand}) => PrimaryAction(
        key: const Key('shareSomethingButton'),
        label: 'New share',
        icon: Icons.add,
        trailingChevron: false,
        expand: expand,
        onTap: () => _openShare(),
      );

  /// Desktop has the width to fit the omnibar and both buttons in one row —
  /// the same row `desktop_collections_pane.dart` always used. A phone-width
  /// window does not, so the omnibar gets its own row there, same as the
  /// full-width "Share something" button `home_screen.dart` used to give it.
  Widget _actionsRow({required bool wide}) {
    if (wide) {
      return Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Expanded(child: _omnibar()),
          const SizedBox(width: 12),
          _shareButton(expand: false),
          const SizedBox(width: 8),
          AddTorrentAction(onTap: _openAddTorrent),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        _omnibar(),
        const SizedBox(height: 10),
        Row(
          children: [
            Expanded(child: _shareButton(expand: true)),
            const SizedBox(width: 8),
            AddTorrentAction(onTap: _openAddTorrent),
          ],
        ),
      ],
    );
  }

  // ---------------------------------------------------------------- Desktop

  /// The same title and subtitle the old mobile Collections screen stated,
  /// from the same frame — they used to be two hand-built headers (one per
  /// platform) that had already drifted to different levels of detail.
  String _summary(List<Collection> all) {
    if (AppControllers.collections.liveRate <= 0) {
      return plural(all.length, 'collection');
    }
    final down = all.fold<double>(0, (s, c) => s + c.downloadMbps);
    final up = all.fold<double>(0, (s, c) => s + c.uploadMbps);
    return '${plural(all.where((c) => c.isMoving).length, 'transfer')}'
        ' · ↓ ${formatRate(down)} · ↑ ${formatRate(up)}';
  }

  Widget _buildDesktop(BuildContext context) {
    final all = AppControllers.collections.collections;
    final error = AppControllers.collections.lastError;
    final shown = _shown;

    final pane = AppScreen(
      title: 'Home',
      subtitle: Text(_summary(all)),
      embedded: true,
      width: ScreenWidth.full,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 12),
            child: _actionsRow(wide: true),
          ),
          Padding(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 4),
            child: FilterChips(
              labels: const ['All', 'Sharing', 'Receiving'],
              selected: _Filter.values.indexOf(_filter),
              onSelected: (i) => setState(() => _filter = _Filter.values[i]),
            ),
          ),
          Expanded(child: _body(all, shown, error)),
        ],
      ),
    );

    return DropTarget(
      onDragDone: _handleDrop,
      child: _dropBusy
          ? Stack(
              children: [
                pane,
                const Positioned(
                  top: 0,
                  left: 0,
                  right: 0,
                  child: LinearProgressIndicator(minHeight: 2),
                ),
              ],
            )
          : pane,
    );
  }

  Widget _body(List<Collection> all, List<Collection> shown, String? error) {
    if (shown.isNotEmpty) {
      return CollectionsList(
        collections: shown,
        openId: widget.openId,
        onOpen: _openCollection,
        detailFor: (collection) => CollectionDetail(
          key: ValueKey(collection.id),
          collection: collection,
          showHeading: false,
        ),
      );
    }
    if (error != null) {
      return CollectionsErrorState(message: error);
    }
    if (_query.isNotEmpty) {
      return Center(
        child: Text(
          'Nothing matches "$_query".',
          textAlign: TextAlign.center,
          style: AppText.body(color: AppColors.textDim),
        ),
      );
    }
    if (_filter != _Filter.all) {
      return Center(
        child: Text(
          _filter == _Filter.sharing
              ? 'Nothing is being shared right now.'
              : 'Nothing is being received right now.',
          style: AppText.body(color: AppColors.textDim),
        ),
      );
    }
    return const EmptyCollectionsWelcome();
  }

  // ----------------------------------------------------------------- Mobile

  Widget _buildMobile(BuildContext context) {
    final all = AppControllers.collections.collections;
    final error = AppControllers.collections.lastError;
    final moving = all
        .where((c) => c.downloadMbps > 0 || c.uploadMbps > 0)
        .toList()
      ..sort((a, b) => (b.downloadMbps + b.uploadMbps)
          .compareTo(a.downloadMbps + a.uploadMbps));
    final hero = moving.isEmpty ? null : moving.first;
    final shown = _shown;

    // A failed backend must not look identical to one that answered
    // "nothing" — full-page, same as desktop's [_body] — but only once
    // there is nothing else to show; a stale list from before the last
    // failed poll is still worth showing.
    if (all.isEmpty && error != null) {
      return PageBody(
        child: CustomScrollView(
          slivers: [
            SliverToBoxAdapter(
              child: HomeHeader(
                identity: AppControllers.identity,
                collections: AppControllers.collections,
              ),
            ),
            SliverFillRemaining(
              hasScrollBody: false,
              child: CollectionsErrorState(message: error),
            ),
          ],
        ),
      );
    }

    return PageBody(
      child: CustomScrollView(
        slivers: [
          SliverToBoxAdapter(
            child: HomeHeader(
              identity: AppControllers.identity,
              collections: AppControllers.collections,
            ),
          ),
          if (!AppControllers.collections.engineReady)
            const SliverToBoxAdapter(child: EngineStartingNotice()),
          if (hero != null)
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                child: LiveTransferCard(
                  collection: hero,
                  onTap: () => _openCollection(hero),
                ),
              ),
            ),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
              child: _actionsRow(wide: false),
            ),
          ),
          if (all.isNotEmpty) ...[
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(22, 18, 22, 0),
                child: Text(
                  _summary(all),
                  style: monoLabel(size: 11, color: AppColors.textDim),
                ),
              ),
            ),
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(22, 10, 22, 0),
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
                      style: AppText.body(color: AppColors.textDim),
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
                    onTap: () => _openCollection(shown[i]),
                  ),
                ),
              ),
          ] else
            const SliverToBoxAdapter(
              child: Padding(
                padding: EdgeInsets.fromLTRB(34, 26, 34, 0),
                child: Welcome(titleSize: 30),
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
  }
}
