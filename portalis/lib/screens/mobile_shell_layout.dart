import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../features/collections/domain/picked_file.dart';
import '../services/navigation.dart';
import '../theme.dart';
import 'home.dart';
import 'people.dart';
import 'settings.dart';
import 'user.dart';

/// Compact arrangement of the shared adaptive shell state.
class MobileShellLayout extends StatelessWidget {
  const MobileShellLayout({
    super.key,
    required this.index,
    required this.onSelected,
    required this.onOpen,
    required this.onShare,
    required this.onJoin,
  });

  final int index;
  final ValueChanged<int> onSelected;
  final ValueChanged<String> onOpen;
  final void Function([List<PickedFile>?]) onShare;
  final ValueChanged<String> onJoin;

  @override
  Widget build(BuildContext context) => Scaffold(
        backgroundColor: AppColors.surfaceDeep,
        body: AmbientBackground(
          intensity: Glow.intensityForRate(AppControllers.collections.liveRate),
          child: SafeArea(
            bottom: false,
            child: IndexedStack(
              index: index,
              children: [
                TickerMode(
                  enabled: index == 0,
                  child: Home(
                    onOpen: onOpen,
                    onShare: onShare,
                    onJoin: onJoin,
                  ),
                ),
                TickerMode(
                  enabled: index == 1,
                  child: const UserScreen(embedded: true),
                ),
                TickerMode(
                  enabled: index == 2,
                  child: const PeopleScreen(embedded: true),
                ),
                TickerMode(
                  enabled: index == 3,
                  child: const SettingsScreen(embedded: true),
                ),
              ],
            ),
          ),
        ),
        bottomNavigationBar: AppBottomNav(
          index: index,
          onSelected: onSelected,
        ),
      );
}

/// The four-destination bottom bar.
class AppBottomNav extends StatelessWidget {
  const AppBottomNav({
    super.key,
    required this.index,
    required this.onSelected,
  });

  final int index;
  final ValueChanged<int> onSelected;

  static const items = <({IconData? icon, String label})>[
    (icon: null, label: 'Home'),
    (icon: Icons.person_outline, label: 'User'),
    (icon: Icons.people_outline, label: 'People'),
    (icon: Icons.tune, label: 'Settings'),
  ];

  @override
  Widget build(BuildContext context) => Container(
        decoration: BoxDecoration(
          color: AppColors.surfaceDeep,
          border: Border(top: BorderSide(color: AppColors.border)),
        ),
        child: SafeArea(
          top: false,
          child: Padding(
            padding: const EdgeInsets.fromLTRB(8, 8, 8, 8),
            child: Row(
              children: [
                for (var i = 0; i < items.length; i++)
                  Expanded(
                    child: InkWell(
                      key: Key('navTab$i'),
                      borderRadius: BorderRadius.circular(AppRadius.inner),
                      onTap: () => i == 0
                          ? AppNavigation.goHome()
                          : onSelected(i),
                      child: Padding(
                        padding: const EdgeInsets.symmetric(vertical: 6),
                        child: Column(
                          mainAxisSize: MainAxisSize.min,
                          children: [
                            if (items[i].icon == null)
                              Opacity(
                                opacity: i == index ? 1 : 0.45,
                                child: const PortalisLogo(size: 24),
                              )
                            else
                              Icon(
                                items[i].icon,
                                size: 24,
                                color: i == index
                                    ? AppColors.text
                                    : AppColors.textGhost,
                              ),
                            const SizedBox(height: 6),
                            Text(
                              items[i].label,
                              style: AppText.caption(
                                color: i == index
                                    ? AppColors.text
                                    : AppColors.textGhost,
                                weight: i == index
                                    ? FontWeight.w600
                                    : FontWeight.w400,
                              ),
                            ),
                          ],
                        ),
                      ),
                    ),
                  ),
              ],
            ),
          ),
        ),
      );
}
