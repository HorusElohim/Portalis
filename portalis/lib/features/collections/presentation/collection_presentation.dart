import 'package:flutter/material.dart';

import '../../../theme.dart';
import '../../../design/formatters.dart';
import '../domain/collection.dart';

/// Rendering facts derived from a collection. Keeping them outside the domain
/// model lets design change without changing controllers or native mappings.
extension CollectionPresentation on Collection {
  String? get etaLabel {
    final seconds = etaSecs;
    return seconds == null ? null : '${formatEta(seconds)} left';
  }

  GlowLevel get glow {
    if (downloadMbps > 0 || uploadMbps > 0) {
      return (downloadMbps + uploadMbps) > 4
          ? GlowLevel.vivid
          : GlowLevel.active;
    }
    if (isConnecting) return GlowLevel.calm;
    return isSharing ? GlowLevel.calm : GlowLevel.none;
  }

  double get liveIntensity =>
      Glow.intensityForRate(downloadMbps + uploadMbps);

  Color get hue => AppColors.hueAt(id.hashCode.abs());

  String get subtitle {
    final count = media.length;
    final items = '$count item${count == 1 ? '' : 's'}';
    if (isConnecting) return '$items · looking for a peer';
    final eta = etaLabel;
    if (eta != null) return '$items · $eta';
    return pendingMedia > 0 ? '$items · $pendingMedia to fetch' : items;
  }

  String get peersLabel => '$livePeers peer${livePeers == 1 ? '' : 's'}';

  String get copiesLabel {
    if (ingestion != null) {
      return ingestion!.failed ? 'Import failed' : 'Preparing locally in Rust';
    }
    if (!isComplete) {
      final eta = etaLabel;
      final completed = formatProgressPercent(progress);
      return eta == null
          ? '$completed · $peersLabel'
          : '$completed · $eta · $peersLabel';
    }
    return livePeers == 0
        ? 'Seeding · this device'
        : 'Seeding · this device + $peersLabel';
  }
}
