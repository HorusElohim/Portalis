import 'dart:convert';

/// Identifies the supported text inputs without performing an action. UI may
/// use this to choose an affordance; commands remain the controller's job.
enum PasteKind {
  empty,
  magnet,
  invite,
  search;

  static PasteKind of(String raw) {
    final value = raw.trim();
    if (value.isEmpty) return PasteKind.empty;
    if (looksLikeMagnet(value)) return PasteKind.magnet;
    if (looksLikeInviteCode(value)) return PasteKind.invite;
    return PasteKind.search;
  }
}

/// A magnet URI or the bare 40-character info hash accepted by librqbit.
bool looksLikeMagnet(String value) {
  final trimmed = value.trim();
  return trimmed.startsWith('magnet:?') ||
      RegExp(r'^[0-9a-fA-F]{40}$').hasMatch(trimmed);
}

/// The 32-byte invite credential that begins a valid decoded invite.
final RegExp inviteSecretPattern = RegExp(r'^[0-9a-fA-F]{64}$');

/// Decodes the human-shareable hexadecimal invite envelope, returning null
/// when it is not valid hexadecimal UTF-8 with the minimum `secret:name`
/// shape. Rust remains the final authority for accepting an invite.
String? decodeInviteCode(String code) {
  final trimmed = code.trim();
  if (trimmed.isEmpty || trimmed.length.isOdd) return null;
  final bytes = <int>[];
  for (var index = 0; index < trimmed.length; index += 2) {
    final byte = int.tryParse(trimmed.substring(index, index + 2), radix: 16);
    if (byte == null) return null;
    bytes.add(byte);
  }
  try {
    final decoded = utf8.decode(bytes);
    return decoded.contains(':') ? decoded : null;
  } catch (_) {
    return null;
  }
}

bool looksLikeInviteCode(String code) {
  final decoded = decodeInviteCode(code);
  if (decoded == null) return false;
  final separator = decoded.indexOf(':');
  return separator >= 0 &&
      inviteSecretPattern.hasMatch(decoded.substring(0, separator));
}
