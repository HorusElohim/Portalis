import 'package:flutter/material.dart';
import '../models.dart';
import '../theme.dart';
import '../widgets/common.dart';

class PeerScreen extends StatelessWidget {
  const PeerScreen({super.key, required this.collaborator, required this.media});

  final Collaborator collaborator;
  final MediaItem media;

  @override
  Widget build(BuildContext context) {
    final stats = [
      ('Downloaded', '${(collaborator.percentComplete * 1.3).round()} MB'),
      ('Uploaded', collaborator.upSpeed),
      ('Share ratio', '${(collaborator.percentComplete / 40).toStringAsFixed(1)}x'),
      ('Connected', '${12 + collaborator.percentComplete % 40} min'),
    ];
    final pieceColors = collaborator.piecesHeld
        .map((held) => held ? AppColors.accent : AppColors.borderStrong)
        .toList();

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: SingleChildScrollView(
          child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: NavBackButton(onTap: () => Navigator.of(context).pop()),
            ),
            const SizedBox(height: 4),
            Avatar(initials: collaborator.initials, size: 56),
            const SizedBox(height: 6),
            Text(
              collaborator.name,
              style: const TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
            ),
            const SizedBox(height: 2),
            Text(
              '${collaborator.device} · online now',
              style: const TextStyle(fontSize: 11, color: AppColors.neutral400),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  SectionLabel('THEIR PIECES OF ${media.label}'),
                  const SizedBox(height: 8),
                  PieceStrip(colors: pieceColors),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: GridView.count(
                shrinkWrap: true,
                physics: const NeverScrollableScrollPhysics(),
                crossAxisCount: 2,
                mainAxisSpacing: 8,
                crossAxisSpacing: 8,
                childAspectRatio: 2.1,
                children: [
                  for (final (label, value) in stats) _StatCard(label, value),
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

class _StatCard extends StatelessWidget {
  const _StatCard(this.label, this.value);

  final String label;
  final String value;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
      decoration: BoxDecoration(
        color: AppColors.surface,
        border: Border.all(color: AppColors.border),
        borderRadius: BorderRadius.circular(8),
      ),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisAlignment: MainAxisAlignment.center,
        children: [
          Text(
            label,
            style: const TextStyle(
              fontSize: 9,
              fontFamily: 'monospace',
              color: AppColors.neutral400,
            ),
          ),
          const SizedBox(height: 3),
          Text(
            value,
            style: const TextStyle(
              fontSize: 15,
              fontWeight: FontWeight.w500,
              color: AppColors.accent300,
            ),
          ),
        ],
      ),
    );
  }
}
