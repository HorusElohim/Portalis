import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../bridge_generated/device.dart' as bridge;
import '../services/collections.dart';
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
  const UserScreen({super.key});

  @override
  State<UserScreen> createState() => _UserScreenState();
}

class _UserScreenState extends State<UserScreen> {
  bridge.DeviceIdentityInfo? _identity;
  String? _error;
  String? _syncAddress;

  @override
  void initState() {
    super.initState();
    _load();
  }

  Future<void> _load() async {
    try {
      final identity = await bridge.deviceIdentity();
      if (mounted) setState(() => _identity = identity);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
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
      final identity = await bridge.setNickname(nickname: result);
      if (mounted) setState(() => _identity = identity);
    } catch (e) {
      if (mounted) showToast(context, 'Couldn\'t rename: $e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final nickname = _identity?.nickname ?? '…';
    final initials =
        _identity != null && nickname.isNotEmpty ? nickname[0].toUpperCase() : '·';

    return ListenableBuilder(
      listenable: Collections.instance,
      builder: (context, _) {
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
                      Text(nickname, style: displayText(size: 26)),
                      const SizedBox(height: 6),
                      Text(
                        _identity == null
                            ? (_error == null ? 'LOADING…' : 'IDENTITY UNAVAILABLE')
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
                          label: 'COLLECTIONS',
                          value: '${collections.length}'),
                      _Stat(label: 'PEOPLE', value: '$people'),
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
                          const Text(
                            'Rotates every launch. Collaborators reach this '
                            'device here to exchange collection contents.',
                            style: TextStyle(
                                fontSize: 12.5,
                                height: 1.45,
                                color: AppColors.textFaint),
                          ),
                        ],
                      ),
                    ),
                  ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                  child: SurfaceCard(
                    padding: const EdgeInsets.all(16),
                    child: Row(
                      crossAxisAlignment: CrossAxisAlignment.start,
                      children: [
                        Icon(Icons.shield_outlined,
                            size: 19, color: AppColors.signal),
                        const SizedBox(width: 13),
                        const Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text('Identity lives on this device',
                                  style: TextStyle(
                                      fontSize: 14.5,
                                      fontWeight: FontWeight.w600)),
                              SizedBox(height: 4),
                              // No "Back up identity" action: nothing in
                              // device.rs can export the keypair yet, and a
                              // button that does nothing is worse than none.
                              Text(
                                'A key pair, no account, no server. Lose the '
                                'device and the identity goes with it.',
                                style: TextStyle(
                                    fontSize: 12.5,
                                    height: 1.45,
                                    color: AppColors.textFaint),
                              ),
                            ],
                          ),
                        ),
                      ],
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                  child: SurfaceCard(
                    padding: const EdgeInsets.all(16),
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute(
                          builder: (_) => const FormatsScreen()),
                    ),
                    child: const Row(
                      children: [
                        Icon(Icons.category_outlined,
                            size: 19, color: AppColors.textDim),
                        SizedBox(width: 13),
                        Expanded(
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Text('File formats',
                                  style: TextStyle(
                                      fontSize: 14.5,
                                      fontWeight: FontWeight.w600)),
                              SizedBox(height: 3),
                              Text(
                                'What Portalis can view, and what it converts',
                                style: TextStyle(
                                    fontSize: 12.5,
                                    color: AppColors.textFaint),
                              ),
                            ],
                          ),
                        ),
                        Icon(Icons.chevron_right,
                            size: 16, color: AppColors.textGhost),
                      ],
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 12, 22, 0),
                  child: SurfaceCard(
                    padding: const EdgeInsets.all(16),
                    onTap: () => Navigator.of(context).push(
                      MaterialPageRoute(
                          builder: (_) => const SettingsScreen()),
                    ),
                    child: const Row(
                      children: [
                        Icon(Icons.tune, size: 19, color: AppColors.textDim),
                        SizedBox(width: 13),
                        Expanded(
                          child: Text('Settings',
                              style: TextStyle(
                                  fontSize: 14.5,
                                  fontWeight: FontWeight.w600)),
                        ),
                        Icon(Icons.chevron_right,
                            size: 16, color: AppColors.textGhost),
                      ],
                    ),
                  ),
                ),
                Padding(
                  padding: const EdgeInsets.fromLTRB(22, 20, 22, 0),
                  child: Center(
                    child: Text('PORTALIS · NO ACCOUNT · NO SERVER',
                        style:
                            monoLabel(size: 10, color: AppColors.textGhost)),
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
  });

  final String label;
  final String value;

  /// Mint only when the figure represents data that actually moved.
  final bool highlight;

  @override
  Widget build(BuildContext context) {
    return SurfaceCard(
      padding: const EdgeInsets.all(14),
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
            Text(label, style: monoLabel(size: 9.5)),
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
