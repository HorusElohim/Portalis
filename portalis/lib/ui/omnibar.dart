// Part of the Portalis UI kit — see ui.dart.

import 'package:file_picker/file_picker.dart';
import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../services/collections.dart';
import '../theme.dart';
import 'toast.dart';

/// One field that takes whatever you paste.
///
/// Replaces the sidebar's magnet field, its Paste and Add buttons, its
/// .torrent picker and its "Join with a key" action — five controls, three of
/// which were the same control in different clothes. What they had in common
/// is that each accepted one line of text and did something with it; what
/// differed is that you had to know in advance which one your line belonged
/// to. This asks instead: paste, and [PasteKind] decides.
///
/// Search is the fallback rather than the purpose, which is why the same
/// field can do both without a mode switch — a magnet link and an invite code
/// are unmistakable, so anything that is neither was meant as a filter.
class Omnibar extends StatefulWidget {
  const Omnibar({
    super.key,
    required this.onSearch,
    required this.onInvite,
    this.autofocus = false,
  });

  /// Fires as the text changes, with '' when it is cleared. The caller owns
  /// the filtering; this widget never holds a filtered list.
  final ValueChanged<String> onSearch;

  /// A recognised invite code, handed on to the join flow rather than joined
  /// from here — joining names you to strangers, so it keeps its confirmation
  /// step.
  final ValueChanged<String> onInvite;

  final bool autofocus;

  @override
  State<Omnibar> createState() => _OmnibarState();
}

class _OmnibarState extends State<Omnibar> {
  final _controller = TextEditingController();
  final _focus = FocusNode();
  bool _busy = false;
  String? _error;

  @override
  void dispose() {
    _controller.dispose();
    _focus.dispose();
    super.dispose();
  }

  PasteKind get _kind => PasteKind.of(_controller.text);

  void _onChanged(String value) {
    setState(() => _error = null);
    // Only a search term filters. A half-typed magnet link is not a search
    // for the letters "magne", and emptying the field has to restore the
    // full list whatever the text used to be.
    final kind = PasteKind.of(value);
    widget.onSearch(kind == PasteKind.search ? value.trim() : '');
  }

  Future<void> _submit() async {
    final text = _controller.text.trim();
    switch (PasteKind.of(text)) {
      case PasteKind.empty:
      case PasteKind.search:
        return;
      case PasteKind.invite:
        _clear();
        widget.onInvite(text);
      case PasteKind.magnet:
        setState(() {
          _busy = true;
          _error = null;
        });
        try {
          await Collections.instance.addFromMagnet(text);
          if (!mounted) return;
          _clear();
          showToast(context, 'Added — joining swarm',
              severity: ToastSeverity.success);
        } catch (e) {
          // Next to the field rather than as a toast: the text that failed is
          // still on screen, so the message belongs beside it.
          if (mounted) setState(() => _error = '$e');
        } finally {
          if (mounted) setState(() => _busy = false);
        }
    }
  }

  void _clear() {
    _controller.clear();
    widget.onSearch('');
  }

  /// The one thing that cannot be pasted. A `.torrent` is a file, so it keeps
  /// an affordance of its own rather than being dropped along with the
  /// sidebar's text controls — it was the only capability there that the bar
  /// does not otherwise absorb.
  Future<void> _pickTorrentFile() async {
    final result =
        await FilePicker.pickFiles(withData: true, type: FileType.any);
    final bytes = result?.files.single.bytes;
    if (bytes == null) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      await Collections.instance.addFromFileBytes(bytes);
      if (!mounted) return;
      showToast(context, 'Torrent added — joining swarm',
          severity: ToastSeverity.success);
    } catch (e) {
      if (mounted) setState(() => _error = '$e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  Future<void> _pasteFromClipboard() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim();
    if (text == null || text.isEmpty) return;
    _controller.text = text;
    _onChanged(text);
    if (mounted) _focus.requestFocus();
  }

  @override
  Widget build(BuildContext context) {
    final kind = _kind;
    // Mint only once the text is genuinely actionable — the same rule the
    // rest of the palette follows. A search term is not a pending action.
    final armed = kind == PasteKind.magnet || kind == PasteKind.invite;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Container(
          decoration: BoxDecoration(
            color: AppColors.surface,
            borderRadius: BorderRadius.circular(AppRadius.control),
            border: Border.all(
              color: armed
                  ? AppColors.signal.withValues(alpha: 0.4)
                  : AppColors.border,
            ),
          ),
          padding: const EdgeInsets.symmetric(horizontal: 14),
          child: Row(
            children: [
              Icon(
                armed ? Icons.bolt_outlined : Icons.search,
                size: 16,
                color: armed ? AppColors.signal : AppColors.textFaint,
              ),
              const SizedBox(width: 11),
              Expanded(
                child: TextField(
                  key: const Key('omnibarField'),
                  controller: _controller,
                  focusNode: _focus,
                  autofocus: widget.autofocus,
                  enabled: !_busy,
                  style: monoLabel(
                      size: 12.5, color: AppColors.text, letterSpacing: 0),
                  decoration: InputDecoration(
                    isDense: true,
                    border: InputBorder.none,
                    contentPadding: const EdgeInsets.symmetric(vertical: 14),
                    hintText: 'Paste an invite key or magnet link, or search…',
                    hintStyle: monoLabel(size: 12.5, letterSpacing: 0),
                  ),
                  onChanged: _onChanged,
                  onSubmitted: (_) => _busy ? null : _submit(),
                ),
              ),
              // What the bar has decided the text is, so the dispatch is never
              // a surprise. Nothing to read when the field is empty.
              if (kind != PasteKind.empty) ...[
                const SizedBox(width: 10),
                _Hint(kind: kind, busy: _busy, onSubmit: _submit),
              ] else
                _IconAction(
                  icon: Icons.content_paste_outlined,
                  tooltip: 'Paste',
                  onTap: _pasteFromClipboard,
                ),
              // Always reachable, whatever is in the field: it is the one
              // thing a paste cannot express.
              const SizedBox(width: 4),
              _IconAction(
                key: const Key('omnibarTorrentFile'),
                icon: Icons.attach_file,
                tooltip: 'Add a .torrent file',
                onTap: _busy ? null : _pickTorrentFile,
              ),
            ],
          ),
        ),
        if (_error != null)
          Padding(
            padding: const EdgeInsets.only(top: 7, left: 2),
            child: Text(
              _error!,
              style: monoLabel(
                  size: 9.5, color: AppColors.danger, letterSpacing: 0.2),
            ),
          ),
      ],
    );
  }
}

/// The right-hand end of the bar: what will happen if you press enter.
class _Hint extends StatelessWidget {
  const _Hint({required this.kind, required this.busy, required this.onSubmit});

  final PasteKind kind;
  final bool busy;
  final VoidCallback onSubmit;

  @override
  Widget build(BuildContext context) {
    if (busy) {
      return const SizedBox(
        width: 14,
        height: 14,
        child: CircularProgressIndicator(
            strokeWidth: 1.6, color: AppColors.signal),
      );
    }
    final label = switch (kind) {
      PasteKind.magnet => 'ADD TORRENT',
      PasteKind.invite => 'JOIN',
      // Filtering happens as you type; there is nothing to press.
      PasteKind.search => 'FILTERING',
      PasteKind.empty => '',
    };
    final actionable = kind == PasteKind.magnet || kind == PasteKind.invite;
    if (!actionable) {
      return Text(label, style: monoLabel(size: 9.5, letterSpacing: 1));
    }
    return Material(
      color: AppColors.signal,
      borderRadius: BorderRadius.circular(AppRadius.tight),
      child: InkWell(
        key: const Key('omnibarSubmit'),
        borderRadius: BorderRadius.circular(AppRadius.tight),
        onTap: onSubmit,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 5),
          child: Text(
            label,
            style: monoLabel(
              size: 9.5,
              color: AppColors.onSignal,
              letterSpacing: 1,
              weight: FontWeight.w500,
            ),
          ),
        ),
      ),
    );
  }
}

class _IconAction extends StatelessWidget {
  const _IconAction({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.onTap,
  });

  final IconData icon;
  final String tooltip;

  /// Null while the bar is busy — dimmed rather than gone, so the row does
  /// not reflow mid-action.
  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return Tooltip(
      message: tooltip,
      child: Opacity(
        opacity: onTap == null ? 0.38 : 1,
        child: InkWell(
          borderRadius: BorderRadius.circular(AppRadius.tight),
          onTap: onTap,
          child: Padding(
            padding: const EdgeInsets.all(4),
            child: Icon(icon, size: 15, color: AppColors.textFaint),
          ),
        ),
      ),
    );
  }
}
