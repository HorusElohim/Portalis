import 'package:flutter/material.dart';
import '../models.dart';
import '../services/torrent_collections.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'add_torrent_screen.dart';
import 'collection_screen.dart';

class HomeScreen extends StatelessWidget {
  const HomeScreen({super.key});

  @override
  Widget build(BuildContext context) {
    return Column(
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 14, 20, 10),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.spaceBetween,
            children: [
              const Text(
                'SmartShare',
                style: TextStyle(
                  fontSize: 21,
                  fontWeight: FontWeight.w500,
                  letterSpacing: -0.2,
                ),
              ),
              const Avatar(initials: 'M'),
            ],
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 0, 20, 12),
          child: Align(
            alignment: Alignment.centerLeft,
            child: Text(
              'Your collections',
              style: TextStyle(
                fontSize: 12.5,
                color: AppColors.neutral400,
              ),
            ),
          ),
        ),
        Expanded(
          child: ListenableBuilder(
            listenable: TorrentCollections.instance,
            builder: (context, _) {
              final collections = TorrentCollections.instance.collections;
              if (collections.isEmpty) {
                return const _EmptyCollections();
              }
              return GridView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 16),
                gridDelegate: const SliverGridDelegateWithFixedCrossAxisCount(
                  crossAxisCount: 2,
                  mainAxisSpacing: 10,
                  crossAxisSpacing: 10,
                  childAspectRatio: 0.78,
                ),
                itemCount: collections.length,
                itemBuilder: (context, index) {
                  final c = collections[index];
                  return _CollectionCard(
                    collection: c,
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute(
                        builder: (_) => CollectionScreen(collection: c),
                      ),
                    ),
                  );
                },
              );
            },
          ),
        ),
        Padding(
          padding: const EdgeInsets.fromLTRB(20, 12, 20, 18),
          child: Center(
            child: PillButton(
              label: '＋ Add torrent',
              onTap: () => Navigator.of(context).push(
                MaterialPageRoute(builder: (_) => const AddTorrentScreen()),
              ),
            ),
          ),
        ),
      ],
    );
  }
}

class _EmptyCollections extends StatelessWidget {
  const _EmptyCollections();

  @override
  Widget build(BuildContext context) {
    return Center(
      child: Padding(
        padding: const EdgeInsets.symmetric(horizontal: 40),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            const Icon(Icons.hub_outlined, size: 32, color: AppColors.neutral500),
            const SizedBox(height: 10),
            Text(
              'No collections yet — add a magnet link or a .torrent file to get started.',
              textAlign: TextAlign.center,
              style: TextStyle(fontSize: 12, color: AppColors.neutral400),
            ),
          ],
        ),
      ),
    );
  }
}

class _CollectionCard extends StatelessWidget {
  const _CollectionCard({required this.collection, required this.onTap});

  final Collection collection;
  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface,
      borderRadius: BorderRadius.circular(8),
      child: InkWell(
        borderRadius: BorderRadius.circular(8),
        onTap: onTap,
        child: Container(
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(8),
            border: Border.all(color: AppColors.border),
          ),
          clipBehavior: Clip.antiAlias,
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.stretch,
            children: [
              SizedBox(
                height: 92,
                child: collection.media.isEmpty
                    ? const PlaceholderTile()
                    : MediaThumbnail(media: collection.media.first),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(12, 10, 12, 12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      collection.name,
                      overflow: TextOverflow.ellipsis,
                      style: const TextStyle(
                        fontSize: 13.5,
                        fontWeight: FontWeight.w500,
                        height: 1.2,
                      ),
                    ),
                    const SizedBox(height: 5),
                    Text(
                      collection.subtitle,
                      style: const TextStyle(
                        fontSize: 11,
                        color: AppColors.neutral400,
                      ),
                    ),
                    const SizedBox(height: 2),
                    Padding(
                      padding: const EdgeInsets.only(top: 2),
                      child: CopiesIndicator(
                        color: collection.hue,
                        label: collection.copiesLabel,
                      ),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
