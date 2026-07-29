import 'package:flutter/material.dart';
import 'theme.dart';

class MediaItem {
  const MediaItem({required this.label});

  final String label;
}

class Collaborator {
  const Collaborator({
    required this.initials,
    required this.name,
    this.isAdmin = false,
    this.device = 'iPhone',
    this.upSpeed = '0 KB/s',
    this.downSpeed = '0 KB/s',
    this.percentComplete = 0,
    this.piecesHeld = const <bool>[],
  });

  final String initials;
  final String name;
  final bool isAdmin;
  final String device;
  final String upSpeed;
  final String downSpeed;
  final int percentComplete;

  /// Which pieces (of the currently-open media item) this peer holds.
  final List<bool> piecesHeld;
}

class Collection {
  Collection({
    required this.name,
    required this.subtitle,
    required this.categories,
    required this.hueIndex,
    required this.copiesLabel,
    required this.collaboratorCount,
    required this.media,
    required this.collaborators,
  });

  final String name;
  final String subtitle;
  final List<String> categories;
  final int hueIndex;
  final String copiesLabel;
  final int collaboratorCount;
  final List<MediaItem> media;
  final List<Collaborator> collaborators;

  Color get hue => AppColors.hueAt(hueIndex);
}

/// In-memory mock data standing in for a real swarm/collection backend.
class MockData {
  MockData._();

  static final collections = <Collection>[
    Collection(
      name: 'Iceland 2024',
      subtitle: '212 photos · 4 videos',
      categories: const ['📷 216', '📍 Iceland', '🗓 Jun 2024'],
      hueIndex: 0,
      copiesLabel: '6 copies alive',
      collaboratorCount: 100,
      media: List.generate(9, (i) => MediaItem(label: 'IMG_${1000 + i}')),
      collaborators: _mockCollaborators(),
    ),
    Collection(
      name: 'Family Reunion',
      subtitle: '84 photos · 12 videos',
      categories: const ['📷 96', '📍 Home', '🗓 Dec 2024'],
      hueIndex: 1,
      copiesLabel: '4 copies alive',
      collaboratorCount: 18,
      media: List.generate(6, (i) => MediaItem(label: 'DSC_${2000 + i}')),
      collaborators: _mockCollaborators(count: 18),
    ),
    Collection(
      name: 'Band Practice',
      subtitle: '3 videos',
      categories: const ['🎥 3', '🗓 Jan 2025'],
      hueIndex: 2,
      copiesLabel: '2 copies alive',
      collaboratorCount: 5,
      media: List.generate(3, (i) => MediaItem(label: 'CLIP_${i + 1}')),
      collaborators: _mockCollaborators(count: 5),
    ),
    Collection(
      name: 'Studio Shoot',
      subtitle: '540 photos',
      categories: const ['📷 540', '🗓 Mar 2025'],
      hueIndex: 3,
      copiesLabel: '9 copies alive',
      collaboratorCount: 34,
      media: List.generate(9, (i) => MediaItem(label: 'RAW_${3000 + i}')),
      collaborators: _mockCollaborators(count: 34),
    ),
  ];

  static List<Collaborator> _mockCollaborators({int count = 100}) {
    const names = [
      'Maya',
      'Theo',
      'Priya',
      'Sam',
      'Nora',
      'Kenji',
      'Ilse',
      'Milo',
    ];
    return List.generate(names.length.clamp(0, count), (i) {
      final name = names[i % names.length];
      return Collaborator(
        initials: name[0],
        name: name,
        isAdmin: i == 0,
        device: i.isEven ? 'iPhone' : 'MacBook',
        upSpeed: '${(i + 1) * 120}KB/s',
        downSpeed: '${(i + 2) * 80}KB/s',
        percentComplete: (30 + i * 7) % 100,
        piecesHeld: List.generate(24, (p) => (p + i) % 3 != 0),
      );
    });
  }

  /// Aggregate piece availability heatmap for a media item's swarm.
  static List<Color> aggregatePieceHeatmap(Collection collection) {
    return List.generate(30, (i) {
      final copies = 1 + ((i * 7 + collection.hueIndex) % 6);
      final opacity = (0.35 + (copies / 6) * 0.65).clamp(0.0, 1.0);
      return collection.hue.withValues(alpha: opacity);
    });
  }

  static List<Color> pieceStrip(Collaborator c) {
    return c.piecesHeld
        .map((held) => held
            ? AppColors.accent
            : AppColors.borderStrong)
        .toList();
  }
}
