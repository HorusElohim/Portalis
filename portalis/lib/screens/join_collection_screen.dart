import 'dart:convert';

import 'package:flutter/material.dart';
import 'package:flutter/services.dart';

import '../bridge_generated/device.dart' as device_bridge;
import '../services/collections.dart';
import '../theme.dart';
import '../widgets/common.dart';

/// Un-hexes an invite code back to `<secret>:<name>[@addr1,addr2]`, or
/// null if it isn't valid hex / valid UTF-8 — mirrors
/// `collab.rs::parse_invite_code`'s first step. The hex layer isn't
/// encryption (the code is already the join credential), just enough to
/// keep the address/name out of plain sight in a screenshot or clipboard
/// history.
String? _unhex(String code) {
  final trimmed = code.trim();
  if (trimmed.isEmpty || trimmed.length.isOdd) return null;
  final bytes = <int>[];
  for (var i = 0; i < trimmed.length; i += 2) {
    final byte = int.tryParse(trimmed.substring(i, i + 2), radix: 16);
    if (byte == null) return null;
    bytes.add(byte);
  }
  try {
    return utf8.decode(bytes);
  } catch (_) {
    return null;
  }
}

/// "Join a collection" — the invite-code half of the old combined Add
/// screen, redesigned per the Portalis Add Flow. The preview card is
/// parsed from the code itself (hex-wrapped
/// `<secret>:<name>[@addresses]`), so what's shown before joining is real:
/// the collection's name and whether the invite carries reachable
/// addresses. The display name comes from this device's identity nickname
/// instead of a second text field.
class JoinCollectionScreen extends StatefulWidget {
  const JoinCollectionScreen({super.key});

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

  /// `(name, addressCount)` when the code un-hexes to
  /// `<64-hex secret>:<name>[@addr1,addr2]`, else null.
  (String, int)? get _parsed {
    final decoded = _unhex(_codeController.text);
    if (decoded == null) return null;
    final split = decoded.indexOf(':');
    if (split == -1) return null;
    final secret = decoded.substring(0, split);
    if (!RegExp(r'^[0-9a-fA-F]{64}$').hasMatch(secret)) return null;
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
        Navigator.of(context).pop();
        ScaffoldMessenger.of(context).showSnackBar(SnackBar(
          content: Text(
            'Joined "${collection.name}" — syncing in the background, it will '
            'fill in on its own',
          ),
        ));
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

    return Scaffold(
      backgroundColor: AppColors.bg,
      body: SafeArea(
        child: Column(
          children: [
            Align(
              alignment: Alignment.centerLeft,
              child: TextButton.icon(
                onPressed: () => Navigator.of(context).pop(),
                icon: const Icon(Icons.chevron_left,
                    size: 18, color: AppColors.neutral300),
                label: const Text('Back',
                    style: TextStyle(fontSize: 14, color: AppColors.neutral300)),
              ),
            ),
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 2, 20, 18),
              child: Column(
                crossAxisAlignment: CrossAxisAlignment.start,
                children: const [
                  Text(
                    'Join a collection',
                    style: TextStyle(
                      fontSize: 25,
                      fontWeight: FontWeight.w500,
                      letterSpacing: -0.4,
                    ),
                  ),
                  SizedBox(height: 4),
                  Text(
                    'Enter the invite code you were sent.',
                    style: TextStyle(fontSize: 12.5, color: AppColors.neutral400),
                  ),
                ],
              ),
            ),
            Expanded(
              child: SingleChildScrollView(
                padding: const EdgeInsets.symmetric(horizontal: 20),
                child: Column(
                  crossAxisAlignment: CrossAxisAlignment.stretch,
                  children: [
                    TextField(
                      key: const Key('inviteCodeField'),
                      controller: _codeController,
                      maxLines: 3,
                      minLines: 1,
                      style: const TextStyle(
                        fontSize: 13,
                        fontFamily: 'monospace',
                        color: AppColors.text,
                      ),
                      decoration: InputDecoration(
                        hintText: 'Paste the invite code',
                        hintStyle: const TextStyle(color: AppColors.neutral500),
                        filled: true,
                        fillColor: AppColors.surface,
                        contentPadding: const EdgeInsets.symmetric(
                            horizontal: 14, vertical: 16),
                        border: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(14),
                          borderSide:
                              const BorderSide(color: AppColors.borderStrong),
                        ),
                        enabledBorder: OutlineInputBorder(
                          borderRadius: BorderRadius.circular(14),
                          borderSide:
                              const BorderSide(color: AppColors.borderStrong),
                        ),
                      ),
                      onChanged: (_) => setState(() {}),
                    ),
                    const SizedBox(height: 8),
                    Row(
                      children: [
                        TextButton(
                          onPressed: _busy ? null : _pasteCode,
                          child: const Text(
                            'Paste code',
                            style: TextStyle(
                                fontSize: 13.5, color: AppColors.accent300),
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
                            style: const TextStyle(
                                fontSize: 11.5, color: AppColors.neutral500),
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
                          borderRadius: BorderRadius.circular(12),
                          border: Border.all(color: AppColors.border),
                        ),
                        child: Row(
                          children: [
                            Avatar(
                              initials: parsed.$1.isEmpty
                                  ? '?'
                                  : parsed.$1[0].toUpperCase(),
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
                                    style: const TextStyle(
                                        fontSize: 14.5,
                                        fontWeight: FontWeight.w500),
                                  ),
                                  const SizedBox(height: 2),
                                  Text(
                                    parsed.$2 == 0
                                        ? 'No address in the code — sync manually after joining'
                                        : '${parsed.$2} address${parsed.$2 == 1 ? '' : 'es'} embedded — syncs on join',
                                    style: const TextStyle(
                                        fontSize: 11.5,
                                        color: AppColors.neutral400),
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
            Padding(
              padding: const EdgeInsets.fromLTRB(20, 12, 20, 16),
              child: Column(
                children: [
                  SizedBox(
                    width: double.infinity,
                    height: 52,
                    child: FilledButton(
                      key: const Key('joinCollectionButton'),
                      onPressed: _busy || parsed == null ? null : _join,
                      style: FilledButton.styleFrom(
                        backgroundColor: AppColors.accent,
                        disabledBackgroundColor: AppColors.borderStrong,
                        foregroundColor: AppColors.bg,
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
                                    AlwaysStoppedAnimation(AppColors.bg),
                              ),
                            )
                          : const Text('Join', style: TextStyle(fontSize: 16)),
                    ),
                  ),
                  const SizedBox(height: 8),
                  Text(
                    'You\'ll appear as $_nickname to the other members',
                    style: const TextStyle(
                        fontSize: 11.5, color: AppColors.neutral500),
                  ),
                ],
              ),
            ),
          ],
        ),
      ),
    );
  }
}
