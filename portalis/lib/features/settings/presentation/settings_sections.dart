import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../collections/application/collections_controller.dart';
import '../../../theme.dart';
import '../domain/engine_settings.dart';

/// Arranges setting sections according to the width they actually receive.
class SettingsSectionsLayout extends StatelessWidget {
  const SettingsSectionsLayout({super.key, required this.sections});

  final List<Widget> sections;

  @override
  Widget build(BuildContext context) => WindowBuilder(
        builder: (context, window) {
          if (!window.isSpacious) return Column(children: sections);
          final left = <Widget>[];
          final right = <Widget>[];
          for (var index = 0; index < sections.length; index++) {
            (index.isEven ? left : right).add(sections[index]);
          }
          return Row(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Expanded(child: Column(children: left)),
              Expanded(child: Column(children: right)),
            ],
          );
        },
      );
}

/// A truthful snapshot of known peer connectivity and engine configuration.
class SettingsHealthCard extends StatelessWidget {
  const SettingsHealthCard({
    super.key,
    required this.settings,
    required this.collections,
  });

  final EngineSettings settings;
  final CollectionsController collections;

  @override
  Widget build(BuildContext context) {
    final peers = collections.collections.fold<int>(
      0,
      (sum, collection) => sum + collection.livePeers,
    );
    final facts = [
      'PORT ${settings.listenPortStart}–${settings.listenPortEnd}',
      settings.disableDht ? 'DHT OFF' : 'DHT ON',
      plural(peers, 'PEER').toUpperCase(),
    ];

    return Padding(
      padding: const EdgeInsets.fromLTRB(kScreenGutter, 14, kScreenGutter, 0),
      child: SurfaceCard(
        padding: const EdgeInsets.all(16),
        glow: peers > 0 ? GlowLevel.calm : GlowLevel.none,
        child: Row(
          children: [
            Expanded(
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    peers > 0 ? 'Connected' : 'Idle',
                    style: AppText.cardTitle(),
                  ),
                  const SizedBox(height: 5),
                  Text(
                    facts.join(' · '),
                    style: monoLabel(
                      size: 10.5,
                      color: peers > 0
                          ? AppColors.signalMuted
                          : AppColors.textFaint,
                      letterSpacing: 0.4,
                    ),
                  ),
                ],
              ),
            ),
            Icon(
              peers > 0 ? Icons.check_circle_outline : Icons.circle_outlined,
              size: 20,
              color: peers > 0 ? AppColors.signal : AppColors.textGhost,
            ),
          ],
        ),
      ),
    );
  }
}
