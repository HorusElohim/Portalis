import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../widgets/common.dart';

String _formatBytes(int bytes) {
  const gb = 1000000000;
  const mb = 1000000;
  const kb = 1000;
  if (bytes >= gb) return '${(bytes / gb).toStringAsFixed(2)} GB';
  if (bytes >= mb) return '${(bytes / mb).toStringAsFixed(1)} MB';
  if (bytes >= kb) return '${(bytes / kb).toStringAsFixed(0)} KB';
  return '$bytes B';
}

String _formatMbps(double mibPerSec) => '${mibPerSec.toStringAsFixed(2)} MiB/s';

/// Everything the backend actually knows about one collection — the
/// collection-level counterpart to [MediaDetailsScreen].
///
/// Every row is read from the live [Collection], which is re-read from
/// [Collections] on each rebuild so transfer figures tick while the screen is
/// open. Nothing here is computed for display alone: where a fact isn't
/// modelled (a plain torrent has no collaborators, an unfetched entry has no
/// byte counts yet) the row is omitted or says so, rather than showing a
/// plausible-looking zero.
class CollectionDetailsScreen extends StatelessWidget {
  const CollectionDetailsScreen({super.key, required this.collectionId});

  /// Held by id, not by value: sync/fetch change the collection while this
  /// screen is open.
  final String collectionId;

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: PageBody(
          child: ListenableBuilder(
            listenable: Collections.instance,
            builder: (context, _) {
              final collection = Collections.instance.byId(collectionId);
              if (collection == null) {
                // Deleted from under us (or the backend isn't up in tests).
                return Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    NavBackButton(onTap: () => Navigator.of(context).pop()),
                    const Expanded(
                      child: Center(
                        child: Text(
                          'This collection is no longer available.',
                          style: TextStyle(
                              fontSize: 12, color: AppColors.neutral400),
                        ),
                      ),
                    ),
                  ],
                );
              }
              return SingleChildScrollView(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Align(
                      alignment: Alignment.centerLeft,
                      child:
                          NavBackButton(onTap: () => Navigator.of(context).pop()),
                    ),
                    Padding(
                      padding: const EdgeInsets.fromLTRB(20, 0, 20, 6),
                      child: Text(
                        collection.name,
                        style: const TextStyle(
                            fontSize: 17, fontWeight: FontWeight.w500),
                      ),
                    ),
                    _Section(
                      label: 'COLLECTION',
                      children: [
                        _InfoRow(
                          label: 'Type',
                          value: collection.isShared
                              ? 'Shared — invite-based, can grow'
                              : 'Torrent — fixed contents',
                        ),
                        _InfoRow(label: 'State', value: collection.state),
                        _InfoRow(
                          label: collection.isShared ? 'Collection id' : 'Info hash',
                          value: collection.id,
                          monospace: true,
                          copyable: true,
                        ),
                      ],
                    ),
                    _Section(
                      label: 'TRANSFER',
                      children: [
                        _InfoRow(
                          label: 'Progress',
                          value: collection.totalBytes > 0
                              ? '${_formatBytes(collection.downloadedBytes)} of '
                                  '${_formatBytes(collection.totalBytes)} · '
                                  '${(collection.progress * 100).toStringAsFixed(0)}%'
                              // No fetched torrent means no metadata, so no
                              // total is knowable yet — say that instead of "0 B
                              // of 0 B".
                              : 'Nothing fetched yet',
                        ),
                        _InfoRow(
                          label: 'Uploaded',
                          value: _formatBytes(collection.uploadedBytes),
                        ),
                        _InfoRow(
                          label: 'Down / up',
                          value: '${_formatMbps(collection.downloadMbps)}'
                              ' · ${_formatMbps(collection.uploadMbps)}',
                        ),
                        _InfoRow(
                          label: 'Peers',
                          value: collection.livePeers == 1
                              ? '1 peer connected'
                              : '${collection.livePeers} peers connected',
                        ),
                        const SizedBox(height: 6),
                        ClipRRect(
                          borderRadius: BorderRadius.circular(99),
                          child: LinearProgressIndicator(
                            value: collection.progress.clamp(0.0, 1.0),
                            minHeight: 4,
                            backgroundColor: AppColors.borderStrong,
                            valueColor: AlwaysStoppedAnimation(collection.hue),
                          ),
                        ),
                      ],
                    ),
                    _EntriesSection(collection: collection),
                    if (collection.isShared)
                      _CollaboratorsSection(collection: collection),
                    const SizedBox(height: 24),
                  ],
                ),
              );
            },
          ),
        ),
      ),
    );
  }
}

/// One manifest entry per row — the unit a shared collection grows by. For a
/// plain torrent there is exactly one, which is the honest picture: its
/// contents are fixed at creation.
class _EntriesSection extends StatelessWidget {
  const _EntriesSection({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    final entries = collection.entries;
    return _Section(
      label: 'CONTENTS · ${entries.length} '
          'ENTR${entries.length == 1 ? 'Y' : 'IES'}',
      children: [
        if (entries.isEmpty)
          const Text(
            'Nothing added yet.',
            style: TextStyle(fontSize: 12, color: AppColors.neutral400),
          )
        else
          for (final entry in entries)
            Padding(
              padding: const EdgeInsets.only(bottom: 10),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Row(
                    children: [
                      Icon(
                        entry.fetched
                            ? Icons.check_circle_outline
                            : Icons.cloud_download_outlined,
                        size: 13,
                        color: entry.fetched
                            ? collection.hue
                            : AppColors.neutral400,
                      ),
                      const SizedBox(width: 6),
                      Expanded(
                        child: Text(
                          entry.label,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(fontSize: 12.5),
                        ),
                      ),
                      Text(
                        entry.fetched
                            ? '${entry.media.length} file'
                                '${entry.media.length == 1 ? '' : 's'} · '
                                '${_formatBytes(entry.totalBytes)}'
                            : 'not fetched',
                        style: const TextStyle(
                          fontSize: 10.5,
                          fontFamily: 'monospace',
                          color: AppColors.neutral400,
                        ),
                      ),
                    ],
                  ),
                  Padding(
                    padding: const EdgeInsets.only(left: 19, top: 2),
                    child: Text(
                      entry.infoHash,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 9.5,
                        fontFamily: 'monospace',
                        color: AppColors.neutral500,
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

class _CollaboratorsSection extends StatelessWidget {
  const _CollaboratorsSection({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    final collaborators = collection.collaborators;
    return _Section(
      label: 'COLLABORATORS · ${collaborators.length}',
      children: [
        if (collaborators.isEmpty)
          const Text(
            'No collaborators recorded yet — sync with someone who has this '
            'collection to exchange lists.',
            style: TextStyle(fontSize: 12, color: AppColors.neutral400),
          )
        else
          for (final c in collaborators)
            Padding(
              padding: const EdgeInsets.only(bottom: 8),
              child: Row(
                children: [
                  Avatar(initials: c.initials, size: 24),
                  const SizedBox(width: 10),
                  Expanded(
                    child: Column(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Text(
                          c.isAdmin ? '${c.name} · admin' : c.name,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(fontSize: 12.5),
                        ),
                        Text(
                          c.deviceId,
                          overflow: TextOverflow.ellipsis,
                          style: const TextStyle(
                            fontSize: 9.5,
                            fontFamily: 'monospace',
                            color: AppColors.neutral500,
                          ),
                        ),
                      ],
                    ),
                  ),
                ],
              ),
            ),
      ],
    );
  }
}

class _Section extends StatelessWidget {
  const _Section({required this.label, required this.children});

  final String label;
  final List<Widget> children;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SectionLabel(label),
          const SizedBox(height: 8),
          ...children,
        ],
      ),
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({
    required this.label,
    required this.value,
    this.monospace = false,
    this.copyable = false,
  });

  final String label;
  final String value;
  final bool monospace;
  final bool copyable;

  @override
  Widget build(BuildContext context) {
    return Padding(
      padding: const EdgeInsets.symmetric(vertical: 5),
      child: Row(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          SizedBox(
            width: 110,
            child: Text(
              label,
              style: const TextStyle(fontSize: 12, color: AppColors.neutral400),
            ),
          ),
          Expanded(
            child: Text(
              value,
              style: TextStyle(
                fontSize: 12,
                fontFamily: monospace ? 'monospace' : null,
              ),
            ),
          ),
          if (copyable)
            InkWell(
              onTap: () {
                Clipboard.setData(ClipboardData(text: value));
                ScaffoldMessenger.of(context).showSnackBar(
                  SnackBar(content: Text('$label copied')),
                );
              },
              child: const Padding(
                padding: EdgeInsets.only(left: 6),
                child:
                    Icon(Icons.copy, size: 13, color: AppColors.neutral400),
              ),
            ),
        ],
      ),
    );
  }
}
