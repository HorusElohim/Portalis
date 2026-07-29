import 'package:flutter/material.dart';
import '../models.dart';
import '../theme.dart';
import '../widgets/common.dart';
import 'peer_screen.dart';

class SwarmScreen extends StatelessWidget {
  const SwarmScreen({
    super.key,
    required this.collection,
    required this.media,
  });

  final Collection collection;
  final MediaItem media;

  @override
  Widget build(BuildContext context) {
    final heatmap = MockData.aggregatePieceHeatmap(collection);
    final collaborators = collection.collaborators;
    final shown = collaborators.take(8).toList();
    final remaining = collaborators.length - shown.length;

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(4, 0, 0, 0),
                child: NavBackButton(onTap: () => Navigator.of(context).pop()),
              ),
            ),
            Padding(
              padding: const EdgeInsets.symmetric(horizontal: 20),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: [
                  Text(
                    '${media.label} — swarm',
                    style: const TextStyle(
                      fontSize: 17,
                      fontWeight: FontWeight.w500,
                    ),
                  ),
                  const SizedBox(height: 8),
                  PieceStrip(colors: heatmap),
                  const SizedBox(height: 6),
                  Row(
                    children: const [
                      Expanded(
                        child: Text(
                          '30 pieces · darker = fewer copies',
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: 9.5,
                            fontFamily: 'monospace',
                            color: AppColors.neutral400,
                          ),
                        ),
                      ),
                      SizedBox(width: 8),
                      Flexible(
                        child: Text(
                          '↑2.1M ↓860K · 14 seeding',
                          textAlign: TextAlign.right,
                          overflow: TextOverflow.ellipsis,
                          style: TextStyle(
                            fontSize: 9.5,
                            fontFamily: 'monospace',
                            color: AppColors.neutral400,
                          ),
                        ),
                      ),
                    ],
                  ),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 14, 20, 6),
              child: Align(
                alignment: Alignment.centerLeft,
                child: SectionLabel('COLLABORATORS · ${collaborators.length}'),
              ),
            ),
            Expanded(
              child: ListView.builder(
                padding: const EdgeInsets.symmetric(horizontal: 12),
                itemCount: shown.length + 1,
                itemBuilder: (context, index) {
                  if (index == shown.length) {
                    return Padding(
                      padding: const EdgeInsets.all(8),
                      child: Text(
                        '+ $remaining more — list scrolls, stats stream in per page',
                        textAlign: TextAlign.center,
                        style: const TextStyle(
                          fontSize: 10,
                          fontFamily: 'monospace',
                          color: AppColors.neutral500,
                        ),
                      ),
                    );
                  }
                  final cb = shown[index];
                  return Material(
                    color: Colors.transparent,
                    child: InkWell(
                      borderRadius: BorderRadius.circular(6),
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute(
                          builder: (_) => PeerScreen(
                            collaborator: cb,
                            media: media,
                          ),
                        ),
                      ),
                      child: Padding(
                        padding:
                            const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                        child: Row(
                          children: [
                            Avatar(initials: cb.initials, size: 24),
                            const SizedBox(width: 9),
                            SizedBox(
                              width: 74,
                              child: Text(
                                cb.name,
                                maxLines: 1,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(fontSize: 12),
                              ),
                            ),
                            if (cb.isAdmin) ...[
                              Container(
                                margin: const EdgeInsets.only(right: 6),
                                padding: const EdgeInsets.symmetric(
                                    horizontal: 4, vertical: 1),
                                decoration: BoxDecoration(
                                  border:
                                      Border.all(color: AppColors.accent600),
                                  borderRadius: BorderRadius.circular(3),
                                ),
                                child: const Text(
                                  'adm',
                                  style: TextStyle(
                                    fontSize: 8,
                                    fontFamily: 'monospace',
                                    color: AppColors.accent300,
                                  ),
                                ),
                              ),
                            ],
                            Expanded(
                              child: PieceStrip(
                                colors: MockData.pieceStrip(cb),
                                height: 8,
                              ),
                            ),
                            SizedBox(
                              width: 86,
                              child: Text(
                                '↑${cb.upSpeed} ↓${cb.downSpeed} · ${cb.percentComplete}%',
                                textAlign: TextAlign.right,
                                style: const TextStyle(
                                  fontSize: 9,
                                  fontFamily: 'monospace',
                                  color: AppColors.neutral400,
                                ),
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  );
                },
              ),
            ),
          ],
        ),
      ),
    );
  }
}
