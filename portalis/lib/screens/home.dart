import 'dart:async';

import 'package:desktop_drop/desktop_drop.dart';
import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/collection.dart';
import '../features/collections/domain/collection_filter.dart';
import '../features/collections/domain/picked_file.dart';
import '../features/collections/presentation/collection_detail.dart';
import '../features/collections/presentation/collection_commands.dart';
import '../features/collections/presentation/collection_join.dart';
import '../features/collections/presentation/collection_library.dart';
import '../features/collections/presentation/collection_share.dart';
import '../features/collections/presentation/collection_removal.dart';
import '../services/navigation.dart';

/// App-shell adapter for the Collections library. Presentation lives in the
/// feature; this screen only coordinates navigation and desktop file drops.
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
  final String? openId;
  final ValueChanged<String>? onOpen;
  final void Function([List<PickedFile>? initialFiles])? onShare;
  final ValueChanged<String>? onJoin;

  @override
  State<Home> createState() => _HomeState();
}

class _HomeState extends State<Home> {
  String _query = '';
  CollectionFilter _filter = CollectionFilter.all;
  bool _dropBusy = false;
  int _welcomeCycle = 0;
  late bool _homeVisible;

  @override
  void initState() {
    super.initState();
    _homeVisible = _isHomeVisible;
    AppNavigation.tab.addListener(_onNavigationChanged);
    AppNavigation.depth.addListener(_onNavigationChanged);
  }

  bool get _isHomeVisible =>
      AppNavigation.tab.value == AppNavigation.homeTab &&
      AppNavigation.depth.value == 0;

  void _onNavigationChanged() {
    final visible = _isHomeVisible;
    if (visible && !_homeVisible && mounted) {
      setState(() => _welcomeCycle++);
    }
    _homeVisible = visible;
  }

  @override
  void dispose() {
    AppNavigation.tab.removeListener(_onNavigationChanged);
    AppNavigation.depth.removeListener(_onNavigationChanged);
    super.dispose();
  }

  void _push(Widget screen) {
    Navigator.of(context).push(MaterialPageRoute(builder: (_) => screen));
  }

  void _openCollection(Collection collection) {
    if (widget.onOpen != null) {
      widget.onOpen!(collection.id);
    } else {
      _push(CollectionScreen(collection: collection));
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

  void _handleCommand((Collection, CollectionCommand) action) {
    final (collection, command) = action;
    if (command == CollectionCommand.delete) {
      unawaited(confirmAndDeleteCollection(
        context,
        collection,
        setBusy: (_) {},
      ));
      return;
    }

    unawaited(_runCommand(collection, command));
  }

  Future<void> _runCommand(
    Collection collection,
    CollectionCommand command,
  ) async {
    try {
      switch (command) {
        case CollectionCommand.restart:
          await AppControllers.collections.restart(collection.id);
        case CollectionCommand.pause:
          await AppControllers.collections.pause(collection.id);
        case CollectionCommand.delete:
          return;
      }
      if (mounted) showToast(context, '${command.label} applied');
    } catch (error) {
      if (mounted) {
        showToast(context, '$error', severity: ToastSeverity.error);
      }
    }
  }

  bool _matchesQuery(Collection collection) {
    if (_query.isEmpty) return true;
    final query = _query.toLowerCase();
    return collection.name.toLowerCase().contains(query) ||
        collection.media
            .any((media) => media.label.toLowerCase().contains(query));
  }

  List<Collection> get _shown => AppControllers.collections.collections
      .where(_matchesQuery)
      .where(_filter.includes)
      .toList(growable: false);

  Future<void> _handleDrop(DropDoneDetails details) async {
    final files = details.files;
    if (files.isEmpty) return;

    if (files.length == 1 &&
        files.single.name.toLowerCase().endsWith('.torrent')) {
      setState(() => _dropBusy = true);
      try {
        await AppControllers.collections.addFromFilePath(files.single.path);
        if (mounted) {
          showToast(
            context,
            'Torrent added — joining swarm',
            severity: ToastSeverity.success,
          );
        }
      } catch (error) {
        if (mounted) {
          showToast(
            context,
            'Couldn\'t add .torrent file: $error',
            severity: ToastSeverity.error,
          );
        }
      } finally {
        if (mounted) setState(() => _dropBusy = false);
      }
      return;
    }

    final picked = await Future.wait(files.map((file) => pickedFileFrom(
          name: file.name,
          nativePath: file.path,
        )));
    if (mounted) _openShare(picked);
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) {
        final library = CollectionLibrary(
          wide: widget.embedded,
          collectionsController: AppControllers.collections,
          identityController: AppControllers.identity,
          collections: AppControllers.collections.collections,
          shown: _shown,
          error: AppControllers.collections.lastError,
          engineReady: AppControllers.collections.engineReady,
          query: _query,
          filter: _filter,
          openId: widget.openId,
          onOpen: _openCollection,
          onSearch: (query) => setState(() => _query = query),
          onFilterChanged: (filter) => setState(() => _filter = filter),
          onShare: () => _openShare(),
          onJoin: _openJoin,
          onCommand: _handleCommand,
          welcomeCycle: _welcomeCycle,
        );
        if (!widget.embedded) return library;
        return DropTarget(
          onDragDone: _handleDrop,
          child: _dropBusy
              ? Stack(
                  children: [
                    library,
                    const Positioned(
                      top: 0,
                      left: 0,
                      right: 0,
                      child: LinearProgressIndicator(minHeight: 2),
                    ),
                  ],
                )
              : library,
        );
      },
    );
  }
}
