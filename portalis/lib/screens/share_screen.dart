import 'dart:io';
import 'dart:typed_data';

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:image_picker/image_picker.dart';

import '../media_convert.dart';
import '../media_kind.dart';
import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';

typedef _PickedFile = ({String name, Uint8List bytes});


String _kindLabel(String name) {
  switch (kindOf(name)) {
    case MediaKind.image:
      return 'Photo';
    case MediaKind.video:
      return 'Video';
    case MediaKind.audio:
      return 'Audio';
    case MediaKind.subtitle:
      return 'Subtitles';
    case MediaKind.other:
      return 'File';
  }
}

/// "Share something" — the share half of the old combined Add screen,
/// redesigned per the Portalis Add Flow: inline-editable collection name,
/// Photos/Files/Folder pickers, a file list with per-item remove, and one
/// Create & share action that seeds the new collection from this device.
class ShareScreen extends StatefulWidget {
  const ShareScreen({super.key});

  @override
  State<ShareScreen> createState() => _ShareScreenState();
}

class _ShareScreenState extends State<ShareScreen> {
  final _nameController = TextEditingController();
  List<_PickedFile> _files = [];
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _nameController.dispose();
    super.dispose();
  }

  void _add(Iterable<_PickedFile> picked) {
    setState(() {
      final existing = _files.map((f) => f.name).toSet();
      _files = [
        ..._files,
        ...picked.where((f) => !existing.contains(f.name)),
      ];
    });
  }

  Future<void> _pickPhotos() async {
    final xfiles = await ImagePicker().pickMultipleMedia();
    if (xfiles.isEmpty) return;
    _add(await Future.wait(
      xfiles.map((f) async => (name: f.name, bytes: await f.readAsBytes())),
    ));
  }

  Future<void> _pickFiles() async {
    final result = await FilePicker.pickFiles(
      withData: true,
      allowMultiple: true,
      type: FileType.any,
    );
    if (result == null) return;
    _add(result.files
        .where((f) => f.bytes != null)
        .map((f) => (name: f.name, bytes: f.bytes!)));
  }

  Future<void> _pickFolder() async {
    try {
      final dir = await FilePicker.getDirectoryPath();
      if (dir == null) return;
      final entries = await Directory(dir)
          .list(recursive: false)
          .where((e) => e is File)
          .cast<File>()
          .toList();
      if (entries.isEmpty) {
        _toast('That folder has no files at its top level');
        return;
      }
      final picked = <_PickedFile>[];
      for (final file in entries) {
        final basename = file.path.split(Platform.pathSeparator).last;
        picked.add((name: basename, bytes: await file.readAsBytes()));
      }
      _add(picked);
    } catch (e) {
      // Folder access is platform-dependent (sandboxing) — degrade to a
      // message rather than a crash; Photos/Files still work.
      _toast('Couldn\'t read that folder: $e');
    }
  }

  void _toast(String msg) {
    if (!mounted) return;
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  Future<void> _createShare() async {
    final name = _nameController.text.trim();
    if (name.isEmpty || _files.isEmpty) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      final normalized = await Future.wait(
        _files.map((f) => normalizeForSharing(name: f.name, bytes: f.bytes)),
      );
      // Creates a *shared* collection, not a bare torrent: what you share is
      // now invitable and can grow later.
      await Collections.instance.createWithMedia(name, normalized);
      if (mounted) {
        FocusScope.of(context).unfocus();
        Navigator.of(context).pop();
        _toastGlobal('"$name" is live — seeding from this device');
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t create collection: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  // After pop, this screen's ScaffoldMessenger is gone — use the root one.
  void _toastGlobal(String msg) {
    ScaffoldMessenger.of(context).showSnackBar(SnackBar(content: Text(msg)));
  }

  int get _totalBytes => _files.fold(0, (sum, f) => sum + f.bytes.length);

  @override
  Widget build(BuildContext context) {
    final name = _nameController.text.trim();
    final canCreate = !_busy && name.isNotEmpty && _files.isNotEmpty;
    final summary = _files.isEmpty
        ? 'Add at least one item'
        : name.isEmpty
            ? 'Name the collection to continue'
            : 'Seeds from this device as soon as it\'s created';

    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(
        child: PageBody(
          child: Column(
            children: [
              Align(
                alignment: Alignment.centerLeft,
                child: _BackButton(onTap: () => Navigator.of(context).pop()),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 2, 20, 12),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    Row(
                      children: [
                        Expanded(
                          child: TextField(
                            key: const Key('collectionNameField'),
                            controller: _nameController,
                            style: const TextStyle(
                              fontSize: 25,
                              fontWeight: FontWeight.w500,
                              letterSpacing: -0.4,
                              color: AppColors.text,
                            ),
                            cursorColor: AppColors.signal,
                            decoration: const InputDecoration(
                              hintText: 'Untitled collection',
                              hintStyle: TextStyle(color: AppColors.textGhost),
                              border: InputBorder.none,
                              isDense: true,
                              contentPadding: EdgeInsets.zero,
                            ),
                            onChanged: (_) => setState(() {}),
                          ),
                        ),
                        const Icon(Icons.edit_outlined,
                            size: 16, color: AppColors.signal),
                      ],
                    ),
                    const SizedBox(height: 4),
                    const Text(
                      'Tap the name to rename · files stay on this phone',
                      style: TextStyle(fontSize: 11.5, color: AppColors.textGhost),
                    ),
                  ],
                ),
              ),
              Expanded(
                child: SingleChildScrollView(
                  padding: const EdgeInsets.fromLTRB(20, 0, 20, 12),
                  child: Column(
                    crossAxisAlignment: CrossAxisAlignment.stretch,
                    children: [
                      Row(
                        children: [
                          Expanded(
                            child: _PickerButton(
                              label: 'Photos',
                              icon: Icons.photo_camera_outlined,
                              onTap: _busy ? null : _pickPhotos,
                            ),
                          ),
                          const SizedBox(width: 9),
                          Expanded(
                            child: _PickerButton(
                              label: 'Files',
                              icon: Icons.description_outlined,
                              onTap: _busy ? null : _pickFiles,
                            ),
                          ),
                          const SizedBox(width: 9),
                          Expanded(
                            child: _PickerButton(
                              label: 'Folder',
                              icon: Icons.folder_outlined,
                              onTap: _busy ? null : _pickFolder,
                            ),
                          ),
                        ],
                      ),
                      const SizedBox(height: 14),
                      if (_files.isEmpty)
                        Container(
                          height: 150,
                          decoration: BoxDecoration(
                            border: Border.all(color: AppColors.borderStrong),
                            borderRadius: BorderRadius.circular(14),
                            color: AppColors.surface.withValues(alpha: 0.4),
                          ),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: const [
                              Icon(Icons.upload_outlined,
                                  size: 26, color: AppColors.textGhost),
                              SizedBox(height: 8),
                              Text(
                                'Nothing added yet',
                                style: TextStyle(
                                    fontSize: 13, color: AppColors.textDim),
                              ),
                            ],
                          ),
                        )
                      else ...[
                        Row(
                          children: [
                            Text(
                              '${_files.length} ITEM${_files.length == 1 ? '' : 'S'} · ${formatBytes(_totalBytes)}',
                              style: const TextStyle(
                                fontSize: 10.5,
                                fontFamily: 'monospace',
                                letterSpacing: 1.0,
                                color: AppColors.textDim,
                              ),
                            ),
                            const Spacer(),
                            TextButton(
                              onPressed: () => setState(() => _files = []),
                              child: const Text(
                                'Remove all',
                                style: TextStyle(
                                    fontSize: 12.5, color: AppColors.signalSoft),
                              ),
                            ),
                          ],
                        ),
                        for (final f in _files) _fileRow(f),
                      ],
                      if (_error != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 14),
                          child: Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 9),
                            decoration: BoxDecoration(
                              color: const Color(0xFFEB5757).withValues(alpha: 0.1),
                              border: Border.all(
                                  color:
                                      const Color(0xFFEB5757).withValues(alpha: 0.4)),
                              borderRadius: BorderRadius.circular(8),
                            ),
                            child: Text(
                              _error!,
                              style: const TextStyle(
                                  fontSize: 11, color: Color(0xFFEB5757)),
                            ),
                          ),
                        ),
                    ],
                  ),
                ),
              ),
              Container(
                padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
                decoration: const BoxDecoration(
                  border: Border(top: BorderSide(color: AppColors.border)),
                ),
                child: Column(
                  children: [
                    SizedBox(
                      width: double.infinity,
                      height: 52,
                      child: FilledButton(
                        key: const Key('createShareButton'),
                        onPressed: canCreate ? _createShare : null,
                        style: FilledButton.styleFrom(
                          backgroundColor: AppColors.signal,
                          disabledBackgroundColor: AppColors.borderStrong,
                          foregroundColor: AppColors.surfaceDeep,
                          shape: RoundedRectangleBorder(
                            borderRadius: BorderRadius.circular(14),
                          ),
                        ),
                        child: _busy
                            ? const SizedBox(
                                width: 20,
                                height: 20,
                                child: CircularProgressIndicator(
                                  strokeWidth: 2,
                                  valueColor: AlwaysStoppedAnimation(AppColors.surfaceDeep),
                                ),
                              )
                            : const Text('Create & share',
                                style: TextStyle(fontSize: 16)),
                      ),
                    ),
                    const SizedBox(height: 8),
                    Text(
                      summary,
                      style: const TextStyle(
                          fontSize: 11.5, color: AppColors.textGhost),
                    ),
                  ],
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }

  Widget _fileRow(_PickedFile f) {
    final dot = f.name.lastIndexOf('.');
    final ext = dot == -1 || dot == f.name.length - 1
        ? 'FILE'
        : f.name.substring(dot + 1).toUpperCase();
    return Container(
      padding: const EdgeInsets.symmetric(vertical: 8),
      decoration: const BoxDecoration(
        border: Border(bottom: BorderSide(color: AppColors.border)),
      ),
      child: Row(
        children: [
          Container(
            width: 34,
            height: 34,
            alignment: Alignment.center,
            decoration: BoxDecoration(
              color: AppColors.surface,
              border: Border.all(color: AppColors.border),
              borderRadius: BorderRadius.circular(7),
            ),
            child: Text(
              ext.length > 4 ? ext.substring(0, 4) : ext,
              style: const TextStyle(
                fontSize: 9,
                fontWeight: FontWeight.w600,
                letterSpacing: 0.6,
                color: AppColors.textDim,
              ),
            ),
          ),
          const SizedBox(width: 12),
          Expanded(
            child: Column(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Text(
                  f.name,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 13.5),
                ),
                const SizedBox(height: 1),
                Text(
                  '${_kindLabel(f.name)} · ${formatBytes(f.bytes.length)}',
                  style: const TextStyle(
                      fontSize: 11.5, color: AppColors.textDim),
                ),
              ],
            ),
          ),
          IconButton(
            onPressed: () => setState(() =>
                _files = _files.where((other) => other != f).toList()),
            icon: const Icon(Icons.close, size: 15, color: AppColors.textDim),
          ),
        ],
      ),
    );
  }
}

class _PickerButton extends StatelessWidget {
  const _PickerButton({required this.label, required this.icon, this.onTap});

  final String label;
  final IconData icon;
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Material(
      color: AppColors.surface,
      borderRadius: BorderRadius.circular(10),
      child: InkWell(
        borderRadius: BorderRadius.circular(10),
        onTap: onTap,
        child: Container(
          height: 74,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(10),
            border: Border.all(color: AppColors.borderStrong),
          ),
          child: Column(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(icon, size: 20, color: AppColors.text),
              const SizedBox(height: 7),
              Text(label,
                  style: const TextStyle(fontSize: 12.5, color: AppColors.text)),
            ],
          ),
        ),
      ),
    );
  }
}

class _BackButton extends StatelessWidget {
  const _BackButton({required this.onTap});

  final VoidCallback onTap;

  @override
  Widget build(BuildContext context) {
    return TextButton.icon(
      onPressed: onTap,
      icon: const Icon(Icons.chevron_left, size: 18, color: AppColors.textDim),
      label: const Text(
        'Back',
        style: TextStyle(fontSize: 14, color: AppColors.textDim),
      ),
    );
  }
}
