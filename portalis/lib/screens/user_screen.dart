import 'package:flutter/material.dart';
import 'package:flutter/services.dart';
import 'package:qr_flutter/qr_flutter.dart';

import '../bridge_generated/collab.dart' as collab_bridge;
import '../bridge_generated/device.dart' as bridge;
import '../services/collab_collections.dart';
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
  String? _syncAddress;

  @override
  void initState() {
    super.initState();
    _load();
    CollabCollections.instance.refresh();
  }

  Future<void> _load() async {
    try {
      final identity = await bridge.deviceIdentity();
      if (mounted) setState(() => _identity = identity);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    }
    // Separately from identity: fetching the sync address is what starts
    // the sync listener, making this device reachable by collaborators for
    // as long as the app runs.
    try {
      final addr = await CollabCollections.instance.syncAddress();
      if (mounted) setState(() => _syncAddress = addr);
    } catch (_) {
      // Backend unavailable (e.g. widget tests) — the row just stays
      // hidden.
    }
  }

  Future<void> _syncCollection(collab_bridge.CollabCollectionInfo c) async {
    final controller = TextEditingController();
    final peerAddr = await showDialog<String>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text('Sync "${c.name}"'),
        content: TextField(
          controller: controller,
          autofocus: true,
          style: const TextStyle(
              color: AppColors.text, fontSize: 13, fontFamily: 'monospace'),
          decoration: const InputDecoration(
            hintText: '192.168.1.23:54321',
            hintStyle: TextStyle(color: AppColors.neutral500),
            helperText: 'The other device\'s sync address (shown on its User screen)',
            helperStyle: TextStyle(fontSize: 10.5, color: AppColors.neutral400),
          ),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(controller.text.trim()),
            child: const Text('Sync'),
          ),
        ],
      ),
    );
    if (peerAddr == null || peerAddr.isEmpty || !mounted) return;

    try {
      final updated = await CollabCollections.instance.sync(c.id, peerAddr);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
        content: Text(
            'Synced "${updated.name}" — ${updated.media.length} media, ${updated.collaborators.length} collaborators'),
      ));
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Sync failed: $e')),
      );
    }
  }

  /// Re-shows a collab collection's invite code/QR — the same view "Add
  /// Collab" shows at creation time, reachable again here since Phase 1
  /// has no other durable place to go looking for it.
  Future<void> _viewInvite(collab_bridge.CollabCollectionInfo c) async {
    await showDialog<void>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: Text(c.name),
        content: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Text(
              '${c.collaborators.length} collaborator${c.collaborators.length == 1 ? '' : 's'} · '
              '${c.media.length} media item${c.media.length == 1 ? '' : 's'}',
              style: const TextStyle(fontSize: 12, color: AppColors.neutral400),
            ),
            const SizedBox(height: 14),
            Center(
              child: Container(
                padding: const EdgeInsets.all(12),
                decoration: BoxDecoration(
                  color: Colors.white,
                  borderRadius: BorderRadius.circular(10),
                ),
                child: QrImageView(
                  data: c.inviteCode,
                  version: QrVersions.auto,
                  size: 200,
                  backgroundColor: Colors.white,
                  eyeStyle: const QrEyeStyle(
                    eyeShape: QrEyeShape.square,
                    color: Colors.black,
                  ),
                  dataModuleStyle: const QrDataModuleStyle(
                    dataModuleShape: QrDataModuleShape.square,
                    color: Colors.black,
                  ),
                ),
              ),
            ),
            const SizedBox(height: 14),
            SelectableText(
              c.inviteCode,
              style: const TextStyle(fontFamily: 'monospace', fontSize: 12),
            ),
          ],
        ),
        actions: [
          TextButton(
            onPressed: () {
              Clipboard.setData(ClipboardData(text: c.inviteCode));
              ScaffoldMessenger.of(dialogContext).showSnackBar(
                const SnackBar(content: Text('Invite code copied')),
              );
            },
            child: const Text('Copy'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(),
            child: const Text('Done'),
          ),
        ],
      ),
    );
  }

  Future<void> _deleteCollabCollection(collab_bridge.CollabCollectionInfo c) async {
    final confirmed = await showDialog<bool>(
      context: context,
      builder: (dialogContext) => AlertDialog(
        backgroundColor: AppColors.surface,
        title: const Text('Delete collection?'),
        content: Text(
          'Removes "${c.name}" from this device only. Other collaborators '
          '(if any have already synced) keep their own copy.',
          style: const TextStyle(fontSize: 12, color: AppColors.neutral400),
        ),
        actions: [
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(false),
            child: const Text('Cancel'),
          ),
          TextButton(
            onPressed: () => Navigator.of(dialogContext).pop(true),
            child: const Text('Delete', style: TextStyle(color: Color(0xFFEB5757))),
          ),
        ],
      ),
    );
    if (confirmed != true) return;
    try {
      await CollabCollections.instance.deleteCollection(c.id);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Deleted "${c.name}"')),
      );
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Couldn\'t delete: $e')),
      );
    }
  }

  Future<void> _fetchCollectionMedia(collab_bridge.CollabCollectionInfo c) async {
    try {
      final started = await CollabCollections.instance.fetchAllMedia(c);
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(SnackBar(
        content: Text(started == 0
            ? 'No media in "${c.name}" yet — sync with a collaborator first'
            : 'Fetching $started item${started == 1 ? '' : 's'} — they\'ll appear on Home as they download'),
      ));
    } catch (e) {
      if (!mounted) return;
      ScaffoldMessenger.of(context).showSnackBar(
        SnackBar(content: Text('Fetch failed: $e')),
      );
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
                    const SectionLabel('COLLAB COLLECTIONS · EXPERIMENTAL'),
                    const SizedBox(height: 8),
                    if (_syncAddress != null)
                      Padding(
                        padding: const EdgeInsets.only(bottom: 8),
                        child: Row(
                          children: [
                            const Text(
                              'Sync address: ',
                              style: TextStyle(
                                  fontSize: 11, color: AppColors.neutral400),
                            ),
                            Flexible(
                              child: Text(
                                _syncAddress!,
                                overflow: TextOverflow.ellipsis,
                                style: const TextStyle(
                                  fontSize: 11,
                                  fontFamily: 'monospace',
                                  color: AppColors.accent300,
                                ),
                              ),
                            ),
                            const SizedBox(width: 6),
                            InkWell(
                              onTap: () {
                                Clipboard.setData(
                                    ClipboardData(text: _syncAddress!));
                                ScaffoldMessenger.of(context).showSnackBar(
                                  const SnackBar(
                                      content: Text('Sync address copied')),
                                );
                              },
                              child: const Icon(Icons.copy,
                                  size: 13, color: AppColors.neutral400),
                            ),
                          ],
                        ),
                      ),
                    ListenableBuilder(
                      listenable: CollabCollections.instance,
                      builder: (context, _) {
                        final collabCollections = CollabCollections.instance.collections;
                        if (collabCollections.isEmpty) {
                          return const Padding(
                            padding: EdgeInsets.only(bottom: 4),
                            child: Text(
                              'None yet — create or join one from the Add screen.',
                              style: TextStyle(fontSize: 11, color: AppColors.neutral500),
                            ),
                          );
                        }
                        return Column(
                          crossAxisAlignment: CrossAxisAlignment.start,
                          children: [
                            for (final c in collabCollections)
                              Material(
                                color: Colors.transparent,
                                child: InkWell(
                                  onTap: () => _viewInvite(c),
                                  borderRadius: BorderRadius.circular(6),
                                  child: Padding(
                                    padding: const EdgeInsets.symmetric(vertical: 6),
                                    child: Row(
                                      children: [
                                        Expanded(
                                          child: Text(
                                            c.name,
                                            overflow: TextOverflow.ellipsis,
                                            style: const TextStyle(fontSize: 12.5),
                                          ),
                                        ),
                                        Text(
                                          '${c.collaborators.length} collab · ${c.media.length} media',
                                          style: const TextStyle(
                                            fontSize: 10.5,
                                            fontFamily: 'monospace',
                                            color: AppColors.neutral400,
                                          ),
                                        ),
                                        const SizedBox(width: 8),
                                        InkWell(
                                          onTap: () => _syncCollection(c),
                                          child: const Padding(
                                            padding: EdgeInsets.all(3),
                                            child: Icon(Icons.sync,
                                                size: 15, color: AppColors.accent300),
                                          ),
                                        ),
                                        InkWell(
                                          onTap: () => _fetchCollectionMedia(c),
                                          child: const Padding(
                                            padding: EdgeInsets.all(3),
                                            child: Icon(Icons.download_outlined,
                                                size: 15, color: AppColors.accent300),
                                          ),
                                        ),
                                        InkWell(
                                          onTap: () => _deleteCollabCollection(c),
                                          child: const Padding(
                                            padding: EdgeInsets.all(3),
                                            child: Icon(Icons.delete_outline,
                                                size: 15, color: Color(0xFFEB5757)),
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                ),
                              ),
                          ],
                        );
                      },
                    ),
                  ],
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
