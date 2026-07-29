import 'package:flutter/material.dart';
import '../theme.dart';
import '../widgets/common.dart';

class UserScreen extends StatelessWidget {
  const UserScreen({super.key});

  @override
  Widget build(BuildContext context) {
    const stats = [
      ('Shared', '3.4 GB'),
      ('Received', '1.1 GB'),
      ('Collections', '4'),
      ('Uptime', '92%'),
    ];
    const devices = [
      ('M', 'Maya’s iPhone — this device'),
      ('M', 'Maya’s MacBook Pro'),
    ];

    return SingleChildScrollView(
      child: Column(
        children: [
          const SizedBox(height: 70),
          const Avatar(initials: 'M', size: 64),
          const SizedBox(height: 6),
          const Text(
            'Maya',
            style: TextStyle(fontSize: 17, fontWeight: FontWeight.w500),
          ),
          const SizedBox(height: 2),
          const Text(
            'nickname + avatar · no account needed',
            style: TextStyle(fontSize: 11, color: AppColors.neutral400),
          ),
          const SizedBox(height: 8),
          PillButton(
            label: '✎ Edit',
            dim: true,
            onTap: () {},
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
            child: GridView.count(
              shrinkWrap: true,
              physics: const NeverScrollableScrollPhysics(),
              crossAxisCount: 2,
              mainAxisSpacing: 8,
              crossAxisSpacing: 8,
              childAspectRatio: 2.1,
              children: [
                for (final (label, value) in stats)
                  Container(
                    padding:
                        const EdgeInsets.symmetric(horizontal: 12, vertical: 10),
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
            ),
          ),
          Padding(
            padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                const SectionLabel('YOUR DEVICES'),
                const SizedBox(height: 8),
                for (final (initials, label) in devices)
                  Padding(
                    padding: const EdgeInsets.only(bottom: 10),
                    child: Row(
                      children: [
                        Avatar(initials: initials, size: 26),
                        const SizedBox(width: 10),
                        Expanded(
                          child: Text(
                            label,
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
                  style: TextStyle(fontSize: 10.5, height: 1.5, color: AppColors.neutral500),
                ),
              ],
            ),
          ),
          const SizedBox(height: 24),
        ],
      ),
    );
  }
}
