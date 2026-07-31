import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../bridge_generated/device.dart' as device_bridge;
import '../models.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'collection_screen.dart';
import 'settings_screen.dart';
import 'share_screen.dart';
import 'transfers_screen.dart';

/// The wide-window layout: sidebar, list, inspector.
///
/// Same model and the same widgets as mobile — only the arrangement differs.
/// Reached from [RootShell] on width alone, so resizing the window moves
/// between layouts without any platform check.
class DesktopShell extends StatefulWidget {
  const DesktopShell({super.key});

  @override
  State<DesktopShell> createState() => _DesktopShellState();
}

enum _Pane { collections, transfers, people, settings }

class _DesktopShellState extends State<DesktopShell> {
  _Pane _pane = _Pane.collections;
  String? _selectedId;

  Collection? get _selected {
    final list = Collections.instance.collections;
    if (list.isEmpty) return null;
    for (final c in list) {
      if (c.id == _selectedId) return c;
    }
    // Selection follows the data: if the selected collection is gone (or
    // nothing has been picked yet), fall back to the first rather than
    // showing an empty inspector beside a populated list.
    return list.first;
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: ListenableBuilder(
        listenable: Collections.instance,
        builder: (context, _) {
          // Collections stays on the left at all times. Swapping it out for
          // Transfers/People/Settings meant losing sight of your own
          // collections just to change a setting — on a screen wide enough to
          // show both, there is no reason to.
          final secondary = _pane == _Pane.collections ? null : _centre();
          final collections = SafeArea(
            child: _CollectionsPane(
              selectedId: _selected?.id,
              onSelect: (id) => setState(() {
                _selectedId = id;
                // Picking a collection means you want to look at it, so any
                // secondary pane steps aside.
                _pane = _Pane.collections;
              }),
            ),
          );
          return Row(
            children: [
              _Sidebar(
                pane: _pane,
                onPane: (p) => setState(() => _pane = p),
              ),
              if (secondary == null)
                Expanded(child: collections)
              else
                // Fixed while something else is open, so the list doesn't
                // reflow every time a secondary pane appears.
                SizedBox(width: 360, child: collections),
              if (secondary != null) Expanded(child: secondary),
              if (_pane == _Pane.collections && _selected != null)
                _Inspector(collection: _selected!),
            ],
          );
        },
      ),
    );
  }

  Widget _centre() {
    switch (_pane) {
      case _Pane.transfers:
        return const SafeArea(child: TransfersScreen());
      case _Pane.people:
        return const SafeArea(child: _PeoplePane());
      case _Pane.settings:
        return const SettingsScreen(embedded: true);
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
    final moving = collections.where(TransfersScreen.isMoving).length;
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
              const _IdentityChip(),
              const SizedBox(height: 22),
              PrimaryAction(
                label: 'New share',
                icon: Icons.add,
                trailingChevron: false,
                onTap: () => Navigator.of(context).push(
                  MaterialPageRoute(builder: (_) => const ShareScreen()),
                ),
              ),
              const SizedBox(height: 22),
              _navItem(_Pane.collections, Icons.dashboard_outlined,
                  'Collections', '${collections.length}'),
              _navItem(_Pane.transfers, Icons.swap_horiz, 'Transfers',
                  moving == 0 ? null : '$moving',
                  countIsLive: moving > 0),
              _navItem(_Pane.people, Icons.people_outline, 'People',
                  people == 0 ? null : '$people'),
              _navItem(_Pane.settings, Icons.tune, 'Settings', null),
              const Spacer(),
              const _SessionRates(),
            ],
          ),
        ),
      ),
    );
  }

  Widget _navItem(_Pane p, IconData icon, String label, String? count,
      {bool countIsLive = false}) {
    final selected = p == pane;
    return Padding(
      padding: const EdgeInsets.only(bottom: 2),
      child: Material(
        color: selected ? AppColors.surfaceRaised : Colors.transparent,
        borderRadius: BorderRadius.circular(11),
        child: InkWell(
          borderRadius: BorderRadius.circular(11),
          onTap: () => onPane(p),
          child: Padding(
            padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
            child: Row(
              children: [
                Icon(icon,
                    size: 17,
                    color: selected ? AppColors.text : AppColors.textDim),
                const SizedBox(width: 11),
                Expanded(
                  child: Text(
                    label,
                    style: TextStyle(
                      fontSize: 13.5,
                      fontWeight:
                          selected ? FontWeight.w600 : FontWeight.w400,
                      color: selected ? AppColors.text : AppColors.textDim,
                    ),
                  ),
                ),
                if (count != null)
                  Text(
                    count,
                    // Mint only when the count represents movement.
                    style: monoLabel(
                      size: 11,
                      color: countIsLive
                          ? AppColors.signal
                          : AppColors.textFaint,
                      letterSpacing: 0,
                    ),
                  ),
              ],
            ),
          ),
        ),
      ),
    );
  }
}

class _IdentityChip extends StatefulWidget {
  const _IdentityChip();

  @override
  State<_IdentityChip> createState() => _IdentityChipState();
}

class _IdentityChipState extends State<_IdentityChip> {
  String? _nickname;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final identity = await device_bridge.deviceIdentity();
      if (mounted) setState(() => _nickname = identity.nickname);
    } catch (_) {
      // Backend unavailable — the chip stays neutral rather than inventing
      // a name.
    }
  }

  @override
  Widget build(BuildContext context) {
    final name = _nickname;
    final peers = Collections.instance.collections
        .fold<int>(0, (s, c) => s + c.livePeers);
    return Padding(
      padding: const EdgeInsets.symmetric(horizontal: 8),
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
                    color: peers > 0 ? AppColors.signal : AppColors.textFaint,
                  ),
                ),
              ],
            ),
          ),
        ],
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
  const _CollectionsPane({required this.selectedId, required this.onSelect});

  final String? selectedId;
  final ValueChanged<String> onSelect;

  @override
  Widget build(BuildContext context) {
    final collections = Collections.instance.collections;
    final error = Collections.instance.lastError;
    final moving = collections.where(TransfersScreen.isMoving).length;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 26, 28, 0),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text('Collections', style: displayText(size: 28)),
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
                    return CollectionRow(
                      collection: c,
                      selected: c.id == selectedId,
                      onTap: () => onSelect(c.id),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

/// Distinct collaborators across every collection, and where they appear.
/// Derived — there is no peer directory in the backend.
class _PeoplePane extends StatelessWidget {
  const _PeoplePane();

  @override
  Widget build(BuildContext context) {
    final byDevice = <String, ({Collaborator who, List<String> collections})>{};
    for (final c in Collections.instance.collections) {
      for (final p in c.collaborators) {
        final entry = byDevice[p.deviceId];
        if (entry == null) {
          byDevice[p.deviceId] = (who: p, collections: [c.name]);
        } else {
          entry.collections.add(c.name);
        }
      }
    }
    final people = byDevice.values.toList();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Padding(
          padding: const EdgeInsets.fromLTRB(28, 26, 28, 0),
          child: Text('People', style: displayText(size: 28)),
        ),
        Expanded(
          child: people.isEmpty
              ? const Center(
                  child: Text(
                    'Nobody yet. Collaborators appear once you share or join.',
                    style: TextStyle(fontSize: 13, color: AppColors.textDim),
                  ),
                )
              : ListView.separated(
                  padding: const EdgeInsets.fromLTRB(28, 20, 28, 28),
                  itemCount: people.length,
                  separatorBuilder: (_, __) => const SizedBox(height: 10),
                  itemBuilder: (context, i) {
                    final p = people[i];
                    return SurfaceCard(
                      child: Row(
                        children: [
                          Avatar(initials: p.who.initials, size: 32),
                          const SizedBox(width: 12),
                          Expanded(
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              children: [
                                Text(
                                  p.who.isAdmin
                                      ? '${p.who.name} · admin'
                                      : p.who.name,
                                  style: const TextStyle(
                                      fontSize: 14,
                                      fontWeight: FontWeight.w600),
                                ),
                                const SizedBox(height: 3),
                                Text(
                                  p.collections.join(' · '),
                                  overflow: TextOverflow.ellipsis,
                                  style: monoLabel(
                                      size: 10.5, letterSpacing: 0.2),
                                ),
                              ],
                            ),
                          ),
                        ],
                      ),
                    );
                  },
                ),
        ),
      ],
    );
  }
}

class _Inspector extends StatelessWidget {
  const _Inspector({required this.collection});

  final Collection collection;

  @override
  Widget build(BuildContext context) {
    final code = collection.inviteCode;
    return Container(
      width: 314,
      decoration: const BoxDecoration(
        color: AppColors.surfaceSunken,
        border: Border(left: BorderSide(color: AppColors.border)),
      ),
      child: SafeArea(
        child: SingleChildScrollView(
          padding: const EdgeInsets.all(24),
          child: Column(
            crossAxisAlignment: CrossAxisAlignment.start,
            children: [
              Text(collection.name, style: displayText(size: 20)),
              const SizedBox(height: 5),
              Text(
                collection.isShared
                    ? 'SHARED WITH ${collection.collaborators.length}'
                    : 'TORRENT',
                style: monoLabel(size: 10.5, letterSpacing: 0.6),
              ),
              const SizedBox(height: 18),
              if (code != null) ...[
                Row(
                  children: [
                    Expanded(
                      child: PrimaryAction(
                        label: 'Copy invite key',
                        trailingChevron: false,
                        onTap: () {
                          Clipboard.setData(ClipboardData(text: code));
                          showToast(context, 'Invite code copied',
                              severity: ToastSeverity.success);
                        },
                      ),
                    ),
                  ],
                ),
                const SizedBox(height: 14),
                Center(
                  child: Container(
                    padding: const EdgeInsets.all(10),
                    decoration: BoxDecoration(
                      color: Colors.white,
                      borderRadius: BorderRadius.circular(10),
                    ),
                    child: QrImageView(
                      data: code,
                      version: QrVersions.auto,
                      size: 180,
                      backgroundColor: Colors.white,
                    ),
                  ),
                ),
                const SizedBox(height: 18),
              ],
              Text('CONTENTS', style: monoLabel(size: 9.5)),
              const SizedBox(height: 10),
              Text(
                collection.subtitle,
                style: const TextStyle(fontSize: 13, color: AppColors.textDim),
              ),
              const SizedBox(height: 18),
              if (collection.collaborators.isNotEmpty) ...[
                Text('COLLABORATORS', style: monoLabel(size: 9.5)),
                const SizedBox(height: 11),
                for (final p in collection.collaborators)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 11),
                    child: Row(
                      children: [
                        Avatar(initials: p.initials, size: 28),
                        const SizedBox(width: 11),
                        Expanded(
                          child: Text(
                            p.name,
                            overflow: TextOverflow.ellipsis,
                            style: const TextStyle(fontSize: 13),
                          ),
                        ),
                        if (p.isAdmin)
                          Text('ADMIN', style: monoLabel(size: 9.5)),
                      ],
                    ),
                  ),
                // No per-peer transfer rates: collaborators come from the
                // signed manifest, throughput is per-torrent, and nothing
                // maps one to the other.
              ],
              const SizedBox(height: 8),
              TextButton(
                onPressed: () => Navigator.of(context).push(
                  MaterialPageRoute(
                    builder: (_) => CollectionScreen(collection: collection),
                  ),
                ),
                child: const Text('Open collection'),
              ),
              const SizedBox(height: 12),
              const Text(
                'Files are read from where they already live. Nothing is '
                'copied to a server.',
                style: TextStyle(
                    fontSize: 12, height: 1.45, color: AppColors.textDim),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
