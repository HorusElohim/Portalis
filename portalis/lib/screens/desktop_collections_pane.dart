import 'package:flutter/material.dart';

import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'collection_screen.dart';

/// The desktop shell's always-visible centre: the paste-or-search bar, the
/// one primary action, and the list itself.
///
/// Split out of `desktop_shell.dart` for the same reason [DesktopSidebar]
/// was: this pane's own concerns — filtering, what to show when there is
/// nothing to filter — are unrelated to which pane the shell has selected,
/// and reading them tangled together in one file made both harder to check.
class DesktopCollectionsPane extends StatefulWidget {
  const DesktopCollectionsPane({
    super.key,
    required this.openId,
    required this.onOpen,
    required this.onShare,
    required this.onJoin,
  });

  final String? openId;
  final ValueChanged<String> onOpen;
  final VoidCallback onShare;

  /// Called with an invite code the omnibar recognised.
  final ValueChanged<String> onJoin;

  @override
  State<DesktopCollectionsPane> createState() =>
      _DesktopCollectionsPaneState();
}

class _DesktopCollectionsPaneState extends State<DesktopCollectionsPane> {
  String _query = '';

  /// Matches on the collection's name and on the files inside it, because
  /// "where is that photo" is the question a filter over a list of albums is
  /// actually being asked.
  bool _matches(Collection c) {
    if (_query.isEmpty) return true;
    final q = _query.toLowerCase();
    return c.name.toLowerCase().contains(q) ||
        c.media.any((m) => m.label.toLowerCase().contains(q));
  }

  @override
  Widget build(BuildContext context) {
    final all = Collections.instance.collections;
    final error = Collections.instance.lastError;
    final collections = all.where(_matches).toList(growable: false);
    final moving = all.where((c) => c.isMoving).length;

    // The same title and subtitle the mobile Collections screen states, from
    // the same frame — they used to be two hand-built headers that had
    // drifted to different sizes and gutters.
    return AppScreen(
      title: 'Collections',
      subtitle: Text(
        moving == 0
            ? '${all.length} collection${all.length == 1 ? '' : 's'}'
            : '$moving transfer${moving == 1 ? '' : 's'} in flight',
      ),
      embedded: true,
      width: ScreenWidth.full,
      body: Column(
        crossAxisAlignment: CrossAxisAlignment.stretch,
        children: [
          Padding(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 16),
            child: Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: Omnibar(
                    onSearch: (q) => setState(() => _query = q),
                    onInvite: widget.onJoin,
                  ),
                ),
                const SizedBox(width: 12),
                // The one primary action, beside the one field — the two
                // things this window is for.
                PrimaryAction(
                  label: 'New share',
                  icon: Icons.add,
                  trailingChevron: false,
                  expand: false,
                  onTap: widget.onShare,
                ),
              ],
            ),
          ),
          Expanded(child: _body(collections, error)),
        ],
      ),
    );
  }

  Widget _body(List<Collection> collections, String? error) {
    if (collections.isNotEmpty) {
      return _CollectionList(
        collections: collections,
        openId: widget.openId,
        onOpen: widget.onOpen,
      );
    }
    // A failed backend or a search with nothing in it: a sentence is the
    // whole story, so [_EmptyWelcome] — which is *about* having nothing
    // yet — would be the wrong story to tell.
    if (error != null || _query.isNotEmpty) {
      return Center(
        child: Text(
          error ?? 'Nothing matches "$_query".',
          textAlign: TextAlign.center,
          style: AppText.body(
              color: error != null ? AppColors.danger : AppColors.textDim),
        ),
      );
    }
    return const _EmptyWelcome();
  }
}

/// The list itself, once there is something in it.
class _CollectionList extends StatelessWidget {
  const _CollectionList({
    required this.collections,
    required this.openId,
    required this.onOpen,
  });

  final List<Collection> collections;
  final String? openId;
  final ValueChanged<String> onOpen;

  @override
  Widget build(BuildContext context) {
    return ListView.separated(
      padding: const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 28),
      itemCount: collections.length,
      separatorBuilder: (_, __) => const SizedBox(height: 10),
      itemBuilder: (context, i) {
        final c = collections[i];
        final open = c.id == openId;
        return CollectionRow(
          collection: c,
          selected: open,
          onTap: () => onOpen(c.id),
          // The card *is* the view. Keyed by id so opening another starts
          // fresh rather than carrying the previous one's disclosure across.
          detail: open
              ? CollectionDetail(
                  key: ValueKey(c.id),
                  collection: c,
                  showHeading: false,
                )
              : null,
        );
      },
    );
  }
}

/// The welcome, for when there is nothing to list yet.
///
/// Same motif as mobile's Home — see [Welcome] — rather than the plain
/// "Nothing here yet." this pane fell back to before. The omnibar and New
/// share above it already say what to do; this says what Portalis is while
/// you decide.
class _EmptyWelcome extends StatelessWidget {
  const _EmptyWelcome();

  @override
  Widget build(BuildContext context) {
    return Center(
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
}
