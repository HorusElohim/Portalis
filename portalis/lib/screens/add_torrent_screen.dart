import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';

import '../media_convert.dart';
import '../services/torrent_collections.dart';
import '../theme.dart';
import '../widgets/common.dart';

/// Entry point for adding a new collection, either side of the swarm:
/// **share** your own files (a new collection, seeded immediately) or
/// **join** one that already exists (magnet link / `.torrent` file). Both
/// end up as the exact same kind of `Collection` — see `TorrentCollections`
/// for why. Pushed from Home's "＋ Add torrent" button; on success it pops
/// back to Home, which picks up the new collection live (already polling).
class AddTorrentScreen extends StatefulWidget {
  const AddTorrentScreen({super.key});

  @override
  State<AddTorrentScreen> createState() => _AddTorrentScreenState();
}

class _AddTorrentScreenState extends State<AddTorrentScreen> {
  final _nameController = TextEditingController();
  final _magnetController = TextEditingController();
  List<PlatformFile> _pickedFiles = [];
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _nameController.dispose();
    _magnetController.dispose();
    super.dispose();
  }

  Future<void> _pickFilesToShare() async {
    final result = await FilePicker.pickFiles(
      withData: true,
      allowMultiple: true,
      type: FileType.any,
    );
    if (result == null) return;
    setState(() => _pickedFiles = result.files);
  }

  void _removePickedFile(PlatformFile file) {
    setState(() => _pickedFiles = _pickedFiles.where((f) => f != file).toList());
  }

  Future<void> _createCollection() async {
    final name = _nameController.text.trim();
    if (name.isEmpty || _pickedFiles.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final normalized = await Future.wait(
        _pickedFiles
            .where((f) => f.bytes != null)
            .map((f) => normalizeForSharing(name: f.name, bytes: f.bytes!)),
      );
      await TorrentCollections.instance.createCollection(name, normalized);
      if (mounted) {
        FocusScope.of(context).unfocus();
        Navigator.of(context).pop();
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t create collection: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
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
      if (mounted) {
        FocusScope.of(context).unfocus();
        Navigator.of(context).pop();
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t add magnet link: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _addFromTorrentFile() async {
    final result = await FilePicker.pickFiles(withData: true, type: FileType.any);
    final bytes = result?.files.single.bytes;
    if (bytes == null) return;

    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await TorrentCollections.instance.addFromFileBytes(bytes);
      if (mounted) {
        FocusScope.of(context).unfocus();
        Navigator.of(context).pop();
      }
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
        child: SingleChildScrollView(
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
                      'Add',
                      style: TextStyle(fontSize: 16, fontWeight: FontWeight.w500),
                    ),
                    const SizedBox(width: 34),
                  ],
                ),
              ),

              // ── Share your own files ──────────────────────────────
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 10, 20, 8),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: SectionLabel('SHARE YOUR FILES'),
                ),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: TextField(
                  key: const Key('collectionNameField'),
                  controller: _nameController,
                  style: const TextStyle(color: AppColors.text, fontSize: 13),
                  decoration: InputDecoration(
                    hintText: 'Collection name',
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
                  onChanged: (_) => setState(() {}),
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 10, 20, 0),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: PillButton(
                    label: '🖼️ Pick photos, videos, audio, or files',
                    dim: true,
                    onTap: _busy ? null : _pickFilesToShare,
                  ),
                ),
              ),
              if (_pickedFiles.isNotEmpty)
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 10, 20, 0),
                  child: Wrap(
                    spacing: 6,
                    runSpacing: 6,
                    children: [
                      for (final f in _pickedFiles)
                        _PickedFileChip(
                          file: f,
                          onRemove: () => _removePickedFile(f),
                        ),
                    ],
                  ),
                ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 14, 20, 0),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: PillButton(
                    label:
                        'Create & share${_pickedFiles.isEmpty ? '' : ' · ${_pickedFiles.length} file${_pickedFiles.length == 1 ? '' : 's'}'}',
                    onTap: _busy ||
                            _pickedFiles.isEmpty ||
                            _nameController.text.trim().isEmpty
                        ? null
                        : _createCollection,
                  ),
                ),
              ),

              Padding(
                padding: const EdgeInsets.symmetric(vertical: 22, horizontal: 20),
                child: DecoratedBox(
                  decoration: const BoxDecoration(
                    border: Border(bottom: BorderSide(color: AppColors.border)),
                  ),
                ),
              ),

              // ── Join an existing swarm ────────────────────────────
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 0, 20, 8),
                child: Align(
                  alignment: Alignment.centerLeft,
                  child: SectionLabel('JOIN A SWARM'),
                ),
              ),
              Padding(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Row(
                  children: [
                    Expanded(
                      child: TextField(
                        key: const Key('magnetField'),
                        controller: _magnetController,
                        style: const TextStyle(color: AppColors.text, fontSize: 13),
                        decoration: InputDecoration(
                          hintText: 'magnet:?xt=urn:btih:...',
                          hintStyle: const TextStyle(color: AppColors.neutral500),
                          filled: true,
                          fillColor: AppColors.surface,
                          contentPadding: const EdgeInsets.symmetric(
                              horizontal: 12, vertical: 11),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(8),
                            borderSide:
                                const BorderSide(color: AppColors.borderStrong),
                          ),
                          enabledBorder: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(8),
                            borderSide:
                                const BorderSide(color: AppColors.borderStrong),
                          ),
                        ),
                        onSubmitted: (_) => _addFromMagnet(),
                      ),
                    ),
                    const SizedBox(width: 10),
                    PillButton(
                      key: const Key('addMagnetButton'),
                      label: 'Add',
                      onTap: _busy ? null : _addFromMagnet,
                    ),
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
                    onTap: _busy ? null : _addFromTorrentFile,
                  ),
                ),
              ),

              if (_error != null)
                Padding(
                  padding: const EdgeInsets.fromLTRB(20, 18, 20, 0),
                  child: Container(
                    padding: const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
                    decoration: BoxDecoration(
                      color: const Color(0xFFEB5757).withValues(alpha: 0.1),
                      border: Border.all(
                          color: const Color(0xFFEB5757).withValues(alpha: 0.4)),
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
              const SizedBox(height: 24),
            ],
          ),
        ),
      ),
    );
  }
}

class _PickedFileChip extends StatelessWidget {
  const _PickedFileChip({required this.file, required this.onRemove});

  final PlatformFile file;
  final VoidCallback onRemove;

  @override
  Widget build(BuildContext context) {
    return Container(
      padding: const EdgeInsets.fromLTRB(10, 6, 6, 6),
      decoration: BoxDecoration(
        color: AppColors.surface,
        border: Border.all(color: AppColors.borderStrong),
        borderRadius: BorderRadius.circular(99),
      ),
      child: Row(
        mainAxisSize: MainAxisSize.min,
        children: [
          ConstrainedBox(
            constraints: const BoxConstraints(maxWidth: 140),
            child: Text(
              file.name,
              overflow: TextOverflow.ellipsis,
              style: const TextStyle(fontSize: 11, color: AppColors.neutral300),
            ),
          ),
          const SizedBox(width: 4),
          InkWell(
            onTap: onRemove,
            child: const Icon(Icons.close, size: 14, color: AppColors.neutral400),
          ),
        ],
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
