import 'dart:async';

import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../bridge_generated/collections.dart' as bridge;
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'collection_screen.dart';

/// What's actually on disk under the download directory, joined against the
/// collections the app knows about — not just the raw filesystem, and not
/// just the one aggregate figure Settings shows. Each row is one top-level
/// item there (almost always one manifest entry's own batch folder; see
/// `collections::add_media_to_collection`), sized recursively and ranked
/// largest first, so "where did my space go" has an answer — and, when a
/// collection still claims it, a real link to that collection rather than
/// just a path.
class StorageScreen extends StatefulWidget {
  const StorageScreen({super.key, this.embedded = false, this.onBack});

  /// Set when this replaces the Settings pane in place on desktop rather
  /// than being pushed over it — see [AdaptiveScreen].
  final bool embedded;

  /// Called instead of popping a route. Only meaningful when [embedded]:
  /// there is no route to pop there, so the caller supplies its own
  /// "collapse back to Settings" callback.
  final VoidCallback? onBack;

  @override
  State<StorageScreen> createState() => _StorageScreenState();
}

class _StorageScreenState extends State<StorageScreen> {
  List<bridge.StorageEntry>? _entries;
  String? _error;
  Timer? _poll;

  @override
  void initState() {
    super.initState();
    _load();
    // Downloads finish and collections get deleted while this is open —
    // same cadence as Settings' own storage poll, and just as cheap: the
    // backend caches the walk for 5s regardless of who's asking.
    _poll = Timer.periodic(const Duration(seconds: 2), (_) => _load());
  }

  @override
  void dispose() {
    _poll?.cancel();
    super.dispose();
  }

  Future<void> _load() async {
    try {
      final entries = await bridge.storageBreakdown();
      if (!mounted) return;
      setState(() {
        _entries = entries;
        _error = null;
      });
    } catch (e) {
      if (!mounted) return;
      setState(() => _error = '$e');
    }
  }

  int get _totalBytes =>
      _entries?.fold<int>(0, (sum, e) => sum + e.bytes.toInt()) ?? 0;

  @override
  Widget build(BuildContext context) {
    final entries = _entries;
    return AdaptiveScreen(
      embedded: widget.embedded,
      forceShowBack: true,
      onBack: widget.onBack,
      body: PageBody(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(22, 8, 22, 0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  const CanvasTitle('Storage', size: 30),
                  const SizedBox(height: 6),
                  Text(
                    entries == null
                        ? 'Reading the download folder…'
                        : '${formatBytes(_totalBytes)} across '
                            '${entries.length} item'
                            '${entries.length == 1 ? '' : 's'}',
                    style:
                        const TextStyle(fontSize: 13, color: AppColors.textDim),
                  ),
                ],
              ),
            ),
            const SizedBox(height: 14),
            Expanded(child: _body(entries)),
          ],
        ),
      ),
    );
  }

  Widget _body(List<bridge.StorageEntry>? entries) {
    if (_error != null) {
      return Center(
        child: Padding(
          padding: const EdgeInsets.all(22),
          child: Text(
            _error!,
            textAlign: TextAlign.center,
            style: const TextStyle(fontSize: 13, color: AppColors.danger),
          ),
        ),
      );
    }
    if (entries == null) {
      return const Center(
        child: CircularProgressIndicator(strokeWidth: 2),
      );
    }
    if (entries.isEmpty) {
      return const Center(
        child: Text(
          'Nothing downloaded yet.',
          style: TextStyle(fontSize: 13, color: AppColors.textDim),
        ),
      );
    }
    final total = _totalBytes;
    return RefreshIndicator(
      onRefresh: _load,
      child: ListView.separated(
        padding: const EdgeInsets.fromLTRB(22, 0, 22, 24),
        itemCount: entries.length,
        separatorBuilder: (_, __) => const SizedBox(height: 8),
        itemBuilder: (context, i) => _EntryRow(
          entry: entries[i],
          fraction: total == 0 ? 0 : entries[i].bytes.toInt() / total,
        ),
      ),
    );
  }
}

class _EntryRow extends StatelessWidget {
  const _EntryRow({required this.entry, required this.fraction});

  final bridge.StorageEntry entry;
  final double fraction;

  /// The collection this entry belongs to, if the app's own state still
  /// claims it — see `collections::storage_breakdown`'s doc for when it
  /// doesn't (almost always a deleted collection's leftovers).
  void _openCollection(BuildContext context) {
    final collection = Collections.instance.byId(entry.collectionId!);
    if (collection == null) {
      showToast(context, 'Couldn\'t find that collection',
          severity: ToastSeverity.error);
      return;
    }
    Navigator.of(context).push(
      MaterialPageRoute(builder: (_) => CollectionScreen(collection: collection)),
    );
  }

  /// Opens the entry's real location in the OS's own file browser — Finder,
  /// Explorer, or whatever the Linux desktop registers for `file://`. A
  /// plain `Uri.file` launch is enough: for a directory (nearly every entry
  /// here — see the class doc) every platform's default handler is its file
  /// manager, the same trick `MediaViewerScreen._openExternally` already
  /// uses for a single downloaded file.
  Future<void> _reveal(BuildContext context) async {
    final ok = await launchUrl(Uri.file(entry.path));
    if (!ok && context.mounted) {
      showToast(context, 'Couldn\'t open ${entry.name}',
          severity: ToastSeverity.error);
    }
  }

  @override
  Widget build(BuildContext context) {
    final linked = entry.collectionId != null;
    return SurfaceCard(
      padding: const EdgeInsets.all(14),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              const Icon(Icons.folder_outlined,
                  size: 17, color: AppColors.textDim),
              const SizedBox(width: 10),
              Expanded(
                child: Text(
                  entry.name,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 13.5, fontWeight: FontWeight.w500),
                ),
              ),
              const SizedBox(width: 10),
              Text(
                formatBytes(entry.bytes.toInt()),
                style: monoLabel(size: 11.5, color: AppColors.signalSoft),
              ),
            ],
          ),
          const SizedBox(height: 6),
          Row(
            children: [
              Expanded(
                child: linked
                    ? InkWell(
                        onTap: () => _openCollection(context),
                        child: Row(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            const Icon(Icons.link,
                                size: 12, color: AppColors.signal),
                            const SizedBox(width: 5),
                            Flexible(
                              child: Text(
                                entry.collectionName!,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontSize: 11,
                                  fontWeight: FontWeight.w600,
                                  color: AppColors.signalSoft,
                                ),
                              ),
                            ),
                          ],
                        ),
                      )
                    : Row(
                        mainAxisSize: MainAxisSize.min,
                        children: [
                          const Icon(Icons.link_off,
                              size: 12, color: AppColors.textFaint),
                          const SizedBox(width: 5),
                          Text(
                            'Not linked to a collection',
                            style: monoLabel(
                                size: 10, color: AppColors.textFaint),
                          ),
                        ],
                      ),
              ),
              InkWell(
                onTap: () => _reveal(context),
                child: const Padding(
                  padding: EdgeInsets.only(left: 6),
                  child: Icon(Icons.open_in_new,
                      size: 13, color: AppColors.textDim),
                ),
              ),
            ],
          ),
          const SizedBox(height: 8),
          ClipRRect(
            borderRadius: BorderRadius.circular(99),
            child: LinearProgressIndicator(
              value: fraction.clamp(0.0, 1.0),
              minHeight: 4,
              backgroundColor: AppColors.borderStrong,
              valueColor: const AlwaysStoppedAnimation(AppColors.signal),
            ),
          ),
        ],
      ),
    );
  }
}
