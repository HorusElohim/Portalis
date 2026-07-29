import 'package:flutter/material.dart';
import 'theme.dart';

class MediaItem {
  const MediaItem({
    required this.label,
    this.localPath,
    this.progress = 1.0,
  });

  final String label;

  /// Absolute path once fully downloaded — null for mock data or while
  /// still in progress. Set by [TorrentCollections] for real torrents.
  final String? localPath;

  /// 0.0..=1.0. Mock media is always "complete" since there's nothing
  /// downloading; real torrent-backed media reports its actual progress.
  final double progress;

  bool get isReady => localPath != null && progress >= 1.0;
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

/// Rendering helpers shared by the swarm/peer screens — generic over any
/// [Collection]/[Collaborator], real or mock.
class SwarmVisuals {
  SwarmVisuals._();

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
