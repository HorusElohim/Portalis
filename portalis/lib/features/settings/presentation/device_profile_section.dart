import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../collections/domain/collection.dart';
import '../../identity/domain/device_profile.dart';

/// Device identity, session totals, and identity-related destinations.
class DeviceProfileSection extends StatelessWidget {
  const DeviceProfileSection({
    super.key,
    required this.profile,
    required this.identityError,
    required this.collections,
    required this.syncAddress,
    required this.onRename,
    required this.onOpenPeople,
    required this.onOpenFormats,
  });

  final DeviceProfile? profile;
  final String? identityError;
  final List<Collection> collections;
  final String? syncAddress;
  final VoidCallback? onRename;
  final VoidCallback onOpenPeople;
  final VoidCallback onOpenFormats;

  @override
  Widget build(BuildContext context) {
    final nickname = profile?.nickname ?? '…';
    final initials = profile != null && nickname.isNotEmpty
        ? nickname[0].toUpperCase()
        : '·';
    final sent = collections.fold<int>(0, (sum, item) => sum + item.uploadedBytes);
    final received =
        collections.fold<int>(0, (sum, item) => sum + item.downloadedBytes);
    final people = <String>{
      for (final collection in collections)
        for (final collaborator in collection.collaborators) collaborator.deviceId,
    }.length;

    return Padding(
      padding: const EdgeInsets.fromLTRB(0, 4, 0, 26),
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Center(
            child: Column(
              children: [
                Avatar(initials: initials, size: 76, primary: true),
                const SizedBox(height: 16),
                CanvasTitle(nickname, size: 30),
                const SizedBox(height: 6),
                Text(
                  profile == null
                      ? (identityError == null
                          ? 'LOADING…'
                          : 'IDENTITY UNAVAILABLE')
                      : 'THIS DEVICE · '
                          '${profile!.deviceId.substring(0, profile!.deviceId.length.clamp(0, 8)).toUpperCase()}',
                  style: monoLabel(size: 10.5, letterSpacing: 0.6),
                ),
                const SizedBox(height: 16),
                PillButton(
                  label: 'Change name',
                  dim: true,
                  onTap: onRename,
                ),
              ],
            ),
          ),
          Padding(
            padding:
                const EdgeInsets.fromLTRB(kScreenGutter, 26, kScreenGutter, 0),
            child: GridView.count(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              crossAxisCount: 2,
              mainAxisSpacing: 10,
              crossAxisSpacing: 10,
              childAspectRatio: 2.0,
              children: [
                _ProfileStat(label: 'SENT · SESSION', value: formatBytes(sent)),
                _ProfileStat(
                  label: 'RECEIVED · SESSION',
                  value: formatBytes(received),
                  highlight: received > 0,
                ),
                _ProfileStat(
                  label: 'COLLECTIONS',
                  value: '${collections.length}',
                ),
                _ProfileStat(
                  label: 'PEOPLE',
                  value: '$people',
                  onTap: onOpenPeople,
                ),
              ],
            ),
          ),
          if (syncAddress != null) _SyncAddressCard(address: syncAddress!),
          Padding(
            padding: const EdgeInsets.fromLTRB(
              kScreenGutter,
              12,
              kScreenGutter,
              0,
            ),
            child: DestinationRow(
              icon: Icons.shield_outlined,
              iconColor: AppColors.signal,
              title: 'Identity lives on this device',
              subtitle: 'A key pair, no account, no server. Lose the device '
                  'and the identity goes with it.',
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(
              kScreenGutter,
              12,
              kScreenGutter,
              0,
            ),
            child: DestinationRow(
              icon: Icons.category_outlined,
              title: 'File formats',
              subtitle: 'What Portalis can view, and what it converts',
              onTap: onOpenFormats,
            ),
          ),
        ],
      ),
    );
  }
}

class _SyncAddressCard extends StatelessWidget {
  const _SyncAddressCard({required this.address});

  final String address;

  @override
  Widget build(BuildContext context) => Padding(
        padding:
            const EdgeInsets.fromLTRB(kScreenGutter, 22, kScreenGutter, 0),
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
                      address,
                      style: monoLabel(
                        size: 12.5,
                        color: AppColors.text,
                        letterSpacing: 0,
                      ),
                    ),
                  ),
                  InkWell(
                    onTap: () {
                      Clipboard.setData(ClipboardData(text: address));
                      showToast(context, 'Address copied');
                    },
                    child: Padding(
                      padding: const EdgeInsets.all(4),
                      child: Icon(Icons.copy, size: 15, color: AppColors.textDim),
                    ),
                  ),
                ],
              ),
              const SizedBox(height: 9),
              Text(
                'Rotates every launch. Collaborators reach this device here '
                'to exchange collection contents.',
                style: AppText.secondary(height: 1.45),
              ),
            ],
          ),
        ),
      );
}

class _ProfileStat extends StatelessWidget {
  const _ProfileStat({
    required this.label,
    required this.value,
    this.highlight = false,
    this.onTap,
  });

  final String label;
  final String value;
  final bool highlight;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) => SurfaceCard(
        padding: const EdgeInsets.all(14),
        onTap: onTap,
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
                    Icon(
                      Icons.chevron_right,
                      size: 13,
                      color: AppColors.textGhost,
                    ),
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
