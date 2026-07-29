import 'package:flutter/material.dart';
import 'package:url_launcher/url_launcher.dart';

import '../models.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'swarm_screen.dart';

class MediaViewerScreen extends StatelessWidget {
  const MediaViewerScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  final Collection collection;
  final MediaItem media;

  Future<void> _open(BuildContext context) async {
    final path = media.localPath;
    if (path == null) return;
    final ok = await launchUrl(Uri.file(path));
    if (!ok && context.mounted) {
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Couldn\'t open ${media.label}')),
      );
    }
  }

  @override
  Widget build(BuildContext context) {
    final peerLabel = collection.collaboratorCount == 1
        ? '1 peer'
        : '${collection.collaboratorCount} peers';

    return Scaffold(
      backgroundColor: AppColors.viewerBg,
      body: SafeArea(
        child: Column(
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 10, 14, 0),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  _CircleButton(
                    icon: Icons.close,
                    onTap: () => Navigator.of(context).pop(),
                  ),
                  PillButton(
                    label: 'Details',
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute(
                        builder: (_) => SwarmScreen(
                          collection: collection,
                          media: media,
                        ),
                      ),
                    ),
                  ),
                ],
              ),
            ),
            Expanded(
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                child: Column(
                  children: [
                    Expanded(
                      child: Center(
                        child: AspectRatio(
                          aspectRatio: 16 / 10,
                          child: Stack(
                            alignment: Alignment.center,
                            children: [
                              MediaThumbnail(media: media, borderRadius: 8),
                              GestureDetector(
                                onTap: media.isReady ? () => _open(context) : null,
                                child: Container(
                                  width: 62,
                                  height: 62,
                                  decoration: BoxDecoration(
                                    shape: BoxShape.circle,
                                    color: AppColors.bg.withValues(alpha: 0.85),
                                    border: Border.all(color: AppColors.accent600),
                                  ),
                                  child: media.isReady
                                      ? const Icon(
                                          Icons.open_in_new_rounded,
                                          color: AppColors.accent300,
                                          size: 24,
                                        )
                                      : Center(
                                          child: Text(
                                            '${(media.progress * 100).toStringAsFixed(0)}%',
                                            style: const TextStyle(
                                              color: AppColors.accent300,
                                              fontSize: 13,
                                              fontWeight: FontWeight.w500,
                                            ),
                                          ),
                                        ),
                                ),
                              ),
                            ],
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(height: 14),
                    Text(
                      media.label,
                      style: const TextStyle(
                        fontSize: 15,
                        fontWeight: FontWeight.w500,
                      ),
                    ),
                    const SizedBox(height: 3),
                    Text(
                      media.isReady
                          ? '${collection.name} · streams from $peerLabel'
                          : '${collection.name} · downloading, $peerLabel connected',
                      style: const TextStyle(
                        fontSize: 11.5,
                        color: AppColors.neutral400,
                      ),
                    ),
                    const SizedBox(height: 14),
                  ],
                ),
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _CircleButton extends StatelessWidget {
  const _CircleButton({required this.icon, required this.onTap});

  final IconData icon;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.white.withValues(alpha: 0.06),
      shape: CircleBorder(side: BorderSide(color: AppColors.borderStrong)),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: SizedBox(
          width: 34,
          height: 34,
          child: Icon(icon, size: 18, color: AppColors.text),
        ),
      ),
    );
  }
}
