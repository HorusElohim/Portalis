import 'dart:async';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:image_picker/image_picker.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../../../app/app_controllers.dart';
import '../../../design/design.dart';
import '../../media/application/media_formats.dart';
import '../../media/domain/media_item.dart';
import '../../media/presentation/media_viewer_screen.dart';
import '../domain/collection.dart';
import 'collection_contents.dart';
import 'collection_commands.dart';
import 'collection_overview.dart';
import '../../../theme.dart';
import 'collection_removal.dart';

/// Shows one collection and coordinates user actions with the collections
/// controller. Collection-specific rendering lives in the presentation layer.
class CollectionDetail extends StatefulWidget {
  const CollectionDetail({
    super.key,
    required this.collection,
    this.showCommands = true,
    this.level = CollectionDetailLevel.full,
    this.showTitle = true,
  });

  final Collection collection;
  final bool showCommands;

  /// How much to show. Defaults to [CollectionDetailLevel.full] because most
  /// callers (a standalone [CollectionScreen], the desktop pane) are already
  /// a dedicated space for one collection — collapsing has nothing to save
  /// there. Only an inline row in a list, which grows to hold this, has a
  /// reason to ask for less.
  final CollectionDetailLevel level;
  final bool showTitle;

  @override
  State<CollectionDetail> createState() => _CollectionDetailState();
}

/// A collection on its own screen, used on compact layouts.
class CollectionScreen extends StatelessWidget {
  const CollectionScreen({super.key, required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(
        child: PageBody(
          child: SingleChildScrollView(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 0, kScreenGutter, 24),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                NavBackButton(onTap: () => Navigator.of(context).pop()),
                CollectionDetail(collection: collection),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _CollectionDetailState extends State<CollectionDetail> {
  bool _busy = false;

  Collection get _collection =>
      AppControllers.collections.byId(widget.collection.id) ?? widget.collection;

  void _toast(String message, {ToastSeverity severity = ToastSeverity.info}) {
    if (mounted) showToast(context, message, severity: severity);
  }

  Future<void> _run(Future<void> Function() action) async {
    setState(() => _busy = true);
    try {
      await action();
    } catch (error) {
      _toast('$error');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _showInvite() async {
    final code = _collection.inviteCode;
    if (code == null) return;

    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Invite a collaborator'),
        content: SizedBox(
          width: 260,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(
                'Share this code — anyone who enters it can join and add media.',
                style: AppText.secondary(color: AppColors.textDim),
              ),
              const SizedBox(height: 14),
              Center(
                child: Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(AppRadius.inner),
                  ),
                  child: QrImageView(
                    data: code,
                    version: QrVersions.auto,
                    size: 200,
                    backgroundColor: Colors.white,
                    eyeStyle: const QrEyeStyle(
                      eyeShape: QrEyeShape.square,
                      color: Colors.black,
                    ),
                    dataModuleStyle: const QrDataModuleStyle(
                      dataModuleShape: QrDataModuleShape.square,
                      color: Colors.black,
                    ),
                  ),
                ),
              ),
              const SizedBox(height: 14),
              SelectableText(code, style: monoLabel(size: 12, letterSpacing: 0)),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () {
              Clipboard.setData(ClipboardData(text: code));
              showToast(
                dialogContext,
                'Invite code copied',
                severity: ToastSeverity.success,
              );
            },
            child: const Text('Copy'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }

  Future<void> _addMedia() async {
    final source = await showModalBottomSheet<String>(
      context: context,
      backgroundColor: AppColors.surface,
      builder: (sheetContext) => SafeArea(
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            ListTile(
              leading: const Icon(Icons.photo_library_outlined),
              title: const Text('Photos & videos'),
              onTap: () => Navigator.of(sheetContext).pop('photos'),
            ),
            ListTile(
              leading: const Icon(Icons.folder_outlined),
              title: const Text('Files'),
              onTap: () => Navigator.of(sheetContext).pop('files'),
            ),
          ],
        ),
      ),
    );
    if (source == null || !mounted) return;

    List<({String name, Uint8List bytes})> picked = [];
    try {
      if (source == 'photos') {
        final files = await ImagePicker().pickMultipleMedia();
        picked = await Future.wait(
          files.map((file) async => (name: file.name, bytes: await file.readAsBytes())),
        );
      } else {
        final result = await FilePicker.pickFiles(
          withData: true,
          allowMultiple: true,
          type: FileType.any,
        );
        picked = (result?.files ?? [])
            .where((file) => file.bytes != null)
            .map((file) => (name: file.name, bytes: file.bytes!))
            .toList();
      }
    } catch (error) {
      _toast('Couldn\'t read those files: $error');
      return;
    }
    if (picked.isEmpty || !mounted) return;

    await _run(() async {
      final normalized = await Future.wait(
        picked.map((file) => normalizeForSharing(name: file.name, bytes: file.bytes)),
      );
      final label = 'Added ${DateTime.now().toIso8601String().split('T').first}';
      await AppControllers.collections.addMedia(_collection.id, label, normalized);
      _toast('Added ${normalized.length} item${normalized.length == 1 ? '' : 's'}');
    });
  }

  Future<void> _fetchPending() => _run(() async {
        final started = await AppControllers.collections.fetchMedia(_collection.id);
        _toast('Fetching $started item${started == 1 ? '' : 's'}');
      });

  Future<void> _sync() async {
    final controller = TextEditingController();
    final peerAddress = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text('Sync "${_collection.name}"'),
        content: TextField(
          controller: controller,
          autofocus: true,
          style: monoLabel(size: 13, color: AppColors.text, letterSpacing: 0),
          decoration: InputDecoration(
            hintText: '192.168.1.23:54321',
            hintStyle: const TextStyle(color: AppColors.textGhost),
            helperText: "The other device's sync address (on its User screen)",
            helperStyle: AppText.caption(color: AppColors.textDim),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(controller.text.trim()),
            child: const Text('Sync'),
          ),
        ],
      ),
    );
    controller.dispose();
    if (peerAddress == null || peerAddress.isEmpty || !mounted) return;

    await _run(() async {
      final updated = await AppControllers.collections.sync(
        _collection.id,
        peerAddress,
      );
      _toast(
        'Synced — ${updated.media.length} item(s), '
        '${updated.collaborators.length} collaborator(s)',
      );
    });
  }

  Future<void> _delete() => confirmAndRemoveCollection(
        context,
        _collection,
        setBusy: (busy) {
          if (mounted) setState(() => _busy = busy);
        },
      );

  Future<void> _deleteFiles() => confirmAndDeleteCollectionFiles(
        context,
        _collection,
        setBusy: (busy) {
          if (mounted) setState(() => _busy = busy);
        },
      );

  void _openMedia(Collection collection, MediaItem media) {
    Navigator.of(context).push(
      MaterialPageRoute(
        builder: (_) => MediaViewerScreen(collection: collection, media: media),
      ),
    );
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: AppControllers.collections,
      builder: (context, _) => _detail(_collection),
    );
  }

  Widget _detail(Collection collection) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        CollectionOverview(
          collection: collection,
          busy: _busy,
          onCommand: _command,
          history: AppControllers.collections.historyFor(collection.id),
          peerHistory: AppControllers.collections.peerHistoryFor(collection.id),
          onForgetPeer: (address) =>
              AppControllers.collections.forgetPeer(address),
          showCommands: widget.showCommands,
          level: widget.level,
          showTitle: widget.showTitle,
          onInvite: _showInvite,
          onAddMedia: _addMedia,
          onSync: _sync,
          onFetch: _fetchPending,
        ),
        if (_busy)
          const Padding(
            padding: EdgeInsets.only(top: 10),
            child: LinearProgressIndicator(minHeight: 2),
          ),
        // Files are the deepest layer — worth the extra tap they cost a
        // merely-mid row, the same trade the peers section makes.
        if (widget.level == CollectionDetailLevel.full) ...[
          const SizedBox(height: 14),
          if (collection.media.isEmpty)
            Padding(
              padding: const EdgeInsets.symmetric(vertical: 22),
              child: Center(
                child: Text(
                  'Nothing in this collection yet.',
                  style: AppText.secondary(color: AppColors.textDim),
                ),
              ),
            )
          else
            _ResizableMediaPreview(
              child: CollectionContents(
                collection: collection,
                onOpenMedia: (media) => _openMedia(collection, media),
              ),
            ),
        ],
      ],
    );
  }

  void _command(CollectionCommand command) {
    if (command == CollectionCommand.delete) {
      unawaited(_delete());
      return;
    }
    if (command == CollectionCommand.deleteFiles) {
      unawaited(_deleteFiles());
      return;
    }
    unawaited(_run(() async {
      final id = _collection.id;
      switch (command) {
        case CollectionCommand.restart:
          await AppControllers.collections.restart(id);
        case CollectionCommand.pause:
          await AppControllers.collections.pause(id);
        case CollectionCommand.forget:
          await AppControllers.collections.stopCollection(id);
        case CollectionCommand.delete:
        case CollectionCommand.deleteFiles:
          return;
      }
      _toast('${command.label} applied');
    }));
  }
}

/// A fixed-height, drag-to-resize frame around the media grid.
///
/// The grid itself still sizes to its own content (`shrinkWrap: true` in
/// [CollectionContents]) — a large collection could otherwise push the whole
/// card, and the list, far down the page. Scrolling inside a height the
/// person controls themselves is the same trade every other resizable panel
/// makes.
class _ResizableMediaPreview extends StatefulWidget {
  const _ResizableMediaPreview({required this.child});

  final Widget child;

  @override
  State<_ResizableMediaPreview> createState() => _ResizableMediaPreviewState();
}

class _ResizableMediaPreviewState extends State<_ResizableMediaPreview> {
  static const double _minHeight = 160;
  static const double _maxHeight = 640;
  static const double _defaultHeight = 280;

  double _height = _defaultHeight;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.stretch,
      children: [
        SizedBox(
          height: _height,
          child: SingleChildScrollView(child: widget.child),
        ),
        MouseRegion(
          cursor: SystemMouseCursors.resizeUpDown,
          child: GestureDetector(
            behavior: HitTestBehavior.opaque,
            onVerticalDragUpdate: (details) {
              setState(() {
                _height = (_height + details.delta.dy)
                    .clamp(_minHeight, _maxHeight);
              });
            },
            child: Padding(
              padding: const EdgeInsets.symmetric(vertical: 6),
              child: Center(
                child: Container(
                  width: 36,
                  height: 4,
                  decoration: BoxDecoration(
                    color: AppColors.borderStrong,
                    borderRadius: BorderRadius.circular(AppRadius.pill),
                  ),
                ),
              ),
            ),
          ),
        ),
      ],
    );
  }
}
