import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/collections.dart';
import '../services/device_identity.dart';
import '../services/navigation.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'collection_screen.dart';
import 'join_collection_screen.dart';
import 'people_screen.dart';
import 'settings_screen.dart';
import 'share_screen.dart';
import 'user_screen.dart';

/// The wide-window layout: sidebar, list, and whatever is being looked at.
///
/// Same model and the same widgets as mobile — only the arrangement differs.
/// Reached from [RootShell] on width alone, so resizing the window moves
/// between layouts without any platform check.
class DesktopShell extends StatefulWidget {
  const DesktopShell({super.key});

  @override
  State<DesktopShell> createState() => _DesktopShellState();
}

/// Neither Transfers nor Home, unlike mobile — both are answers to questions
/// this layout doesn't leave open.
///
/// Transfers showed the same collections a second time: every row already
/// carries its own bar, rate and countdown, and the list is permanently on
/// screen. Home was the welcome — what Portalis is, and the three ways to
/// start something — which the sidebar now says outright with its own actions
/// beside a list that is always visible. On a phone both earn their keep,
/// because there the list is one destination among four and a row is small.
///
/// [share] and [join] are here for the same reason as the rest: whatever the
/// centre pane shows, the sidebar and list stay put. They differ from
/// [people]/[you]/[settings] only in how they're reached — a one-shot action
/// in the sidebar rather than a persistent header button — `_select` doesn't
/// otherwise distinguish them.
enum _Pane { collections, people, you, settings, share, join }

class _DesktopShellState extends State<DesktopShell> {
  _Pane _pane = _Pane.collections;
  /// The collection whose card is showing its contents, if any.
  String? _openId;

  /// The two shells share a destination wherever they have one in common, so
  /// that resizing across the breakpoint leaves you where you were rather than
  /// somewhere arbitrary — which matters now that the window can be dragged
  /// freely between the desktop and phone layouts. It is also what makes
  /// Home's "go to Collections" link work here: it sets the tab, and nothing
  /// was listening.
  /// Mobile's Home lands on Collections, which is what this layout puts in its
  /// place. The tab itself is left alone in that case, so narrowing the window
  /// again returns you to Home rather than stranding you somewhere you never
  /// chose.
  static _Pane? _paneForTab(int tab) => switch (tab) {
        0 || 1 => _Pane.collections,
        2 => _Pane.you,
        _ => null,
      };

  static int? _tabForPane(_Pane pane) => switch (pane) {
        _Pane.collections => 1,
        _Pane.you => 2,
        // No mobile peer: People is derived from collections, Settings is
        // reached through You there, and Share/Join are their own pushed
        // screens on mobile rather than a pane of anything.
        _Pane.people || _Pane.settings || _Pane.share || _Pane.join => null,
      };

  @override
  void initState() {
    super.initState();
    _pane = _paneForTab(AppNavigation.tab.value) ?? _Pane.collections;
    AppNavigation.tab.addListener(_onTabChanged);
  }

  @override
  void dispose() {
    AppNavigation.tab.removeListener(_onTabChanged);
    super.dispose();
  }

  void _onTabChanged() {
    final pane = _paneForTab(AppNavigation.tab.value);
    if (pane != null && pane != _pane && mounted) {
      setState(() => _pane = pane);
    }
  }

  /// Selects a pane, or closes it if it is already open.
  ///
  /// Toggling matters now that Collections has no control of its own: the
  /// collections are what the window shows when nothing is layered over them,
  /// so tapping the open pane's button again is how you get back to them.
  void _select(_Pane pane) {
    final next = pane == _pane ? _Pane.collections : pane;
    if (next == _pane) return;
    setState(() => _pane = next);
    final tab = _tabForPane(next);
    if (tab != null) AppNavigation.tab.value = tab;
  }

  /// Clicking an open collection closes it again — the card is the view, so
  /// there is nothing else for a second click to mean.
  void _open(String id) {
    setState(() => _openId = _openId == id ? null : id);
    _select(_Pane.collections);
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: ListenableBuilder(
        listenable: Collections.instance,
        builder: (context, _) {
          // No second panel. A collection opens inside its own card, in
          // the one list — a panel beside it was a second, thinner account
          // of the same collection, and a button to get from one to the
          // other.
          return Row(
            children: [
              _Sidebar(pane: _pane, onPane: _select),
              Expanded(
                child: SafeArea(
                  child: _pane == _Pane.collections
                      ? _CollectionsPane(openId: _openId, onOpen: _open)
                      : _centre(),
                ),
              ),
            ],
          );
        },
      ),
    );
  }

  Widget _centre() {
    switch (_pane) {
      // `embedded: true` — see AdaptiveScreen. Each renders bare (no
      // Scaffold, no SafeArea), relying on the one already established in
      // build() above, so getting that right isn't something this switch or
      // any one screen has to remember on its own.
      case _Pane.you:
        return const UserScreen(embedded: true);
      case _Pane.people:
        return const PeopleScreen(embedded: true);
      case _Pane.settings:
        return const SettingsScreen(embedded: true);
      // Share and Join have their own Scaffold/SafeArea regardless of
      // context — a one-shot action rather than a destination with an
      // embedded/pushed duality, so AdaptiveScreen doesn't apply. Closing
      // returns to the list they replaced, same as re-tapping an open header
      // button.
      case _Pane.share:
        return ShareScreen(onClose: () => _select(_Pane.collections));
      case _Pane.join:
        return JoinCollectionScreen(onClose: () => _select(_Pane.collections));
      case _Pane.collections:
        // Rendered directly in build(), since it is always on screen.
        return const SizedBox.shrink();
    }
  }
}

class _Sidebar extends StatelessWidget {
  const _Sidebar({required this.pane, required this.onPane});

  final _Pane pane;
  final ValueChanged<_Pane> onPane;

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
                      selected: pane == _Pane.you,
                      onTap: () => onPane(_Pane.you),
                    ),
                  ),
                  const SizedBox(width: 4),
                  _HeaderButton(
                    icon: Icons.people_outline,
                    tooltip: 'People',
                    selected: pane == _Pane.people,
                    badge: people == 0 ? null : '$people',
                    onTap: () => onPane(_Pane.people),
                  ),
                  const SizedBox(width: 2),
                  _HeaderButton(
                    icon: Icons.tune,
                    tooltip: 'Settings',
                    selected: pane == _Pane.settings,
                    onTap: () => onPane(_Pane.settings),
                  ),
                ],
              ),
              const SizedBox(height: 20),
              // Desktop had no Home at all: the welcome — what Portalis is
              // and the three ways to start something — was reachable only by
              // narrowing the window into the phone layout. It is the same
              // destination as the mobile bar's first item, and carries the
              // same mark for the same reason: it is both where you are and
              // how you get back.
              PrimaryAction(
                label: 'New share',
                icon: Icons.add,
                trailingChevron: false,
                onTap: () => onPane(_Pane.share),
              ),
              const SizedBox(height: 8),
              // The other ways in. Mobile offers all three from one FAB;
              // desktop offered only this first one, so joining with an invite
              // key or adding a magnet was reachable solely from inside the
              // Home pane — which is not where anyone looks for an action.
              _miniAction(context, Icons.link, 'Join with a key',
                  () => onPane(_Pane.join)),
              const SizedBox(height: 14),
              const _TorrentQuickAdd(),
              const SizedBox(height: 20),
              // No destination list at all. Collections is not a place you go
              // — it is what this window *is*, permanently on the right — and
              // the three things that aren't it are the header controls above,
              // each of which toggles back to the collections when tapped
              // again.
              const Spacer(),
              const _SessionRates(),
            ],
          ),
        ),
      ),
    );
  }

  /// A secondary way in.
  Widget _miniAction(BuildContext context, IconData icon, String label,
      VoidCallback onTap, {Color color = AppColors.signalSoft}) {
    return Material(
      color: AppColors.surfaceRaised,
      borderRadius: BorderRadius.circular(14),
      child: InkWell(
        borderRadius: BorderRadius.circular(14),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 15),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(icon, size: 17, color: color),
              const SizedBox(width: 9),
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(
                      fontSize: 14, fontWeight: FontWeight.w500),
                ),
              ),
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
        borderRadius: BorderRadius.circular(11),
        child: InkWell(
          key: Key('header${tooltip}Button'),
          borderRadius: BorderRadius.circular(11),
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
                  Text(badge!,
                      style: monoLabel(size: 10.5, letterSpacing: 0)),
                ],
              ],
            ),
          ),
        ),
      ),
    );
  }
}

/// Adding a torrent, in the sidebar rather than through a screen.
///
/// A magnet link is one line of text and a `.torrent` is one file — the full
/// screen exists to preview what a link *says* it contains before committing,
/// which is worth a screen on a phone and is not worth losing the whole
/// desktop layout for. The screen is still there behind the Home destination
/// on mobile; this is the same two calls with no navigation at all.
class _TorrentQuickAdd extends StatefulWidget {
  const _TorrentQuickAdd();

  @override
  State<_TorrentQuickAdd> createState() => _TorrentQuickAddState();
}

class _TorrentQuickAddState extends State<_TorrentQuickAdd> {
  final _controller = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  bool get _isValid => looksLikeMagnet(_controller.text);

  Future<void> _run(Future<void> Function() action, String done) async {
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await action();
      if (!mounted) return;
      _controller.clear();
      showToast(context, done, severity: ToastSeverity.success);
    } catch (e) {
      // Shown here rather than as a toast: the field is still on screen with
      // the text that failed, so the message belongs next to it.
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _paste() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim();
    if (text == null || text.isEmpty) return;
    setState(() => _controller.text = text);
  }

  Future<void> _pickFile() async {
    final result =
        await FilePicker.pickFiles(withData: true, type: FileType.any);
    final bytes = result?.files.single.bytes;
    if (bytes == null) return;
    await _run(
      () => Collections.instance.addFromFileBytes(bytes),
      'Torrent added — joining swarm',
    );
  }

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        const SectionLabel('TORRENT'),
        const SizedBox(height: 7),
        TextField(
          key: const Key('sidebarMagnetField'),
          controller: _controller,
          enabled: !_busy,
          style: monoLabel(size: 11.5, color: AppColors.text, letterSpacing: 0),
          decoration: InputDecoration(
            isDense: true,
            hintText: 'magnet: or info hash',
            hintStyle: monoLabel(size: 11.5, letterSpacing: 0),
            filled: true,
            fillColor: AppColors.surface,
            contentPadding:
                const EdgeInsets.symmetric(horizontal: 12, vertical: 14),
            border: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: AppColors.border),
            ),
            enabledBorder: OutlineInputBorder(
              borderRadius: BorderRadius.circular(12),
              borderSide: const BorderSide(color: AppColors.border),
            ),
          ),
          onChanged: (_) => setState(() {}),
          onSubmitted: (_) => _isValid && !_busy
              ? _run(
                  () => Collections.instance.addFromMagnet(_controller.text.trim()),
                  'Added — joining swarm',
                )
              : null,
        ),
        const SizedBox(height: 6),
        Row(
          children: [
            Expanded(
              child: _button('Paste', onTap: _busy ? null : _paste),
            ),
            const SizedBox(width: 6),
            Expanded(
              child: _button(
                'Add',
                primary: true,
                onTap: !_isValid || _busy
                    ? null
                    : () => _run(
                          () => Collections.instance
                              .addFromMagnet(_controller.text.trim()),
                          'Added — joining swarm',
                        ),
              ),
            ),
            const SizedBox(width: 6),
            Tooltip(
              message: 'Add a .torrent file',
              child: _button(
                null,
                icon: Icons.attach_file,
                onTap: _busy ? null : _pickFile,
              ),
            ),
          ],
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(top: 6),
            child: Text(
              _error!,
              style: monoLabel(
                  size: 9.5, color: AppColors.danger, letterSpacing: 0.2),
            ),
          ),
      ],
    );
  }

  Widget _button(String? label,
      {IconData? icon, VoidCallback? onTap, bool primary = false}) {
    final enabled = onTap != null;
    return Material(
      color: primary && enabled
          ? AppColors.signal
          : AppColors.surfaceRaised,
      borderRadius: BorderRadius.circular(12),
      child: InkWell(
        borderRadius: BorderRadius.circular(12),
        onTap: onTap,
        child: Padding(
          padding: EdgeInsets.symmetric(
              horizontal: icon == null ? 6 : 11, vertical: 12),
          child: Center(
            child: icon != null
                ? Icon(icon,
                    size: 17,
                    color: enabled ? AppColors.ember : AppColors.textFaint)
                : Text(
                    label!,
                    style: TextStyle(
                      fontSize: 13,
                      fontWeight: FontWeight.w500,
                      color: !enabled
                          ? AppColors.textFaint
                          : primary
                              ? AppColors.onSignal
                              : AppColors.text,
                    ),
                  ),
          ),
        ),
      ),
    );
  }
}

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
      borderRadius: BorderRadius.circular(11),
      child: InkWell(
        key: const Key('identityChip'),
        borderRadius: BorderRadius.circular(11),
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
                      style: const TextStyle(
                          fontSize: 13.5, fontWeight: FontWeight.w600),
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
      radius: 14,
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

class _CollectionsPane extends StatelessWidget {
  const _CollectionsPane({required this.openId, required this.onOpen});

  final String? openId;
  final ValueChanged<String> onOpen;

  @override
  Widget build(BuildContext context) {
    final collections = Collections.instance.collections;
    final error = Collections.instance.lastError;
    final moving = collections.where((c) => c.isMoving).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 26, 28, 0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              const CanvasTitle('Collections', size: 32),
              const SizedBox(height: 4),
              Text(
                moving == 0
                    ? '${collections.length} collection'
                        '${collections.length == 1 ? '' : 's'}'
                    : '$moving transfer${moving == 1 ? '' : 's'} in flight',
                style:
                    const TextStyle(fontSize: 13.5, color: AppColors.textDim),
              ),
            ],
          ),
        ),
        Expanded(
          child: collections.isEmpty
              ? Center(
                  child: Text(
                    error ?? 'Nothing here yet.',
                    textAlign: TextAlign.center,
                    style: TextStyle(
                      fontSize: 13,
                      color: error != null
                          ? AppColors.danger
                          : AppColors.textDim,
                    ),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(28, 20, 28, 28),
                  itemCount: collections.length,
                  separatorBuilder: (_, __) => const SizedBox(height: 10),
                  itemBuilder: (context, i) {
                    final c = collections[i];
                    final open = c.id == openId;
                    return CollectionRow(
                      collection: c,
                      selected: open,
                      onTap: () => onOpen(c.id),
                      // The card *is* the view. Keyed by id so opening
                      // another starts fresh rather than carrying the
                      // previous one's disclosure across.
                      detail: open
                          ? CollectionDetail(
                              key: ValueKey(c.id),
                              collection: c,
                              showHeading: false,
                            )
                          : null,
                    );
                  },
                ),
        ),
      ],
    );
  }
}

// PeopleScreen (see people_screen.dart) supplies this pane's content — it's
// shared with the mobile push reached from the You tab.
