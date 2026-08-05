// Cross-feature screen layout primitive.

import 'package:flutter/material.dart';

import '../theme.dart';
import 'controls.dart';
import 'primitives.dart';
import 'window.dart';

/// How much horizontal room a screen's content is allowed to take.
///
/// The rule, applied everywhere rather than decided per screen: content you
/// *read* is a centred column at a comfortable measure; content that
/// *reflows on its own* — a grid of cards, a list of rows — fills the pane
/// it was given and arranges itself with [WindowSize.columns].
enum ScreenWidth {
  /// A centred column. Text, forms, settings, detail.
  reading,

  /// The full pane. Grids and lists that already respond to their width.
  full,
}

/// The frame every screen in Portalis is built in.
///
/// One widget owns what used to be five independent decisions repeated at
/// every screen: whether to supply a [Scaffold] and [SafeArea], whether to
/// show a back button, how wide the content may get, how big the title is,
/// and where the left edge sits. The result was that no two screens agreed —
/// see [kScreenGutter] for the gutters, and the title scale ran 25, 30, 32,
/// 34 and 46 across screens with no rule behind which got which.
///
/// Here the rule is the type scale responds to the window, not to the
/// screen: every title is [ImpactTitle], at 34 on a narrow window and 46 on
/// a spacious one. A screen no longer chooses — so a screen can no longer
/// disagree.
///
/// [embedded] means this is rendered inside a parent that already supplies
/// chrome — a desktop shell pane, or a mobile tab of [RootShell]. It gets no
/// Scaffold, no SafeArea and no back button, because its parent has all
/// three. Not embedded means it owns a pushed route and supplies its own.
///
/// [forceShowBack] covers the one case [embedded] gets wrong: a screen
/// reached by drilling *into* another one — File formats from You, Storage
/// or Advanced from Settings — has no sidebar to go back through, so it
/// needs its back button even embedded. Its caller passes the callback that
/// collapses it, since there is no route to pop.
class AppScreen extends StatelessWidget {
  const AppScreen({
    super.key,
    required this.title,
    required this.body,
    this.subtitle,
    this.titleLeading,
    this.footer,
    this.embedded = false,
    this.onBack,
    this.forceShowBack = false,
    this.width = ScreenWidth.reading,
    this.wideMaxWidth,
  });

  final String title;

  /// A line under the title — usually what the screen is currently showing
  /// ("3 collaborators…", "12.4 GB across 6 items"). A widget rather than a
  /// string so a screen with something live to say can style it: Collections
  /// turns its subtitle mint while data is moving.
  final Widget? subtitle;

  /// A mark beside the title — only Add torrent has one, and it is the
  /// ember torrent glyph that says which kind of transfer this starts.
  /// Deliberately not a free-form header slot: a screen may add a mark, not
  /// pick its own type scale.
  final Widget? titleLeading;

  /// The action that closes a flow, pinned under the body — see
  /// [ScreenAction]. The rule above it and the gutter around it come from
  /// here, so the three flows can't disagree about them again: Join's was
  /// missing the rule the other two drew.
  final Widget? footer;

  final Widget body;
  final bool embedded;
  final VoidCallback? onBack;
  final bool forceShowBack;
  final ScreenWidth width;

  /// Overrides the shared wide cap for a screen that genuinely does
  /// something else with the room. Only Settings does — it splits into two
  /// columns — and it says so where it passes this.
  final double? wideMaxWidth;

  bool get _showBack => forceShowBack || !embedded;

  @override
  Widget build(BuildContext context) {
    final content = Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (_showBack)
          Padding(
            padding: const EdgeInsets.only(left: kScreenGutter - 12),
            child: Align(
              alignment: Alignment.centerLeft,
              child: NavBackButton(onTap: onBack),
            ),
          ),
        Padding(
          padding: EdgeInsets.fromLTRB(
              kScreenGutter, _showBack ? 4 : 24, kScreenGutter, 18),
          child: ScreenHeader(
            title: title,
            subtitle: subtitle,
            leading: titleLeading,
          ),
        ),
        // The header stays put and the body scrolls under it. It used to be
        // whichever the screen happened to build: Formats, Collections and
        // Settings scrolled theirs away, People, Storage and the desktop
        // Collections pane pinned theirs.
        Expanded(child: body),
        if (footer != null)
          Container(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 12, kScreenGutter, 16),
            decoration: const BoxDecoration(
              border: Border(top: BorderSide(color: AppColors.border)),
            ),
            child: footer,
          ),
      ],
    );

    final sized = width == ScreenWidth.reading
        ? PageBody(
            wideMaxWidth: wideMaxWidth ?? PageBody.defaultWideMaxWidth,
            child: content,
          )
        : content;

    if (embedded) return sized;
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(child: sized),
    );
  }
}

/// A screen's title and its one supporting line.
///
/// Exposed because two places show a heading outside an [AppScreen] frame —
/// but the scale is decided here, once, for all of them.
class ScreenHeader extends StatelessWidget {
  const ScreenHeader({
    super.key,
    required this.title,
    this.subtitle,
    this.leading,
  });

  final String title;
  final Widget? subtitle;
  final Widget? leading;

  @override
  Widget build(BuildContext context) {
    return WindowBuilder(
      builder: (context, window) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        mainAxisSize: MainAxisSize.min,
        children: [
          // The window decides the scale, not the screen — see [AppScreen].
          if (leading == null)
            ImpactTitle(title, size: window.isSpacious ? 46 : 34)
          else
            Row(
              crossAxisAlignment: CrossAxisAlignment.center,
              children: [
                leading!,
                const SizedBox(width: 14),
                Flexible(
                  child: ImpactTitle(title, size: window.isSpacious ? 46 : 34),
                ),
              ],
            ),
          if (subtitle != null) ...[
            const SizedBox(height: 10),
            DefaultTextStyle(
              style: AppText.body(color: AppColors.textDim),
              child: subtitle!,
            ),
          ],
        ],
      ),
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
