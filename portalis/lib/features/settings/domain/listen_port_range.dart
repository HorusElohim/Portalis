/// A validated inclusive TCP listening range selected in Settings.
class ListenPortRange {
  const ListenPortRange(this.start, this.end);

  final int start;
  final int end;
}

/// Accepts `6881-6999` and the typographic `6881–6999` form shown in the UI.
/// Whitespace around the separator is irrelevant. Invalid and out-of-range
/// ports are rejected here instead of relying on a later FFI conversion error.
ListenPortRange? parseListenPortRange(String raw) {
  final parts = raw.trim().split(RegExp(r'\s*[-\u2013]\s*'));
  if (parts.isEmpty || parts.length > 2) return null;
  final start = int.tryParse(parts.first);
  final end = int.tryParse(parts.length == 1 ? parts.first : parts.last);
  if (start == null || end == null || start < 1 || end > 65535 || start > end) {
    return null;
  }
  return ListenPortRange(start, end);
}
