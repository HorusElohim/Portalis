import 'package:flutter/material.dart';
import '../models.dart';
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

/// Real per-file and per-torrent stats for the media currently open in
/// [MediaViewerScreen] — file size/progress plus the swarm's actual
/// transfer rates, state, peer count, and info hash. Replaces the
/// mockup's fabricated piece heatmap and per-peer list: real per-peer
/// identity isn't modeled yet (a torrent peer is just an IP:port, see
/// `TorrentCollections`), so this shows only what's genuinely known
/// rather than inventing collaborator names/avatars for real data.
class MediaDetailsScreen extends StatelessWidget {
  const MediaDetailsScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  final Collection collection;
  final MediaItem media;

  @override
  Widget build(BuildContext context) {
    final peerLabel = collection.livePeers == 1
        ? '1 peer connected'
        : '${collection.livePeers} peers connected';

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: PageBody(
          child: SingleChildScrollView(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Align(
                  alignment: Alignment.centerLeft,
                  child: NavBackButton(onTap: () => Navigator.of(context).pop()),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 0, 20, 6),
                  child: Text(
                    media.label,
                    style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w500),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 10, 20, 0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const SectionLabel('FILE'),
                      const SizedBox(height: 8),
                      _InfoRow(
                        label: 'Size',
                        value: media.sizeBytes > 0
                            ? _formatBytes(media.sizeBytes)
                            : 'Unknown',
                      ),
                      _InfoRow(
                        label: 'Downloaded',
                        value: media.sizeBytes > 0
                            ? '${_formatBytes(media.downloadedBytes)} of ${_formatBytes(media.sizeBytes)} · ${(media.progress * 100).toStringAsFixed(0)}%'
                            : '${(media.progress * 100).toStringAsFixed(0)}%',
                      ),
                      const SizedBox(height: 6),
                      ClipRRect(
                        borderRadius: BorderRadius.circular(99),
                        child: LinearProgressIndicator(
                          value: media.progress.clamp(0.0, 1.0),
                          minHeight: 4,
                          backgroundColor: AppColors.borderStrong,
                          valueColor: AlwaysStoppedAnimation(collection.hue),
                        ),
                      ),
                    ],
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 20, 20, 0),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      const SectionLabel('TORRENT'),
                      const SizedBox(height: 8),
                      _InfoRow(
                        label: 'Collection',
                        value: collection.name,
                      ),
                      _InfoRow(
                        label: 'State',
                        value: collection.state.isEmpty ? 'Unknown' : collection.state,
                      ),
                      _InfoRow(
                        label: 'Download speed',
                        value: _formatMbps(collection.downloadMbps),
                      ),
                      _InfoRow(
                        label: 'Upload speed',
                        value: _formatMbps(collection.uploadMbps),
                      ),
                      _InfoRow(label: 'Peers', value: peerLabel),
                      // The info hash of the *torrent this file came from*.
                      // A shared collection has one per manifest entry, so it
                      // belongs to the media item, not the collection.
                      if (media.infoHash.isNotEmpty)
                        _InfoRow(
                          label: 'Info hash',
                          value: media.infoHash,
                          monospace: true,
                        ),
                    ],
                  ),
                ),
                const SizedBox(height: 24),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _InfoRow extends StatelessWidget {
  const _InfoRow({
    required this.label,
    required this.value,
    this.monospace = false,
  });

  final String label;
  final String value;
  final bool monospace;

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
        ],
      ),
    );
  }
}
