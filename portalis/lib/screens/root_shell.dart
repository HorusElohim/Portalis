import 'package:flutter/material.dart';

import '../app/app_controllers.dart';
import 'adaptive_shell.dart';
import 'desktop_pane.dart';
import 'desktop_shell_layout.dart';
import 'home.dart';
import 'mobile_shell_layout.dart';
import '../features/collections/presentation/collection_join.dart';
import '../features/collections/presentation/collection_share.dart';
import 'people.dart';
import 'settings.dart';
import 'user.dart';

/// The single application shell. Its parent state owns navigation and
/// lifecycle; only the arrangement changes when the window crosses the
/// responsive breakpoint.
class RootShell extends AdaptiveShell {
  const RootShell({super.key});

  @override
  State<RootShell> createState() => _RootShellState();
}

class _RootShellState extends AdaptiveShellState<RootShell> {
  @override
  Widget buildCompactLayout(BuildContext context) {
    if (pane == DesktopPane.share) {
      return ShareScreen(
        initialFiles: pendingShareFiles,
        onClose: closeShare,
      );
    }
    if (pane == DesktopPane.join) {
      return JoinCollectionScreen(
        initialCode: pendingInvite,
        onClose: closeJoin,
      );
    }
    return MobileShellLayout(
        index: tab,
        onSelected: selectTab,
        onOpen: (id) => openCollection(id, inline: false),
        onShare: ([files]) => openShare(files, false),
        onJoin: (code) => openJoin(code, inline: false),
      );
  }

  @override
  Widget buildWideLayout(BuildContext context) => DesktopShellLayout(
        pane: pane,
        onPane: selectPane,
        liveRate: AppControllers.collections.liveRate,
        home: Home(
          embedded: true,
          openId: openId,
          onOpen: (id) => openCollection(id, inline: true),
          onShare: ([files]) => openShare(files, true),
          onJoin: openJoinInline,
        ),
        content: _desktopContent(),
      );

  void openJoinInline(String code) => openJoin(code, inline: true);

  Widget _desktopContent() => switch (pane) {
        DesktopPane.people => const PeopleScreen(embedded: true),
        DesktopPane.user => const UserScreen(embedded: true),
        DesktopPane.settings => const SettingsScreen(embedded: true),
        DesktopPane.share => ShareScreen(
            key: ValueKey(pendingShareFiles),
            initialFiles: pendingShareFiles,
            onClose: closeShare,
          ),
        DesktopPane.join => JoinCollectionScreen(
            key: ValueKey(pendingInvite),
            initialCode: pendingInvite,
            onClose: closeJoin,
          ),
        DesktopPane.home => const SizedBox.shrink(),
      };
}
