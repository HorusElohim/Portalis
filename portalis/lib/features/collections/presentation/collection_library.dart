import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../identity/application/identity_controller.dart';
import '../application/collections_controller.dart';
import '../domain/collection.dart';
import '../domain/collection_filter.dart';
import 'collection_detail.dart';
import 'collection_commands.dart';
import 'collection_views.dart';
import 'collections_list.dart';
import 'empty_collections_call_to_action.dart';
import 'empty_collections_welcome.dart';
import 'home_header.dart';
import 'home_library_toolbar.dart';
import 'share_collection_action.dart';

/// Adaptive collection library layout. It renders supplied state only; route
/// selection, native lifecycle, and file-drop handling remain outside it.
class CollectionLibrary extends StatelessWidget {
  const CollectionLibrary({
    super.key,
    required this.wide,
    required this.collectionsController,
    required this.identityController,
    required this.collections,
    required this.shown,
    required this.error,
    required this.engineReady,
    required this.query,
    required this.filter,
    required this.openId,
    required this.onOpen,
    required this.onSearch,
    required this.onFilterChanged,
    required this.onShare,
    required this.onJoin,
    required this.onCommand,
    required this.welcomeCycle,
  });

  final bool wide;
  final CollectionsController collectionsController;
  final IdentityController identityController;
  final List<Collection> collections;
  final List<Collection> shown;
  final String? error;
  final bool engineReady;
  final String query;
  final CollectionFilter filter;
  final String? openId;
  final ValueChanged<Collection> onOpen;
  final ValueChanged<String> onSearch;
  final ValueChanged<CollectionFilter> onFilterChanged;
  final VoidCallback onShare;
  final ValueChanged<String> onJoin;
  final ValueChanged<(Collection, CollectionCommand)> onCommand;
  final int welcomeCycle;

  @override
  Widget build(BuildContext context) => wide ? _wide() : _compact();

  Widget _toolbar({required bool wide, required bool showFilters}) =>
      HomeLibraryToolbar(
        wide: wide,
        showFilters: showFilters,
        filter: filter,
        onSearch: onSearch,
        onFilterChanged: onFilterChanged,
        onJoin: onJoin,
      );

  Widget _wide() => AppScreen(
        title: 'Home',
        subtitle: Text(_summary()),
        embedded: true,
        width: ScreenWidth.full,
        body: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(
                kScreenGutter,
                0,
                kScreenGutter,
                12,
              ),
              child: _toolbar(wide: true, showFilters: false),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(
                kScreenGutter,
                0,
                kScreenGutter,
                28,
              ),
              child: FilterChips(
                labels: const ['All', 'Sharing', 'Receiving'],
                selected: CollectionFilter.values.indexOf(filter),
                onSelected: (index) => onFilterChanged(
                  CollectionFilter.values[index],
                ),
              ),
            ),
            Expanded(child: _wideBody()),
          ],
        ),
      );

  Widget _wideBody() {
    if (shown.isNotEmpty) {
      return CollectionsList(
        collections: shown,
        openId: openId,
        onOpen: onOpen,
        onCommand: onCommand,
        footer: ShareCollectionAction(onTap: onShare),
        detailFor: (collection, level, inlineHeader, inlineStatus) =>
            CollectionDetail(
          key: ValueKey(collection.id),
          collection: collection,
          showCommands: true,
          level: level,
          showTitle: false,
          inlineHeader: inlineHeader,
          inlineStatus: inlineStatus,
        ),
      );
    }
    if (error != null) return CollectionsErrorState(message: error!);
    if (query.isNotEmpty) return _emptyMessage('Nothing matches "$query".');
    if (filter != CollectionFilter.all) {
      return _emptyMessage(
        filter == CollectionFilter.sharing
            ? 'Nothing is being shared right now.'
            : 'Nothing is being received right now.',
      );
    }
    return EmptyCollectionsWelcome(
      onShare: onShare,
      welcomeCycle: welcomeCycle,
    );
  }

  Widget _compact() {
    if (collections.isEmpty && error != null) {
      return PageBody(
        child: CustomScrollView(
          slivers: [
            SliverToBoxAdapter(child: _header()),
            SliverFillRemaining(
              hasScrollBody: false,
              child: CollectionsErrorState(message: error!),
            ),
          ],
        ),
      );
    }

    return PageBody(
      child: CustomScrollView(
        slivers: [
          SliverToBoxAdapter(child: _header()),
          if (!engineReady)
            const SliverToBoxAdapter(child: EngineStartingNotice()),
          SliverToBoxAdapter(
            child: Padding(
              padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
              child: _toolbar(wide: false, showFilters: collections.length > 1),
            ),
          ),
          if (collections.isNotEmpty) ...[
            if (shown.isEmpty)
              SliverToBoxAdapter(
                child: Padding(
                  padding: const EdgeInsets.fromLTRB(22, 40, 22, 0),
                  child: Center(
                    child: _emptyMessage(
                      filter == CollectionFilter.sharing
                          ? 'Nothing is being shared right now.'
                          : 'Nothing is being received right now.',
                    ),
                  ),
                ),
              )
            else
              SliverPadding(
                padding: const EdgeInsets.fromLTRB(28, 34, 28, 0),
                sliver: SliverList.separated(
                  itemCount: shown.length,
                  separatorBuilder: (_, __) => const SizedBox(height: 14),
                  itemBuilder: (context, index) => CollectionRow(
                    collection: shown[index],
                    onTap: () => onOpen(shown[index]),
                    onCommand: (command) => onCommand((shown[index], command)),
                  ),
                ),
              ),
          ] else
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(34, 26, 34, 0),
                child: EmptyCollectionsCallToAction(
                  onShare: onShare,
                  welcomeCycle: welcomeCycle,
                ),
              ),
            ),
          if (collections.isNotEmpty)
            SliverToBoxAdapter(
              child: Padding(
                padding: const EdgeInsets.fromLTRB(22, 18, 22, 8),
                child: ShareCollectionAction(onTap: onShare),
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

  Widget _header() => HomeHeader(
        identity: identityController,
        collections: collectionsController,
      );

  Widget _emptyMessage(String message) => Center(
        child: Text(
          message,
          textAlign: TextAlign.center,
          style: AppText.body(color: AppColors.textDim),
        ),
      );

  String _summary() {
    if (collectionsController.liveRate <= 0) {
      return plural(collections.length, 'collection');
    }
    final down = collections.fold<double>(
      0,
      (sum, collection) => sum + collection.downloadMbps,
    );
    final up = collections.fold<double>(
      0,
      (sum, collection) => sum + collection.uploadMbps,
    );
    return '${plural(collections.where((item) => item.isMoving).length, 'transfer')}'
        ' · ↓ ${formatRate(down)} · ↑ ${formatRate(up)}';
  }
}
