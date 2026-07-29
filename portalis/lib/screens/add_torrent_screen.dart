import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../services/torrent_collections.dart';
import '../theme.dart';
import '../widgets/common.dart';

/// Entry point for adding a new collection via magnet link or raw
/// `.torrent` file. A torrent's files become the collection's media — see
/// `TorrentCollections` for why that mapping works so well. Pushed from
/// Home's "＋ Add torrent" button; on success it pops back to Home, which
/// picks up the new collection live (`TorrentCollections` is already
/// polling).
class AddTorrentScreen extends StatefulWidget {
  const AddTorrentScreen({super.key});

  @override
  State<AddTorrentScreen> createState() => _AddTorrentScreenState();
}

class _AddTorrentScreenState extends State<AddTorrentScreen> {
  final _magnetController = TextEditingController();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _magnetController.dispose();
    super.dispose();
  }

  Future<void> _addFromMagnet() async {
    final magnet = _magnetController.text.trim();
    if (magnet.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await TorrentCollections.instance.addFromMagnet(magnet);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      setState(() => _error = 'Couldn\'t add magnet link: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _addFromFile() async {
    final result = await FilePicker.pickFiles(withData: true, type: FileType.any);
    final bytes = result?.files.single.bytes;
    if (bytes == null) return;

    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await TorrentCollections.instance.addFromFileBytes(bytes);
      if (mounted) Navigator.of(context).pop();
    } catch (e) {
      setState(() => _error = 'Couldn\'t add .torrent file: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            Padding(
              padding: const EdgeInsets.fromLTRB(14, 0, 14, 6),
              child: Row(
                mainAxisAlignment: MainAxisAlignment.spaceBetween,
                children: [
                  _CircleCloseButton(onTap: () => Navigator.of(context).pop()),
                  const Text(
                    'Add torrent',
                    style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                  ),
                  const SizedBox(width: 34),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 8, 20, 0),
              child: Text(
                'Its files become the collection\'s media — most torrents '
                'already bundle several related files.',
                style: TextStyle(fontSize: 11.5, color: AppColors.neutral400),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 16, 20, 0),
              child: Row(
                children: [
                  Expanded(
                    child: TextField(
                      controller: _magnetController,
                      style: const TextStyle(color: AppColors.text, fontSize: 13),
                      decoration: InputDecoration(
                        hintText: 'magnet:?xt=urn:btih:...',
                        hintStyle: const TextStyle(color: AppColors.neutral500),
                        filled: true,
                        fillColor: AppColors.surface,
                        contentPadding:
                            const EdgeInsets.symmetric(horizontal: 12, vertical: 11),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(8),
                          borderSide: const BorderSide(color: AppColors.borderStrong),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(8),
                          borderSide: const BorderSide(color: AppColors.borderStrong),
                        ),
                      ),
                      onSubmitted: (_) => _addFromMagnet(),
                    ),
                  ),
                  const SizedBox(width: 10),
                  PillButton(label: 'Add', onTap: _busy ? null : _addFromMagnet),
                ],
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 12, 20, 0),
              child: Align(
                alignment: Alignment.centerLeft,
                child: PillButton(
                  label: '📄 Pick .torrent file',
                  dim: true,
                  onTap: _busy ? null : _addFromFile,
                ),
              ),
            ),
            if (_error != null)
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 14, 20, 0),
                child: Container(
                  padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
                  decoration: BoxDecoration(
                    color: const Color(0xFFEB5757).withValues(alpha: 0.1),
                    border: Border.all(color: const Color(0xFFEB5757).withValues(alpha: 0.4)),
                    borderRadius: BorderRadius.circular(8),
                  ),
                  child: Text(
                    _error!,
                    style: const TextStyle(fontSize: 11, color: Color(0xFFEB5757)),
                  ),
                ),
              ),
            if (_busy)
              const Padding(
                padding: EdgeInsets.only(top: 20),
                child: Center(
                  child: SizedBox(
                    width: 20,
                    height: 20,
                    child: CircularProgressIndicator(
                      strokeWidth: 2,
                      valueColor: AlwaysStoppedAnimation(AppColors.accent),
                    ),
                  ),
                ),
              ),
          ],
        ),
      ),
    );
  }
}

class _CircleCloseButton extends StatelessWidget {
  const _CircleCloseButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: Colors.transparent,
      shape: CircleBorder(side: BorderSide(color: AppColors.borderStrong)),
      child: InkWell(
        customBorder: const CircleBorder(),
        onTap: onTap,
        child: const SizedBox(
          width: 34,
          height: 34,
          child: Icon(Icons.close, size: 18, color: AppColors.text),
        ),
      ),
    );
  }
}
