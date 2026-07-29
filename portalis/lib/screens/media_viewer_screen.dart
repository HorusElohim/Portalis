import 'package:flutter/material.dart';
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

  @override
  Widget build(BuildContext context) {
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
                              const PlaceholderTile(borderRadius: 8),
                              Container(
                                width: 62,
                                height: 62,
                                decoration: BoxDecoration(
                                  shape: BoxShape.circle,
                                  color: AppColors.bg.withValues(alpha: 0.85),
                                  border: Border.all(color: AppColors.accent600),
                                ),
                                child: const Icon(
                                  Icons.play_arrow_rounded,
                                  color: AppColors.accent300,
                                  size: 28,
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
                      '${collection.name} · streams from 6 peers',
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
