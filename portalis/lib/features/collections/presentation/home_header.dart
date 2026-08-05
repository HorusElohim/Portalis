import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../identity/application/identity_controller.dart';
import '../application/collections_controller.dart';

/// Identity and one compact, non-duplicated collection-health summary.
class HomeHeader extends StatelessWidget {
  const HomeHeader({
    super.key,
    required this.identity,
    required this.collections,
  });

  final IdentityController identity;
  final CollectionsController collections;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: identity,
        builder: (context, _) {
          final nickname = identity.info?.nickname;
          final initials = nickname == null || nickname.isEmpty
              ? '·'
              : nickname[0].toUpperCase();
          final all = collections.collections;
          final active = all.where((collection) => collection.isMoving).toList();
          final peers = all.fold<int>(
            0,
            (sum, collection) => sum + collection.livePeers,
          );
          final down = all.fold<double>(
            0,
            (sum, collection) => sum + collection.downloadMbps,
          );
          final up = all.fold<double>(
            0,
            (sum, collection) => sum + collection.uploadMbps,
          );
          final status = <String>[
            if (active.isNotEmpty) plural(active.length, 'active transfer'),
            if (down > 0) '↓ ${formatRate(down)}',
            if (up > 0) '↑ ${formatRate(up)}',
            if (active.isEmpty && peers > 0)
              '$peers peer${peers == 1 ? '' : 's'} connected',
          ];

          return Padding(
            padding: const EdgeInsets.fromLTRB(22, 14, 22, 0),
            child: Row(
              children: [
                Avatar(initials: initials, size: 34, primary: true),
                const SizedBox(width: 10),
                Expanded(
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.start,
                    children: [
                      Text('Portalis', style: displayText(size: 16)),
                      if (status.isNotEmpty)
                        Text(
                          status.join(' · '),
                          maxLines: 1,
                          overflow: TextOverflow.ellipsis,
                          style: monoLabel(
                            size: 10,
                            color: active.isEmpty
                                ? AppColors.textDim
                                : AppColors.signal,
                          ),
                        ),
                    ],
                  ),
                ),
              ],
            ),
          );
        },
      );
}
