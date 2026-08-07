import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import '../design/design.dart';
import '../theme.dart';
import 'desktop_event_rail.dart';
import 'desktop_identity_chip.dart';
import 'desktop_navigation_action.dart';
import 'desktop_pane.dart';

/// Desktop chrome shared by every pane: identity, live events, navigation.
class DesktopTopBar extends StatelessWidget {
  const DesktopTopBar({super.key, required this.pane, required this.onPane});

  final DesktopPane pane;
  final ValueChanged<DesktopPane> onPane;

  @override
  Widget build(BuildContext context) => ListenableBuilder(
        listenable: AppControllers.collections,
        builder: (context, _) {
          final active = AppControllers.collections.collections
              .any((collection) => collection.isMoving);
          return Container(
            decoration: BoxDecoration(
              color: AppColors.surfaceSunken,
              border: Border(bottom: BorderSide(color: AppColors.border)),
            ),
            child: SafeArea(
              bottom: false,
              child: Padding(
                padding: const EdgeInsets.fromLTRB(22, 10, 22, 10),
                child: Row(
                  children: [
                    Tooltip(
                      message: 'Home',
                      child: Material(
                        color: pane == DesktopPane.home
                            ? AppColors.surfaceRaised
                            : Colors.transparent,
                        borderRadius: BorderRadius.circular(AppRadius.inner),
                        child: InkWell(
                          key: const Key('headerHomeButton'),
                          borderRadius: BorderRadius.circular(AppRadius.inner),
                          onTap: () => onPane(DesktopPane.home),
                          child: Padding(
                            padding: const EdgeInsets.all(5),
                            child: PortalisLogo(size: 34, energized: active),
                          ),
                        ),
                      ),
                    ),
                    const SizedBox(width: 12),
                    DesktopIdentityChip(
                      selected: pane == DesktopPane.user,
                      onTap: () => onPane(DesktopPane.user),
                    ),
                    const SizedBox(width: 18),
                    const Expanded(child: DesktopEventRail()),
                    DesktopNavigationAction(
                      icon: Icons.people_outline,
                      tooltip: 'People',
                      selected: pane == DesktopPane.people,
                      badge: _peopleCount,
                      onTap: () => onPane(DesktopPane.people),
                    ),
                    const SizedBox(width: 4),
                    DesktopNavigationAction(
                      icon: Icons.tune,
                      tooltip: 'Settings',
                      selected: pane == DesktopPane.settings,
                      onTap: () => onPane(DesktopPane.settings),
                    ),
                  ],
                ),
              ),
            ),
          );
        },
      );

  String? get _peopleCount {
    final ids = <String>{
      for (final collection in AppControllers.collections.collections)
        for (final person in collection.collaborators) person.deviceId,
    };
    return ids.isEmpty ? null : '${ids.length}';
  }
}
