import 'package:flutter/material.dart';
import '../models.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'media_viewer_screen.dart';

class CollectionScreen extends StatelessWidget {
  const CollectionScreen({super.key, required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    final shownCollabs = collection.collaborators.take(6).toList();
    final remaining = collection.collaboratorCount - shownCollabs.length;

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          children: [
            Stack(
              children: [
                const SizedBox(
                  height: 190,
                  width: double.infinity,
                  child: PlaceholderTile(label: 'cover — chosen by admins'),
                ),
                Positioned(
                  top: 10,
                  left: 12,
                  child: _FloatingPill(
                    child: NavBackButton(
                      onTap: () => Navigator.of(context).pop(),
                    ),
                  ),
                ),
                Positioned(
                  right: 10,
                  bottom: 10,
                  child: _FloatingPill(
                    child: Padding(
                      padding:
                          const EdgeInsets.symmetric(horizontal: 11, vertical: 5),
                      child: Text(
                        '✎ cover · 2 admins',
                        style: TextStyle(
                          fontSize: 10,
                          fontFamily: 'monospace',
                          color: AppColors.neutral300,
                        ),
                      ),
                    ),
                  ),
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
                      const SizedBox(height: 14),
                      Text(
                        collection.name,
                        style: const TextStyle(
                          fontSize: 20,
                          fontWeight: FontWeight.w500,
                        ),
                      ),
                      const SizedBox(height: 6),
                      Wrap(
                        spacing: 10,
                        runSpacing: 4,
                        children: [
                          for (final cat in collection.categories)
                            Text(
                              cat,
                              style: const TextStyle(
                                fontSize: 10.5,
                                fontFamily: 'monospace',
                                color: AppColors.neutral300,
                              ),
                            ),
                        ],
                      ),
                      const SizedBox(height: 6),
                      CopiesIndicator(
                        color: collection.hue,
                        label: collection.copiesLabel,
                        fontSize: 12,
                      ),
                      const SizedBox(height: 14),
                      SectionLabel('COLLABORATORS · ${collection.collaboratorCount}'),
                      const SizedBox(height: 7),
                      SizedBox(
                        height: 27,
                        child: Row(
                          children: [
                            SizedBox(
                              width: 27.0 + (shownCollabs.length - 1) * 19,
                              child: Stack(
                                children: [
                                  for (var i = 0; i < shownCollabs.length; i++)
                                    Positioned(
                                      left: i * 19.0,
                                      child: DecoratedBox(
                                        decoration: const BoxDecoration(
                                          shape: BoxShape.circle,
                                        ),
                                        child: Container(
                                          decoration: BoxDecoration(
                                            shape: BoxShape.circle,
                                            border: Border.all(
                                              color: AppColors.bg,
                                              width: 2,
                                            ),
                                          ),
                                          child: Avatar(
                                            initials: shownCollabs[i].initials,
                                            size: 27,
                                          ),
                                        ),
                                      ),
                                    ),
                                ],
                              ),
                            ),
                            const SizedBox(width: 10),
                            Expanded(
                              child: Text(
                                '+$remaining keep this collection alive',
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
                      const SizedBox(height: 12),
                      GridView.builder(
                        shrinkWrap: true,
                        physics: const NeverScrollableScrollPhysics(),
                        gridDelegate:
                            const SliverGridDelegateWithFixedCrossAxisCount(
                          crossAxisCount: 3,
                          mainAxisSpacing: 8,
                          crossAxisSpacing: 8,
                          childAspectRatio: 1,
                        ),
                        itemCount: collection.media.length,
                        itemBuilder: (context, index) {
                          final m = collection.media[index];
                          return Material(
                            color: Colors.transparent,
                            borderRadius: BorderRadius.circular(6),
                            child: InkWell(
                              borderRadius: BorderRadius.circular(6),
                              onTap: () => Navigator.of(context).push(
                                MaterialPageRoute(
                                  builder: (_) => MediaViewerScreen(
                                    collection: collection,
                                    media: m,
                                  ),
                                ),
                              ),
                              child: MediaThumbnail(media: m, borderRadius: 6),
                            ),
                          );
                        },
                      ),
                      const SizedBox(height: 12),
                    ],
                  ),
                ),
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20, vertical: 12),
              child: Wrap(
                alignment: WrapAlignment.center,
                spacing: 10,
                runSpacing: 8,
                children: [
                  PillButton(
                    label: 'Add collab',
                    icon: const Icon(Icons.people_alt_outlined,
                        size: 16, color: AppColors.accent300),
                    onTap: () {},
                  ),
                  PillButton(label: '＋ Add media', dim: true, onTap: () {}),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}

class _FloatingPill extends StatelessWidget {
  const _FloatingPill({required this.child});

  final Widget child;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: BoxDecoration(
        color: AppColors.bg.withValues(alpha: 0.8),
        borderRadius: BorderRadius.circular(99),
        border: Border.all(color: AppColors.borderStrong),
      ),
      child: child,
    );
  }
}
