import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../theme.dart';

/// The local device identity and its current connected-peer count.
class DesktopIdentityChip extends StatefulWidget {
  const DesktopIdentityChip({
    super.key,
    required this.selected,
    required this.onTap,
  });

  final bool selected;
  final VoidCallback onTap;

  @override
  State<DesktopIdentityChip> createState() => _DesktopIdentityChipState();
}

class _DesktopIdentityChipState extends State<DesktopIdentityChip> {
  @override
  void initState() {
    super.initState();
    AppControllers.identity.load();
  }

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: Listenable.merge([
          AppControllers.identity,
          AppControllers.collections,
        ]),
        builder: (context, _) {
          final name = AppControllers.identity.info?.nickname;
          final peers = AppControllers.collections.collections
              .fold<int>(0, (sum, collection) => sum + collection.livePeers);
          final initials = name == null || name.isEmpty ? '-' : name[0].toUpperCase();

          return Material(
            color: widget.selected ? AppColors.surfaceRaised : Colors.transparent,
            borderRadius: BorderRadius.circular(AppRadius.inner),
            child: InkWell(
              key: const Key('identityChip'),
              borderRadius: BorderRadius.circular(AppRadius.inner),
              onTap: widget.onTap,
              child: Padding(
                padding: const EdgeInsets.symmetric(horizontal: 8, vertical: 6),
                child: Row(
                  mainAxisSize: MainAxisSize.min,
                  children: [
                    Avatar(initials: initials, size: 30, primary: true),
                    const SizedBox(width: 10),
                    ConstrainedBox(
                      constraints: const BoxConstraints(maxWidth: 150),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            name ?? 'This device',
                            overflow: TextOverflow.ellipsis,
                            style: AppText.body(weight: FontWeight.w600),
                          ),
                          Text(
                            peers == 0 ? 'NO PEERS' : plural(peers, 'PEER').toUpperCase(),
                            style: monoLabel(
                              size: 9.5,
                              color: peers > 0 ? AppColors.signal : AppColors.textFaint,
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
        },
      );
}
