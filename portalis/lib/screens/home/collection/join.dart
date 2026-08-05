import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../../../bridge_generated/device.dart' as device_bridge;
import '../../../services/collections.dart';
import '../../../theme.dart';
import '../../../ui/ui.dart';

/// "Join a collection" — the invite-code half of the old combined Add
/// screen, redesigned per the Portalis Add Flow. The preview card is
/// parsed from the code itself (hex-wrapped
/// `<secret>:<name>[@addresses]`), so what's shown before joining is real:
/// the collection's name and whether the invite carries reachable
/// addresses. The display name comes from this device's identity nickname
/// instead of a second text field.
class JoinCollectionScreen extends StatefulWidget {
  const JoinCollectionScreen({super.key, this.onClose, this.initialCode});

  /// Called instead of popping a route — set when this is embedded in the
  /// desktop shell's centre pane rather than pushed over it.
  final VoidCallback? onClose;

  /// A code the omnibar already recognised. Arriving here still shows the
  /// preview and still asks — joining announces you to strangers, so it is
  /// never something a paste completes on its own.
  final String? initialCode;

  @override
  State<JoinCollectionScreen> createState() => _JoinCollectionScreenState();
}

class _JoinCollectionScreenState extends State<JoinCollectionScreen> {
  final _codeController = TextEditingController();
  String _nickname = 'Me';
  bool _busy = false;
  String? _error;

  @override
  void initState() {
    super.initState();
    _codeController.text = widget.initialCode ?? '';
    _loadNickname();
  }

  Future<void> _loadNickname() async {
    try {
      final identity = await device_bridge.deviceIdentity();
      if (mounted) setState(() => _nickname = identity.nickname);
    } catch (_) {
      // Backend unavailable (e.g. tests) — keep the fallback.
    }
  }

  @override
  void dispose() {
    _codeController.dispose();
    super.dispose();
  }

  /// Pops the pushed route, or — embedded in the desktop shell — hands
  /// control back to whatever put this on screen.
  void _close() =>
      widget.onClose != null ? widget.onClose!() : Navigator.of(context).pop();

  /// `(name, addressCount)` when the code un-hexes to
  /// `<64-hex secret>:<name>[@addr1,addr2]`, else null.
  (String, int)? get _parsed {
    final decoded = decodeInviteCode(_codeController.text);
    if (decoded == null) return null;
    final split = decoded.indexOf(':');
    if (split == -1) return null;
    final secret = decoded.substring(0, split);
    // Same pattern the omnibar's PasteKind checks — see
    // [inviteSecretPattern] — so a code arriving here already recognised as
    // an invite is never one this preview then fails to parse.
    if (!inviteSecretPattern.hasMatch(secret)) return null;
    var rest = decoded.substring(split + 1);
    var addressCount = 0;
    final at = rest.lastIndexOf('@');
    if (at != -1) {
      final suffix = rest.substring(at + 1);
      final addrs = suffix.split(',');
      final allLookLikeAddrs = addrs.isNotEmpty &&
          addrs.every((a) {
            final colon = a.lastIndexOf(':');
            return colon != -1 && int.tryParse(a.substring(colon + 1)) != null;
          });
      if (allLookLikeAddrs) {
        addressCount = addrs.length;
        rest = rest.substring(0, at);
      }
    }
    if (rest.isEmpty) return null;
    return (rest, addressCount);
  }

  Future<void> _pasteCode() async {
    final data = await Clipboard.getData(Clipboard.kTextPlain);
    final text = data?.text?.trim();
    if (text == null || text.isEmpty) return;
    setState(() => _codeController.text = text);
  }

  Future<void> _join() async {
    final parsed = _parsed;
    if (parsed == null) return;
    setState(() {
      _busy = true;
      _error = null;
    });
    try {
      // Returns as soon as the local join record exists — any sync with
      // the addresses embedded in the code happens in the background
      // (see collections.rs::join_collection), so `collection.media` here
      // is always the pre-sync snapshot, not a verdict on whether it worked.
      final collection = await Collections.instance
          .join(_codeController.text.trim(), _nickname);
      if (mounted) {
        FocusScope.of(context).unfocus();
        _close();
        showToast(
          context,
          'Joined "${collection.name}" — syncing in the background, it will '
          'fill in on its own',
          severity: ToastSeverity.success,
        );
      }
    } catch (e) {
      setState(() => _error = 'Couldn\'t join: $e');
    } finally {
      if (mounted) setState(() => _busy = false);
    }
  }

  @override
  Widget build(BuildContext context) {
    final parsed = _parsed;

    return AppScreen(
      title: 'Join a collection',
      subtitle: const Text('Enter the invite code you were sent.'),
      onBack: _close,
      body: SingleChildScrollView(
        padding: const EdgeInsets.symmetric(horizontal: kScreenGutter),
        child: Column(
          crossAxisAlignment: CrossAxisAlignment.stretch,
          children: [
            TextField(
              key: const Key('inviteCodeField'),
              controller: _codeController,
              maxLines: 3,
              minLines: 1,
              style:
                  monoLabel(size: 13, color: AppColors.text, letterSpacing: 0),
              decoration: InputDecoration(
                hintText: 'Paste the invite code',
                hintStyle: const TextStyle(color: AppColors.textGhost),
                filled: true,
                fillColor: AppColors.surface,
                contentPadding:
                    const EdgeInsets.symmetric(horizontal: 14, vertical: 16),
                border: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(AppRadius.control),
                  borderSide: const BorderSide(color: AppColors.borderStrong),
                ),
                enabledBorder: OutlineInputBorder(
                  borderRadius: BorderRadius.circular(AppRadius.control),
                  borderSide: const BorderSide(color: AppColors.borderStrong),
                ),
              ),
              onChanged: (_) => setState(() {}),
            ),
            const SizedBox(height: 8),
            Row(
              children: [
                TextButton(
                  onPressed: _busy ? null : _pasteCode,
                  child: Text(
                    'Paste code',
                    style: AppText.body(color: AppColors.signalSoft),
                  ),
                ),
                const SizedBox(width: 8),
                Flexible(
                  child: Text(
                    parsed != null
                        ? 'Code recognised'
                        : 'Codes come from "Add collab"',
                    textAlign: TextAlign.right,
                    overflow: TextOverflow.ellipsis,
                    style: AppText.caption(),
                  ),
                ),
              ],
            ),
            if (parsed != null) ...[
              const SizedBox(height: 8),
              Container(
                padding: const EdgeInsets.all(14),
                decoration: BoxDecoration(
                  color: AppColors.surface,
                  borderRadius: BorderRadius.circular(AppRadius.inner),
                  border: Border.all(color: AppColors.border),
                ),
                child: Row(
                  children: [
                    Avatar(
                      initials:
                          parsed.$1.isEmpty ? '?' : parsed.$1[0].toUpperCase(),
                      size: 38,
                    ),
                    const SizedBox(width: 12),
                    Expanded(
                      child: Column(
                        crossAxisAlignment: CrossAxisAlignment.start,
                        children: [
                          Text(
                            parsed.$1,
                            overflow: TextOverflow.ellipsis,
                            style: AppText.cardTitle(),
                          ),
                          const SizedBox(height: 2),
                          Text(
                            parsed.$2 == 0
                                ? 'No address in the code — sync manually after joining'
                                : '${parsed.$2} address${parsed.$2 == 1 ? '' : 'es'} embedded — syncs on join',
                            style: AppText.caption(color: AppColors.textDim),
                          ),
                        ],
                      ),
                    ),
                  ],
                ),
              ),
            ],
            if (_error != null)
              Padding(
                padding: const EdgeInsets.only(top: 14),
                child: Container(
                  padding:
                      const EdgeInsets.symmetric(horizontal: 12, vertical: 9),
                  decoration: BoxDecoration(
                    color: const Color(0xFFEB5757).withValues(alpha: 0.1),
                    border: Border.all(
                        color: const Color(0xFFEB5757).withValues(alpha: 0.4)),
                    borderRadius: BorderRadius.circular(AppRadius.tight),
                  ),
                  child: Text(
                    _error!,
                    style: AppText.caption(color: Color(0xFFEB5757)),
                  ),
                ),
              ),
          ],
        ),
      ),
      footer: ScreenAction(
        buttonKey: const Key('joinCollectionButton'),
        label: 'Join',
        onPressed: parsed == null ? null : _join,
        busy: _busy,
        hint: 'You\'ll appear as $_nickname to the other members',
      ),
    );
  }
}
