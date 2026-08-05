import 'package:flutter/material.dart';

import '../services/collections.dart';
import '../services/device_identity.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'desktop_pane.dart';

/// The desktop shell's left column: who you are, where you can go, and
/// nothing else.
///
/// Split out of `desktop_shell.dart` so that file can stay what it says it
/// is — the pane router — while this one is free to be read on its own: an
/// identity chip, two header controls, and a throughput card, in that order,
/// with nothing about *what the panes contain* leaking in.
class DesktopSidebar extends StatelessWidget {
  const DesktopSidebar({super.key, required this.pane, required this.onPane});

  final DesktopPane pane;
  final ValueChanged<DesktopPane> onPane;

  @override
  Widget build(BuildContext context) {
    final collections = Collections.instance.collections;
    final people = <String>{
      for (final c in collections)
        for (final p in c.collaborators) p.deviceId,
    }.length;

    return Container(
      width: 236,
      decoration: const BoxDecoration(
        color: AppColors.surfaceSunken,
        border: Border(right: BorderSide(color: AppColors.border)),
      ),
      child: SafeArea(
        child: Padding(
          padding: const EdgeInsets.fromLTRB(14, 20, 14, 14),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              // Who you are and how the app behaves, side by side and out of
              // the way. Both were rows in the list below, competing for
              // attention with the collections themselves — which is what
              // someone actually came here to look at.
              Row(
                children: [
                  Expanded(
                    child: _IdentityChip(
                      selected: pane == DesktopPane.you,
                      onTap: () => onPane(DesktopPane.you),
                    ),
                  ),
                  const SizedBox(width: 4),
                  _HeaderButton(
                    icon: Icons.people_outline,
                    tooltip: 'People',
                    selected: pane == DesktopPane.people,
                    badge: people == 0 ? null : '$people',
                    onTap: () => onPane(DesktopPane.people),
                  ),
                  const SizedBox(width: 2),
                  _HeaderButton(
                    icon: Icons.tune,
                    tooltip: 'Settings',
                    selected: pane == DesktopPane.settings,
                    onTap: () => onPane(DesktopPane.settings),
                  ),
                ],
              ),
              // Pure navigation from here down. The sidebar used to carry a
              // New share button, a Join action, a magnet field, a Paste
              // button, an Add button and a .torrent picker — six controls,
              // where the omnibar above the list now takes any of them as one
              // paste, and New share sits beside it as the single primary
              // action. What is left is who you are and where you can go.
              const Spacer(),
              const _SessionRates(),
            ],
          ),
        ),
      ),
    );
  }
}

/// One of the header controls beside the identity chip.
///
/// People and Settings were rows in a destination list. Neither is a thing you
/// look at the way a collection is — one is a derived directory, the other is
/// how the engine behaves — and listing them put them at the same weight as
/// the collections themselves. Up here they are what they are: controls, next
/// to the other control that says who you are.
///
/// Tapping the active one returns to the collections, which is the only way
/// back now that Collections has no button of its own.
class _HeaderButton extends StatelessWidget {
  const _HeaderButton({
    required this.icon,
    required this.tooltip,
    required this.selected,
    required this.onTap,
    this.badge,
  });

  final IconData icon;
  final String tooltip;
  final bool selected;
  final VoidCallback onTap;

  /// A count worth knowing without opening the pane, e.g. how many people.
  final String? badge;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Material(
        color: selected ? AppColors.surfaceRaised : Colors.transparent,
        borderRadius: BorderRadius.circular(AppRadius.inner),
        child: InkWell(
          key: Key('header${tooltip}Button'),
          borderRadius: BorderRadius.circular(AppRadius.inner),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 11),
            child: Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Icon(
                  icon,
                  size: 20,
                  color: selected ? AppColors.text : AppColors.textDim,
                ),
                if (badge != null) ...[
                  const SizedBox(width: 5),
                  Text(badge!, style: monoLabel(size: 10.5, letterSpacing: 0)),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Avatar, name and live peer count — and the way in to the You pane, since
/// clicking your own name is the one navigation a person reaches for without
/// having to learn it is there.
class _IdentityChip extends StatefulWidget {
  const _IdentityChip({required this.selected, required this.onTap});

  final bool selected;
  final VoidCallback onTap;

  @override
  State<_IdentityChip> createState() => _IdentityChipState();
}

class _IdentityChipState extends State<_IdentityChip> {
  @override
  void initState() {
    super.initState();
    DeviceIdentity.instance.load();
  }

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: DeviceIdentity.instance,
      builder: (context, _) => _build(context),
    );
  }

  Widget _build(BuildContext context) {
    final name = DeviceIdentity.instance.info?.nickname;
    final peers = Collections.instance.collections
        .fold<int>(0, (s, c) => s + c.livePeers);
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
            children: [
              Avatar(
                initials: (name == null || name.isEmpty)
                    ? '·'
                    : name[0].toUpperCase(),
                size: 30,
                primary: true,
              ),
              const SizedBox(width: 10),
              Expanded(
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Text(
                      name ?? 'This device',
                      overflow: TextOverflow.ellipsis,
                      style: AppText.body(weight: FontWeight.w600),
                    ),
                    const SizedBox(height: 1),
                    Text(
                      peers == 0
                          ? 'NO PEERS'
                          : '$peers PEER${peers == 1 ? '' : 'S'}',
                      style: monoLabel(
                        size: 9.5,
                        color:
                            peers > 0 ? AppColors.signal : AppColors.textFaint,
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
  }
}

/// Current aggregate throughput. Instantaneous only — no history is retained
/// anywhere, so the design's sparkline would have had nothing to plot.
class _SessionRates extends StatelessWidget {
  const _SessionRates();

  @override
  Widget build(BuildContext context) {
    final collections = Collections.instance.collections;
    final down = collections.fold<double>(0, (s, c) => s + c.downloadMbps);
    final up = collections.fold<double>(0, (s, c) => s + c.uploadMbps);
    final live = down > 0 || up > 0;
    return SurfaceCard(
      radius: AppRadius.control,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('RIGHT NOW', style: monoLabel(size: 9.5)),
          const SizedBox(height: 8),
          Text(
            '↓ ${down.toStringAsFixed(1)} · ↑ ${up.toStringAsFixed(1)} MB/s',
            style: monoLabel(
              size: 11,
              color: live ? AppColors.signal : AppColors.textDim,
              letterSpacing: 0,
            ),
          ),
        ],
      ),
    );
  }
}
