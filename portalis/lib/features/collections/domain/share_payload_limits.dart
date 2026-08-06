/// Limits for the one-shot file payload used by the Flutter-Rust bridge.
///
/// The current generated bridge serializes byte-vector lengths as signed
/// 32-bit integers. Keep the application limit below that wire limit so a
/// large selection becomes a normal validation error instead of an FFI panic.
abstract final class SharePayloadLimits {
  static const maxFiles = 10000;
  static const maxBytes = 2000000000;

  static String? error({
    required int fileCount,
    required int totalBytes,
    required int largestFileBytes,
  }) {
    if (fileCount == 0) return 'Add at least one file';
    if (fileCount > maxFiles) {
      return 'A share can contain at most $maxFiles files';
    }
    if (largestFileBytes > maxBytes) {
      return 'One selected file is too large to share in one operation';
    }
    if (totalBytes > maxBytes) {
      return 'The selected files are too large to share in one operation';
    }
    return null;
  }
}
