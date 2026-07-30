

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:image_picker/image_picker.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../media_convert.dart';
import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'collection_details_screen.dart';
import 'media_viewer_screen.dart';

/// One collection, of either kind. Everything here now comes from the single
/// unified model in `collections.rs`: a shared collection carries its own
/// invite code (no more minting a throwaway one on the "Add collab" tap,
/// which is what this screen used to do because the two models weren't
/// linked), and its media is the union across every manifest entry.
///
/// Stateful and id-based rather than rendering the [Collection] it was
/// constructed with: sync, fetch and add-media all change the collection
/// while this screen is open, and it re-reads from [Collections] so those
/// land without a manual back-and-forward.
class CollectionScreen extends StatefulWidget {
  const CollectionScreen({super.key, required this.collection});

  final Collection collection;

  @override
  State<CollectionScreen> createState() => _CollectionScreenState();
}

class _CollectionScreenState extends State<CollectionScreen> {
  bool _busy = false;

  /// The live version, falling back to the one we were pushed with if it has
  /// since been deleted (the screen pops in that case).
  Collection get _collection =>
      Collections.instance.byId(widget.collection.id) ?? widget.collection;

  void _toast(String msg) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  Future<void> _run(Future<void> Function() action) async {
    setState(() => _busy = true);
    try {
      await action();
    } catch (e) {
      _toast('$e');
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
        // Explicit width, not an intrinsic one: AlertDialog measures its
        // content's intrinsic width, and QrImageView builds a LayoutBuilder
        // internally — which cannot answer an intrinsic query. A fixed-width
        // box short-circuits that walk.
        content: SizedBox(
          width: 260,
          child: Column(
            mainAxisSize: MainAxisSize.min,
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const Text(
                'Share this code — anyone who enters it can join and add media.',
                style: TextStyle(fontSize: 12, color: AppColors.neutral400),
              ),
              const SizedBox(height: 14),
              Center(
                child: Container(
                  padding: const EdgeInsets.all(12),
                  decoration: BoxDecoration(
                    color: Colors.white,
                    borderRadius: BorderRadius.circular(10),
                  ),
                  // White background: QR readers need light-on-dark contrast
                  // regardless of the app's dark theme.
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
              SelectableText(
                code,
                style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
              ),
            ],
          ),
        ),
        actions: [
          TextButton(
            onPressed: () {
              Clipboard.setData(ClipboardData(text: code));
              ScaffoldMessenger.of(dialogContext).showSnackBar(
                const SnackBar(content: Text('Invite code copied')),
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

  /// Adds a new batch of files as one new signed manifest entry — the thing
  /// that makes a collection *growable*, and the reason this button finally
  /// does something (it was a no-op while the two models were separate).
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

    // Picking has to be guarded too, not just the upload that follows it: a
    // denied photo-library permission throws out of the picker, and an
    // unhandled throw here would surface as nothing happening at all.
    List<({String name, Uint8List bytes})> picked = [];
    try {
      if (source == 'photos') {
        final xfiles = await ImagePicker().pickMultipleMedia();
        picked = await Future.wait(
          xfiles.map((f) async => (name: f.name, bytes: await f.readAsBytes())),
        );
      } else {
        final result = await FilePicker.pickFiles(
          withData: true,
          allowMultiple: true,
          type: FileType.any,
        );
        picked = (result?.files ?? [])
            .where((f) => f.bytes != null)
            .map((f) => (name: f.name, bytes: f.bytes!))
            .toList();
      }
    } catch (e) {
      _toast('Couldn\'t read those files: $e');
      return;
    }
    if (picked.isEmpty || !mounted) return;

    await _run(() async {
      final normalized = await Future.wait(
        picked.map((f) => normalizeForSharing(name: f.name, bytes: f.bytes)),
      );
      final label = 'Added ${DateTime.now().toIso8601String().split('T').first}';
      await Collections.instance.addMedia(_collection.id, label, normalized);
      _toast('Added ${normalized.length} item${normalized.length == 1 ? '' : 's'}');
    });
  }

  Future<void> _fetchPending() => _run(() async {
        final started = await Collections.instance.fetchMedia(_collection.id);
        _toast('Fetching $started item${started == 1 ? '' : 's'}');
      });

  Future<void> _sync() async {
    final controller = TextEditingController();
    final peerAddr = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text('Sync "${_collection.name}"'),
        content: TextField(
          controller: controller,
          autofocus: true,
          style: const TextStyle(
              color: AppColors.text, fontSize: 13, fontFamily: 'monospace'),
          decoration: const InputDecoration(
            hintText: '192.168.1.23:54321',
            hintStyle: TextStyle(color: AppColors.neutral500),
            helperText: "The other device's sync address (on its User screen)",
            helperStyle: TextStyle(fontSize: 10.5, color: AppColors.neutral400),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () =>
                Navigator.of(dialogContext).pop(controller.text.trim()),
            child: const Text('Sync'),
          ),
        ],
      ),
    );
    if (peerAddr == null || peerAddr.isEmpty || !mounted) return;
    await _run(() async {
      final updated = await Collections.instance.sync(_collection.id, peerAddr);
      _toast('Synced — ${updated.media.length} item(s), '
          '${updated.collaborators.length} collaborator(s)');
    });
  }

  Future<void> _delete() async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text('Remove "${_collection.name}"?'),
        content: const Text(
          'This only removes it from this device. Downloaded files stay on '
          'disk, and other collaborators keep their own copies.',
          style: TextStyle(fontSize: 12, color: AppColors.neutral400),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Remove'),
          ),
        ],
      ),
    );
    if (confirmed != true || !mounted) return;
    // Not fire-and-forget: deleting genuinely fails (a torrent that isn't in
    // the session, a store write that can't land), and without this the
    // dialog would just close with nothing happening and no error shown.
    setState(() => _busy = true);
    try {
      await Collections.instance.delete(_collection.id);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      _toast('Couldn\'t remove this collection: $e');
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) {
        final collection = _collection;
        final shown = collection.collaborators.take(6).toList();
        final remaining = collection.collaborators.length - shown.length;
        final adminCount =
            collection.collaborators.where((c) => c.isAdmin).length;

        return Scaffold(
          backgroundColor: AppColors.bg,
          body: SafeArea(
            child: Column(
              children: [
                // No cover image: collection artwork isn't modeled anywhere
                // in the backend, so there's nothing to render one from.
                Row(
                  children: [
                    NavBackButton(onTap: () => Navigator.of(context).pop()),
                    const Spacer(),
                    if (adminCount > 0)
                      Text(
                        '$adminCount admin${adminCount == 1 ? '' : 's'}',
                        style: const TextStyle(
                          fontSize: 10,
                          fontFamily: 'monospace',
                          color: AppColors.neutral300,
                        ),
                      ),
                    IconButton(
                      tooltip: 'Details',
                      icon: const Icon(Icons.info_outline,
                          size: 18, color: AppColors.neutral400),
                      onPressed: () => Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (_) => CollectionDetailsScreen(
                            collectionId: collection.id,
                          ),
                        ),
                      ),
                    ),
                    IconButton(
                      tooltip: 'Remove from this device',
                      icon: const Icon(Icons.delete_outline,
                          size: 18, color: AppColors.neutral400),
                      onPressed: _busy ? null : _delete,
                    ),
                  ],
                ),
                Expanded(
                  child: SingleChildScrollView(
                    child: Padding(
                      padding: const EdgeInsets.symmetric(horizontal: 20),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            collection.name,
                            style: const TextStyle(
                              fontSize: 20,
                              fontWeight: FontWeight.w500,
                            ),
                          ),
                          const SizedBox(height: 6),
                          Text(
                            collection.isShared
                                ? 'Shared collection · ${collection.subtitle}'
                                : 'Torrent · ${collection.subtitle}',
                            style: const TextStyle(
                              fontSize: 10.5,
                              fontFamily: 'monospace',
                              color: AppColors.neutral300,
                            ),
                          ),
                          const SizedBox(height: 6),
                          CopiesIndicator(
                            color: collection.hue,
                            label: collection.copiesLabel,
                            fontSize: 12,
                          ),
                          const SizedBox(height: 14),
                          _Collaborators(
                            collection: collection,
                            shown: shown,
                            remaining: remaining,
                          ),
                          const SizedBox(height: 12),
                          if (collection.media.isEmpty)
                            const Padding(
                              padding: EdgeInsets.symmetric(vertical: 28),
                              child: Center(
                                child: Text(
                                  'Nothing in this collection yet.',
                                  style: TextStyle(
                                      fontSize: 12,
                                      color: AppColors.neutral400),
                                ),
                              ),
                            )
                          else
                            _MediaGrid(collection: collection),
                          const SizedBox(height: 12),
                        ],
                      ),
                    ),
                  ),
                ),
                if (_busy) const LinearProgressIndicator(minHeight: 2),
                Padding(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
                  child: Wrap(
                    alignment: WrapAlignment.center,
                    spacing: 10,
                    runSpacing: 8,
                    children: [
                      // Invite/add/sync exist only for shared collections —
                      // a plain torrent has no invite secret and its contents
                      // are fixed forever by its info-hash.
                      if (collection.isShared) ...[
                        PillButton(
                          label: 'Invite',
                          icon: const Icon(Icons.people_alt_outlined,
                              size: 16, color: AppColors.accent300),
                          onTap: _busy ? null : _showInvite,
                        ),
                        PillButton(
                          label: '＋ Add media',
                          dim: true,
                          onTap: _busy ? null : _addMedia,
                        ),
                        PillButton(
                          label: 'Sync',
                          dim: true,
                          onTap: _busy ? null : _sync,
                        ),
                      ],
                      if (collection.pendingMedia > 0)
                        PillButton(
                          label: 'Fetch ${collection.pendingMedia}',
                          onTap: _busy ? null : _fetchPending,
                        ),
                    ],
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _Collaborators extends StatelessWidget {
  const _Collaborators({
    required this.collection,
    required this.shown,
    required this.remaining,
  });

  final Collection collection;
  final List<Collaborator> shown;
  final int remaining;

  @override
  Widget build(BuildContext context) {
    // The avatar stack needs *named* collaborators, which only shared
    // collections have. A plain torrent's peers are just IP:port, so this
    // degrades to a peer count rather than rendering an empty stack.
    if (shown.isEmpty) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel('PEERS · ${collection.livePeers}'),
          const SizedBox(height: 7),
          Text(
            collection.livePeers == 0
                ? 'No peers connected right now'
                : '${collection.peersLabel} connected · not identified by name',
            style: const TextStyle(
              fontSize: 10,
              fontFamily: 'monospace',
              color: AppColors.neutral400,
            ),
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        SectionLabel('COLLABORATORS · ${collection.collaborators.length}'),
        const SizedBox(height: 7),
        SizedBox(
          height: 27,
          child: Row(
            children: [
              SizedBox(
                width: 27.0 + (shown.length - 1) * 19,
                child: Stack(
                  children: [
                    for (var i = 0; i < shown.length; i++)
                      Positioned(
                        left: i * 19.0,
                        child: Container(
                          decoration: BoxDecoration(
                            shape: BoxShape.circle,
                            border:
                                Border.all(color: AppColors.bg, width: 2),
                          ),
                          child:
                              Avatar(initials: shown[i].initials, size: 27),
                        ),
                      ),
                  ],
                ),
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  remaining > 0
                      ? '+$remaining more'
                      : shown.map((c) => c.name).join(', '),
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                    fontSize: 10,
                    fontFamily: 'monospace',
                    color: AppColors.neutral400,
                  ),
                ),
              ),
            ],
          ),
        ),
      ],
    );
  }
}

class _MediaGrid extends StatelessWidget {
  const _MediaGrid({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    return GridView.builder(
      shrinkWrap: true,
      physics: const NeverScrollableScrollPhysics(),
      gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
        crossAxisCount: 3,
        mainAxisSpacing: 8,
        crossAxisSpacing: 8,
        childAspectRatio: 1,
      ),
      itemCount: collection.media.length,
      itemBuilder: (context, index) {
        final m = collection.media[index];
        return PerimeterProgress(
          progress: m.progress,
          color: collection.hue,
          borderRadius: BorderRadius.circular(6),
          child: Container(
            // Always-visible boundary, independent of the progress ring —
            // otherwise a finished (or not-yet-started) tile has no outline
            // at all, since the ring only paints while 0 < progress < 1.
            decoration: BoxDecoration(
              borderRadius: BorderRadius.circular(6),
              border: Border.all(color: AppColors.border),
            ),
            clipBehavior: Clip.antiAlias,
            child: Material(
              color: Colors.transparent,
              child: InkWell(
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(
                    builder: (_) => MediaViewerScreen(
                      collection: collection,
                      media: m,
                    ),
                  ),
                ),
                child: Stack(
                  fit: StackFit.expand,
                  children: [
                    MediaThumbnail(media: m, borderRadius: 6),
                    // "Known but not fetched": a peer signed this into the
                    // manifest, we just don't have the bytes. Distinct from
                    // "downloading" (the progress ring) and "ready".
                    if (!m.fetched)
                      Container(
                        color: AppColors.bg.withValues(alpha: 0.55),
                        alignment: Alignment.center,
                        child: const Icon(
                          Icons.cloud_download_outlined,
                          size: 22,
                          color: AppColors.accent300,
                        ),
                      ),
                  ],
                ),
              ),
            ),
          ),
        );
      },
    );
  }
}
