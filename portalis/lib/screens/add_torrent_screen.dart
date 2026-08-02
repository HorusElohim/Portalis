import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/collections.dart';
import '../theme.dart';
import '../ui/ui.dart';

/// "Torrent" — the join-a-swarm half of the old combined Add screen,
/// redesigned per the Portalis Add Flow: magnet input with a live preview
/// card parsed from the link itself (name from `dn=`, hash from `btih:`),
/// plus a .torrent file picker. Anything actually unknown before the
/// engine fetches metadata is shown as "—" rather than invented.
class AddTorrentScreen extends StatefulWidget {
  const AddTorrentScreen({super.key});

  @override
  State<AddTorrentScreen> createState() => _AddTorrentScreenState();
}

class _AddTorrentScreenState extends State<AddTorrentScreen> {
  final _magnetController = TextEditingController();
  bool _touched = false;
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _magnetController.dispose();
    super.dispose();
  }

  String get _magnet => _magnetController.text.trim();

  bool get _isValid => looksLikeMagnet(_magnet);

  String get _previewName {
    final dn = RegExp(r'[?&]dn=([^&]+)', caseSensitive: false)
        .firstMatch(_magnet)
        ?.group(1);
    if (dn == null) return 'Unnamed torrent';
    return Uri.decodeComponent(dn.replaceAll('+', ' '));
  }

  String get _previewHash {
    final hash = RegExp(r'btih:([a-zA-Z0-9]+)', caseSensitive: false)
        .firstMatch(_magnet)
        ?.group(1);
    if (hash != null) return 'btih:${hash.toLowerCase()}';
    if (RegExp(r'^[0-9a-fA-F]{40}$').hasMatch(_magnet)) {
      return 'btih:${_magnet.toLowerCase()}';
    }
    return 'btih: —';
  }

  Future<void> _paste() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim();
    if (text == null || text.isEmpty) return;
    setState(() {
      _magnetController.text = text;
      _touched = true;
    });
  }

  Future<void> _addMagnet() async {
    if (!_isValid) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await Collections.instance.addFromMagnet(_magnet);
      if (mounted) {
        FocusScope.of(context).unfocus();
        Navigator.of(context).pop();
        showToast(context, 'Added $_previewName — joining swarm',
            severity: ToastSeverity.success);
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t add magnet link: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _pickTorrentFile() async {
    final result = await FilePicker.pickFiles(withData: true, type: FileType.any);
    final bytes = result?.files.single.bytes;
    if (bytes == null) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await Collections.instance.addFromFileBytes(bytes);
      if (mounted) {
        Navigator.of(context).pop();
        showToast(context, 'Torrent added — joining swarm',
            severity: ToastSeverity.success);
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t add .torrent file: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final showInvalid = _touched && _magnet.isNotEmpty && !_isValid;

    return Scaffold(
      backgroundColor: AppColors.surfaceDeep,
      body: SafeArea(
        child: PageBody(
          child: Column(
            children: [
              Align(
                alignment: Alignment.centerLeft,
                child: TextButton.icon(
                  onPressed: () => Navigator.of(context).pop(),
                  icon: const Icon(Icons.chevron_left,
                      size: 18, color: AppColors.textDim),
                  label: const Text('Back',
                      style: TextStyle(fontSize: 14, color: AppColors.textDim)),
                ),
              ),
              Padding(
                padding: const EdgeInsets.fromLTRB(20, 2, 20, 14),
                child: Row(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: [
                    // The torrent mark, decoded at its display size rather
                    // than the source's 1254² — see the same treatment on
                    // first run.
                    ClipRRect(
                      borderRadius: BorderRadius.circular(14),
                      child: Image.asset(
                        'assets/PortalisTorrentNature.png',
                        width: 46,
                        height: 46,
                        cacheWidth: 138,
                        cacheHeight: 138,
                        filterQuality: FilterQuality.medium,
                      ),
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                  crossAxisAlignment: CrossAxisAlignment.start,
                  children: const [
                    Text(
                      'Torrent',
                      style: TextStyle(
                        fontSize: 25,
                        fontWeight: FontWeight.w500,
                        letterSpacing: -0.4,
                      ),
                    ),
                    SizedBox(height: 4),
                    Text(
                      'Paste a magnet link or open a .torrent file.',
                      style: TextStyle(fontSize: 12.5, color: AppColors.textDim),
                    ),
                  ],
                ),
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
                      TextField(
                        key: const Key('magnetField'),
                        controller: _magnetController,
                        style: const TextStyle(
                          fontSize: 13,
                          fontFamily: 'monospace',
                          color: AppColors.text,
                        ),
                        decoration: InputDecoration(
                          hintText: 'magnet:?xt=urn:btih:…',
                          hintStyle: const TextStyle(color: AppColors.textGhost),
                          filled: true,
                          fillColor: AppColors.surface,
                          contentPadding: const EdgeInsets.symmetric(
                              horizontal: 14, vertical: 16),
                          border: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(10),
                            borderSide:
                                const BorderSide(color: AppColors.borderStrong),
                          ),
                          enabledBorder: OutlineInputBorder(
                            borderRadius: BorderRadius.circular(10),
                            borderSide:
                                const BorderSide(color: AppColors.borderStrong),
                          ),
                        ),
                        onChanged: (_) => setState(() => _touched = true),
                        onSubmitted: (_) => _addMagnet(),
                      ),
                      const SizedBox(height: 9),
                      Row(
                        children: [
                          Expanded(
                            child: _SecondaryButton(
                              label: 'Paste',
                              icon: Icons.content_paste,
                              onTap: _busy ? null : _paste,
                            ),
                          ),
                          const SizedBox(width: 9),
                          Expanded(
                            child: _SecondaryButton(
                              label: '.torrent file',
                              icon: Icons.description_outlined,
                              onTap: _busy ? null : _pickTorrentFile,
                            ),
                          ),
                        ],
                      ),
                      if (showInvalid)
                        const Padding(
                          padding: EdgeInsets.only(top: 10),
                          child: Text(
                            'That isn\'t a magnet link — it should start with magnet:? '
                            '(a bare 40-character info-hash works too)',
                            style: TextStyle(
                                fontSize: 12.5,
                                height: 1.45,
                                color: AppColors.signalSoft),
                          ),
                        ),
                      const SizedBox(height: 14),
                      if (_isValid)
                        Container(
                          padding: const EdgeInsets.all(16),
                          decoration: BoxDecoration(
                            color: AppColors.surface,
                            borderRadius: BorderRadius.circular(14),
                            border: Border.all(color: AppColors.border),
                          ),
                          child: Column(
                            crossAxisAlignment: CrossAxisAlignment.start,
                            children: [
                              Row(
                                crossAxisAlignment: CrossAxisAlignment.start,
                                children: [
                                  Container(
                                    width: 40,
                                    height: 40,
                                    alignment: Alignment.center,
                                    decoration: BoxDecoration(
                                      color: AppColors.signalDeep,
                                      borderRadius: BorderRadius.circular(10),
                                      border:
                                          Border.all(color: AppColors.signalDim),
                                    ),
                                    child: const Icon(Icons.download_outlined,
                                        size: 19, color: AppColors.signal),
                                  ),
                                  const SizedBox(width: 12),
                                  Expanded(
                                    child: Column(
                                      crossAxisAlignment:
                                          CrossAxisAlignment.start,
                                      children: [
                                        const Text(
                                          'READY TO ADD',
                                          style: TextStyle(
                                            fontSize: 10,
                                            letterSpacing: 1.1,
                                            color: AppColors.signal,
                                          ),
                                        ),
                                        const SizedBox(height: 2),
                                        Text(
                                          _previewName,
                                          style: const TextStyle(
                                              fontSize: 16,
                                              fontWeight: FontWeight.w500,
                                              height: 1.25),
                                        ),
                                        const SizedBox(height: 3),
                                        Text(
                                          _previewHash,
                                          maxLines: 1,
                                          overflow: TextOverflow.ellipsis,
                                          style: const TextStyle(
                                            fontSize: 11,
                                            fontFamily: 'monospace',
                                            color: AppColors.textGhost,
                                          ),
                                        ),
                                      ],
                                    ),
                                  ),
                                ],
                              ),
                              const SizedBox(height: 12),
                              Container(
                                padding:
                                    const EdgeInsets.symmetric(vertical: 12),
                                decoration: const BoxDecoration(
                                  border: Border(
                                    top: BorderSide(color: AppColors.border),
                                  ),
                                ),
                                child: const Text(
                                  'Size, files, and peers resolve once the swarm '
                                  'is joined — watch them live on Home.',
                                  style: TextStyle(
                                      fontSize: 11.5,
                                      height: 1.45,
                                      color: AppColors.textDim),
                                ),
                              ),
                            ],
                          ),
                        )
                      else
                        Container(
                          height: 172,
                          decoration: BoxDecoration(
                            border: Border.all(color: AppColors.borderStrong),
                            borderRadius: BorderRadius.circular(14),
                            color: AppColors.surface.withValues(alpha: 0.4),
                          ),
                          child: Column(
                            mainAxisAlignment: MainAxisAlignment.center,
                            children: const [
                              Icon(Icons.download_outlined,
                                  size: 26, color: AppColors.textGhost),
                              SizedBox(height: 8),
                              Padding(
                                padding: EdgeInsets.symmetric(horizontal: 60),
                                child: Text(
                                  'Paste a link above and the torrent details appear here',
                                  textAlign: TextAlign.center,
                                  style: TextStyle(
                                      fontSize: 13,
                                      height: 1.45,
                                      color: AppColors.textDim),
                                ),
                              ),
                            ],
                          ),
                        ),
                      if (_error != null)
                        Padding(
                          padding: const EdgeInsets.only(top: 14),
                          child: Container(
                            padding: const EdgeInsets.symmetric(
                                horizontal: 12, vertical: 9),
                            decoration: BoxDecoration(
                              color:
                                  const Color(0xFFEB5757).withValues(alpha: 0.1),
                              border: Border.all(
                                  color: const Color(0xFFEB5757)
                                      .withValues(alpha: 0.4)),
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
                        key: const Key('addMagnetButton'),
                        onPressed: _busy || !_isValid ? null : _addMagnet,
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
                                  valueColor:
                                      AlwaysStoppedAnimation(AppColors.surfaceDeep),
                                ),
                              )
                            : const Text('Add & start',
                                style: TextStyle(fontSize: 16)),
                      ),
                    ),
                    const SizedBox(height: 8),
                    const Text(
                      'Starts fetching immediately',
                      style:
                          TextStyle(fontSize: 11.5, color: AppColors.textGhost),
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
}

class _SecondaryButton extends StatelessWidget {
  const _SecondaryButton({required this.label, required this.icon, this.onTap});

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
          height: 46,
          decoration: BoxDecoration(
            borderRadius: BorderRadius.circular(10),
            border: Border.all(color: AppColors.borderStrong),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 8),
          child: Row(
            mainAxisAlignment: MainAxisAlignment.center,
            children: [
              Icon(icon, size: 16, color: AppColors.text),
              const SizedBox(width: 8),
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: const TextStyle(fontSize: 14, color: AppColors.text),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}
