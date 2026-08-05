import 'package:flutter/material.dart';

import '../../../design/design.dart';

/// Secondary action beside the wide-layout primary share action.
class AddTorrentAction extends StatelessWidget {
  const AddTorrentAction({super.key, required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) => OutlinedIconActionButton(
        key: const Key('addTorrentButton'),
        tone: ActionButtonTone.ember,
        icon: Icons.download_outlined,
        tooltip: 'Add a torrent',
        onTap: onTap,
      );
}
