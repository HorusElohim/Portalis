// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../theme.dart';
import 'controls.dart';

/// Shared chrome for a screen reachable two ways: swapped into a parent
/// layout in place on a wide desktop window (`embedded: true`), or as its
/// own route everywhere else (`embedded: false`) — mobile, and a desktop
/// window narrow enough to be running the phone layout.
///
/// Every screen that lives in both places — You, People, Settings, File
/// formats, Storage — renders exactly this plus its own [body]. That is the
/// whole contract: there is one place chrome is decided, not five slightly
/// different copies of it, which is how File formats ended up reachable on
/// mobile but easy to lose track of on desktop in the first place.
///
/// Embedded, this renders bare — no [Scaffold], no [SafeArea]. The parent
/// pane (ultimately the desktop shell) already provides both; a second layer
/// is only ever redundant chrome nobody sees. Pushed, it supplies both
/// itself, since a route owns its own background.
///
/// [forceShowBack] is the one thing that has to vary per *screen* rather
/// than per context. A screen reachable from the desktop sidebar (You,
/// People, Settings) hides its back button when embedded, because the
/// sidebar is itself the way back out. A screen reached only by drilling
/// into another one — File formats from You, Storage or Advanced from
/// Settings — has no such alternative and needs its back button regardless
/// of [embedded]; its caller passes `forceShowBack: true` (or, for a state
/// internal to the same widget like Settings' Advanced view, whatever
/// condition means "showing the drilled-into view").
class AdaptiveScreen extends StatelessWidget {
  const AdaptiveScreen({
    super.key,
    required this.embedded,
    required this.body,
    this.onBack,
    this.forceShowBack = false,
  });

  final bool embedded;
  final Widget body;

  /// Pushed: defaults to popping the route (see [NavBackButton]). Embedded
  /// with [forceShowBack]: the caller's own "collapse back to the parent"
  /// callback — there is no route here to pop.
  final VoidCallback? onBack;

  final bool forceShowBack;

  @override
  Widget build(BuildContext context) {
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (forceShowBack || !embedded)
          Padding(
            padding: const EdgeInsets.fromLTRB(14, 8, 14, 0),
            child: Align(
              alignment: Alignment.centerLeft,
              child: NavBackButton(onTap: onBack),
            ),
          ),
        Expanded(child: body),
      ],
    );
    if (embedded) return content;
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(child: content),
    );
  }
}

/// Opens a screen nested inside this one — File formats from You, Storage
/// from Settings: swapped in place when [embedded] (desktop — there is
/// nowhere else for a pushed route to go but over the whole shell), or
/// pushed as its own route otherwise (mobile, where the drill-down keeps its
/// own back-stack entry). The one place that decision is made, so the next
/// nested screen someone adds can't wire up one platform and forget the
/// other.
void openNestedScreen(
  BuildContext context, {
  required bool embedded,
  required VoidCallback showInPlace,
  required WidgetBuilder push,
}) {
  if (embedded) {
    showInPlace();
  } else {
    Navigator.of(context).push(MaterialPageRoute(builder: push));
  }
}
