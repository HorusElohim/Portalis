import 'package:flutter/material.dart';
import '../bridge_generated/device.dart' as bridge;
import '../services/torrent_collections.dart';
import '../theme.dart';
import '../widgets/common.dart';

String _formatBytes(int bytes) {
  const gb = 1000000000;
  const mb = 1000000;
  if (bytes >= gb) return '${(bytes / gb).toStringAsFixed(1)} GB';
  if (bytes >= mb) return '${(bytes / mb).toStringAsFixed(0)} MB';
  return '$bytes B';
}

class UserScreen extends StatefulWidget {
  const UserScreen({super.key});

  @override
  State<UserScreen> createState() => _UserScreenState();
}

class _UserScreenState extends State<UserScreen> {
  bridge.DeviceIdentityInfo? _identity;
  String? _error;

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
  }

  Future<void> _editNickname() async {
    final controller = TextEditingController(text: _identity?.nickname ?? '');
    final result = await showDialog<String>(
      context: context,
      builder: (context) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Nickname'),
        content: TextField(
          controller: controller,
          autofocus: true,
          style: const TextStyle(color: AppColors.text),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(context).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(context).pop(controller.text.trim()),
            child: const Text('Save'),
          ),
        ],
      ),
    );
    if (result == null || result.isEmpty) return;
    try {
      final identity = await bridge.setNickname(nickname: result);
      if (mounted) setState(() => _identity = identity);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
  }

  @override
  Widget build(BuildContext context) {
    final nickname = _identity?.nickname ?? '…';
    final initials = nickname.isNotEmpty ? nickname[0].toUpperCase() : '?';

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: SingleChildScrollView(
          child: Column(
            children: [
              Align(
                alignment: Alignment.centerLeft,
                child: NavBackButton(onTap: () => Navigator.of(context).pop()),
              ),
              const SizedBox(height: 10),
              Avatar(initials: initials, size: 64),
              const SizedBox(height: 6),
              Text(
                nickname,
                style: const TextStyle(fontSize: 17, fontWeight: FontWeight.w500),
              ),
              const SizedBox(height: 2),
              Text(
                _identity == null
                    ? (_error ?? 'loading…')
                    : 'device ${_identity!.deviceId.substring(0, _identity!.deviceId.length.clamp(0, 12))}… · no account needed',
                style: const TextStyle(fontSize: 11, color: AppColors.neutral400),
              ),
              const SizedBox(height: 8),
              PillButton(
                label: '✎ Edit',
                dim: true,
                onTap: _identity == null ? null : _editNickname,
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
                child: ListenableBuilder(
                  listenable: TorrentCollections.instance,
                  builder: (context, _) {
                    final collections = TorrentCollections.instance.collections;
                    final shared = collections.fold<int>(
                        0, (sum, c) => sum + c.uploadedBytes);
                    final received = collections.fold<int>(
                        0, (sum, c) => sum + c.downloadedBytes);
                    final peers = collections.fold<int>(
                        0, (sum, c) => sum + c.collaboratorCount);
                    final stats = [
                      ('Shared', _formatBytes(shared)),
                      ('Received', _formatBytes(received)),
                      ('Collections', '${collections.length}'),
                      ('Peers', '$peers'),
                    ];
                    return GridView.count(
                      shrinkWrap: true,
                      physics: const NeverScrollableScrollPhysics(),
                      crossAxisCount: 2,
                      mainAxisSpacing: 8,
                      crossAxisSpacing: 8,
                      childAspectRatio: 2.1,
                      children: [
                        for (final (label, value) in stats)
                          Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 10),
                            decoration: BoxDecoration(
                              color: AppColors.surface,
                              border: Border.all(color: AppColors.border),
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Column(
                              crossAxisAlignment: CrossAxisAlignment.start,
                              mainAxisAlignment: MainAxisAlignment.center,
                              children: [
                                Text(
                                  label,
                                  style: const TextStyle(
                                    fontSize: 9,
                                    fontFamily: 'monospace',
                                    color: AppColors.neutral400,
                                  ),
                                ),
                                const SizedBox(height: 3),
                                Text(
                                  value,
                                  style: const TextStyle(
                                    fontSize: 15,
                                    fontWeight: FontWeight.w500,
                                    color: AppColors.accent300,
                                  ),
                                ),
                              ],
                            ),
                          ),
                      ],
                    );
                  },
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    const SectionLabel('YOUR DEVICES'),
                    const SizedBox(height: 8),
                    Padding(
                      padding: const EdgeInsets.only(bottom: 10),
                      child: Row(
                        children: [
                          Avatar(initials: initials, size: 26),
                          const SizedBox(width: 10),
                          Expanded(
                            child: Text(
                              '$nickname — this device',
                              overflow: TextOverflow.ellipsis,
                              style: const TextStyle(fontSize: 12.5),
                            ),
                          ),
                        ],
                      ),
                    ),
                    const Text(
                      'Your identity is a key pair on your devices. Lose them all '
                      'and the identity is gone — back it up in Settings.',
                      style: TextStyle(
                          fontSize: 10.5,
                          height: 1.5,
                          color: AppColors.neutral500),
                    ),
                  ],
                ),
              ),
              const SizedBox(height: 24),
            ],
          ),
        ),
      ),
    );
  }
}
