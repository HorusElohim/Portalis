import 'package:flutter/material.dart';
import 'theme.dart';

class MediaItem {
  const MediaItem({
    required this.label,
    this.localPath,
    this.progress = 1.0,
    this.sizeBytes = 0,
    this.downloadedBytes = 0,
  });

  final String label;

  /// Absolute path once fully downloaded — null for mock data or while
  /// still in progress. Set by [TorrentCollections] for real torrents.
  final String? localPath;

  /// 0.0..=1.0. Mock media is always "complete" since there's nothing
  /// downloading; real torrent-backed media reports its actual progress.
  final double progress;

  /// Real byte counts from the torrent engine, for the details panel's
  /// "X of Y downloaded" display. Zero for mock data.
  final int sizeBytes;
  final int downloadedBytes;

  bool get isReady => localPath != null && progress >= 1.0;
}

class Collaborator {
  const Collaborator({
    required this.initials,
    required this.name,
    this.isAdmin = false,
  });

  final String initials;
  final String name;
  final bool isAdmin;
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
    this.progress = 1.0,
    this.downloadedBytes = 0,
    this.uploadedBytes = 0,
    this.downloadMbps = 0.0,
    this.uploadMbps = 0.0,
    this.state = '',
    this.infoHash = '',
  });

  final String name;
  final String subtitle;
  final List<String> categories;
  final int hueIndex;
  final String copiesLabel;
  final int collaboratorCount;
  final List<MediaItem> media;
  final List<Collaborator> collaborators;

  /// Overall download progress, 0.0..=1.0. Mock collections are always
  /// "complete"; real torrent-backed ones report actual progress — drives
  /// Home's perimeter progress ring.
  final double progress;

  /// Real byte counters from the torrent engine, used for the User screen's
  /// aggregate "Shared"/"Received" stats. Zero for mock data.
  final int downloadedBytes;
  final int uploadedBytes;

  /// Real live transfer rates and torrent state, for the media details
  /// panel. Zero/empty for mock data.
  final double downloadMbps;
  final double uploadMbps;
  final String state;
  final String infoHash;

  Color get hue => AppColors.hueAt(hueIndex);
}
