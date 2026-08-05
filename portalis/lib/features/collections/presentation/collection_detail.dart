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
import 'collection_overview.dart';
import '../../../theme.dart';
import 'collection_removal.dart';

/// Shows one collection and coordinates user actions with the collections
/// controller. Collection-specific rendering lives in the presentation layer.
class CollectionDetail extends StatefulWidget {
  const CollectionDetail({
    super.key,
    required this.collection,
    this.showHeading = true,
  });

  final Collection collection;

  /// False where the name and item count are already visible in the desktop
  /// list row above this detail.
  final bool showHeading;

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
  bool _showDetails = false;

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
          showHeading: widget.showHeading,
          showDetails: _showDetails,
          busy: _busy,
          onToggleDetails: () => setState(() => _showDetails = !_showDetails),
          onDelete: _delete,
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
          CollectionContents(
            collection: collection,
            onOpenMedia: (media) => _openMedia(collection, media),
          ),
      ],
    );
  }
}
