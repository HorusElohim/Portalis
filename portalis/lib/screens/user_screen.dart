import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../bridge_generated/device.dart' as bridge;
import '../services/collections.dart';
import '../services/device_identity.dart';
import '../services/navigation.dart';
import '../theme.dart';
import '../ui/ui.dart';
import 'formats_screen.dart';
import 'settings_screen.dart';

/// You — this device's identity and what it has moved.
///
/// Everything here is device-level and real. The design's lifetime "SENT /
/// RECEIVED" totals are not: those counters are per-collection and reset with
/// the session, so they're labelled as this session rather than implying a
/// running total the backend never keeps.
class UserScreen extends StatefulWidget {
  const UserScreen({super.key, this.embedded = false});

  /// Set when this is a pane of the desktop shell rather than a pushed screen.
  /// Settings is its own sidebar destination there, so the row that opens it
  /// would be a full-screen push over a layout that already has room for it.
  final bool embedded;

  @override
  State<UserScreen> createState() => _UserScreenState();
}

class _UserScreenState extends State<UserScreen> {
  String? _syncAddress;

  /// Embedded (desktop) only: shows [FormatsScreen] in place of the profile
  /// instead of pushing it over the shell's sidebar and list — see
  /// [openNestedScreen], which the "File formats" row's `onTap` calls.
  bool _showFormats = false;

  bridge.DeviceIdentityInfo? get _identity => DeviceIdentity.instance.info;
  String? get _error => DeviceIdentity.instance.lastError;

  @override
  void initState() {
    super.initState();
    DeviceIdentity.instance.load();
    _loadSyncAddress();
  }

  Future<void> _loadSyncAddress() async {
    // Separate from identity: fetching the sync address is what starts the
    // listener, making this device reachable for as long as the app runs.
    try {
      final addr = await Collections.instance.syncAddress();
      if (mounted) setState(() => _syncAddress = addr);
    } catch (_) {
      // Backend unavailable — the row stays hidden rather than showing a
      // made-up address.
    }
  }

  Future<void> _rename() async {
    final result = await promptForText(
      context,
      title: 'Your name',
      initialValue: _identity?.nickname,
      helper: 'This is how you appear to collaborators.',
    );
    if (result == null || result.isEmpty || !mounted) return;
    try {
      await DeviceIdentity.instance.rename(result);
    } catch (e) {
      if (mounted) showToast(context, 'Couldn\'t rename: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    if (_showFormats) {
      return FormatsScreen(
        embedded: widget.embedded,
        onBack: () => setState(() => _showFormats = false),
      );
    }
    return ListenableBuilder(
      listenable: Listenable.merge(
        [Collections.instance, DeviceIdentity.instance],
      ),
      builder: (context, _) {
        // Read inside the builder, not above it: only the builder re-runs when
        // the identity changes, so a name computed outside would still be the
        // one from before the rename.
        final nickname = _identity?.nickname ?? '…';
        final initials = _identity != null && nickname.isNotEmpty
            ? nickname[0].toUpperCase()
            : '·';
        final collections = Collections.instance.collections;
        final sent = collections.fold<int>(0, (s, c) => s + c.uploadedBytes);
        final received =
            collections.fold<int>(0, (s, c) => s + c.downloadedBytes);
        // One person can collaborate on several collections; count devices,
        // not memberships.
        final people = <String>{
          for (final c in collections)
            for (final p in c.collaborators) p.deviceId,
        }.length;

        return PageBody(
          child: SingleChildScrollView(
            padding: const EdgeInsets.only(bottom: 28),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SizedBox(height: 28),
                Center(
                  child: Column(
                    children: [
                      Avatar(initials: initials, size: 76, primary: true),
                      const SizedBox(height: 16),
                      CanvasTitle(nickname, size: 30),
                      const SizedBox(height: 6),
                      Text(
                        _identity == null
                            ? (_error == null
                                ? 'LOADING…'
                                : 'IDENTITY UNAVAILABLE')
                            : 'THIS DEVICE · '
                                '${_identity!.deviceId.substring(0, _identity!.deviceId.length.clamp(0, 8)).toUpperCase()}',
                        style: monoLabel(size: 10.5, letterSpacing: 0.6),
                      ),
                      const SizedBox(height: 16),
                      PillButton(
                        label: 'Change name',
                        dim: true,
                        onTap: _identity == null ? null : _rename,
                      ),
                    ],
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 26, 22, 0),
                  child: GridView.count(
                    shrinkWrap: true,
                    physics: const NeverScrollableScrollPhysics(),
                    crossAxisCount: 2,
                    mainAxisSpacing: 10,
                    crossAxisSpacing: 10,
                    childAspectRatio: 2.0,
                    children: [
                      // "This session", not a lifetime total — the engine
                      // keeps no running counter across restarts.
                      _Stat(label: 'SENT · SESSION', value: formatBytes(sent)),
                      _Stat(
                        label: 'RECEIVED · SESSION',
                        value: formatBytes(received),
                        highlight: received > 0,
                      ),
                      _Stat(
                          label: 'COLLECTIONS', value: '${collections.length}'),
                      // People now has its own destination on both layouts —
                      // a bottom tab here, the header button on desktop — so
                      // this card is a shortcut rather than the only way in.
                      // It switches the shared tab rather than pushing a
                      // second copy of the same screen on top of it.
                      _Stat(
                        label: 'PEOPLE',
                        value: '$people',
                        onTap: widget.embedded
                            ? null
                            : () => AppNavigation.tab.value = 2,
                      ),
                    ],
                  ),
                ),
                if (_syncAddress != null)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(22, 22, 22, 0),
                    child: SurfaceCard(
                      padding: const EdgeInsets.all(16),
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text('YOUR ADDRESS', style: monoLabel(size: 10)),
                          const SizedBox(height: 9),
                          Row(
                            children: [
                              Expanded(
                                child: Text(
                                  _syncAddress!,
                                  style: monoLabel(
                                      size: 12.5,
                                      color: AppColors.text,
                                      letterSpacing: 0),
                                ),
                              ),
                              InkWell(
                                onTap: () {
                                  Clipboard.setData(
                                      ClipboardData(text: _syncAddress!));
                                  showToast(context, 'Address copied');
                                },
                                child: const Padding(
                                  padding: EdgeInsets.all(4),
                                  child: Icon(Icons.copy,
                                      size: 15, color: AppColors.textDim),
                                ),
                              ),
                            ],
                          ),
                          const SizedBox(height: 9),
                          Text(
                            'Rotates every launch. Collaborators reach this '
                            'device here to exchange collection contents.',
                            style: AppText.secondary(height: 1.45),
                          ),
                        ],
                      ),
                    ),
                  ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                  // No "Back up identity" action: nothing in device.rs can
                  // export the keypair yet, and a button that does nothing
                  // is worse than none.
                  child: DestinationRow(
                    icon: Icons.shield_outlined,
                    iconColor: AppColors.signal,
                    title: 'Identity lives on this device',
                    subtitle: 'A key pair, no account, no server. Lose the '
                        'device and the identity goes with it.',
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                  child: DestinationRow(
                    icon: Icons.category_outlined,
                    title: 'File formats',
                    subtitle: 'What Portalis can view, and what it converts',
                    onTap: () => openNestedScreen(
                      context,
                      embedded: widget.embedded,
                      showInPlace: () => setState(() => _showFormats = true),
                      push: (_) => const FormatsScreen(),
                    ),
                  ),
                ),
                // On desktop Settings is its own sidebar destination, so this
                // row would push a full screen over a layout that already has
                // a place to put it. (People is reached from its own count
                // in the grid above, on both layouts.)
                if (!widget.embedded)
                  Padding(
                    padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                    child: DestinationRow(
                      icon: Icons.tune,
                      title: 'Settings',
                      onTap: () => Navigator.of(context).push(
                        MaterialPageRoute(
                            builder: (_) => const SettingsScreen()),
                      ),
                    ),
                  ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                  child: Center(
                    child: Text('PORTALIS · NO ACCOUNT · NO SERVER',
                        style: monoLabel(size: 10, color: AppColors.textGhost)),
                  ),
                ),
              ],
            ),
          ),
        );
      },
    );
  }
}

class _Stat extends StatelessWidget {
  const _Stat({
    required this.label,
    required this.value,
    this.highlight = false,
    this.onTap,
  });

  final String label;
  final String value;

  /// Mint only when the figure represents data that actually moved.
  final bool highlight;

  /// Set when the figure has somewhere to go — PEOPLE opens the directory
  /// the number counts. A chevron beside the label says so, since a card
  /// that merely states a number gives no reason to try tapping it.
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      padding: const EdgeInsets.all(14),
      onTap: onTap,
      // Scaled to fit rather than fixed: the label is a long uppercase mono
      // string and the value can be four characters wide, and the card has a
      // fixed aspect ratio in the grid.
      child: FittedBox(
        fit: BoxFit.scaleDown,
        alignment: Alignment.centerLeft,
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.start,
          mainAxisSize: MainAxisSize.min,
          children: [
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Text(label, style: monoLabel(size: 9.5)),
                if (onTap != null) ...[
                  const SizedBox(width: 4),
                  const Icon(Icons.chevron_right,
                      size: 13, color: AppColors.textGhost),
                ],
              ],
            ),
            const SizedBox(height: 6),
            Text(
              value,
              style: displayText(
                size: 24,
                color: highlight ? AppColors.signal : AppColors.text,
              ),
            ),
          ],
        ),
      ),
    );
  }
}
