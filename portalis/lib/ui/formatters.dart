// Part of the Portalis UI kit — see ui.dart.

/// Byte/rate formatting, in one place.
///
/// Five screens each carried their own private `_formatBytes`, and two of them
/// disagreed about precision — so the same figure read differently depending
/// on where you saw it. These are the single source.
///
/// Decimal units (1 MB = 1,000,000 B) throughout, matching what the engine
/// reports and what a transfer speed conventionally means.
library;

const _kb = 1000;
const _mb = 1000000;
const _gb = 1000000000;

/// `1.4 GB` / `842 MB` / `12 KB` / `340 B`.
String formatBytes(int bytes) {
  if (bytes >= _gb) return '${(bytes / _gb).toStringAsFixed(1)} GB';
  if (bytes >= _mb) return '${(bytes / _mb).toStringAsFixed(0)} MB';
  if (bytes >= _kb) return '${(bytes / _kb).toStringAsFixed(0)} KB';
  return '$bytes B';
}

/// Like [formatBytes] but keeps two decimals on GB/MB, for detail panels
/// where the exact size is the point.
String formatBytesPrecise(int bytes) {
  if (bytes >= _gb) return '${(bytes / _gb).toStringAsFixed(2)} GB';
  if (bytes >= _mb) return '${(bytes / _mb).toStringAsFixed(1)} MB';
  if (bytes >= _kb) return '${(bytes / _kb).toStringAsFixed(0)} KB';
  return '$bytes B';
}

/// A throughput figure the engine reports, in MB/s.
String formatRate(double mbps) => '${mbps.toStringAsFixed(1)} MB/s';

/// A configured bytes-per-second cap. `null` or `0` means no cap — and says
/// so, rather than rendering as "0 B/s".
String formatLimit(int? bytesPerSecond) {
  if (bytesPerSecond == null || bytesPerSecond == 0) return 'Unlimited';
  if (bytesPerSecond < _mb) {
    return '${(bytesPerSecond / _kb).toStringAsFixed(0)} KB/s';
  }
  return '${(bytesPerSecond / _mb).toStringAsFixed(1)} MB/s';
}

/// `3 peers` / `1 peer`. English pluralisation only — there is no
/// localisation layer in this app yet, and pretending otherwise would be
/// worse than being explicit about it.
String plural(int n, String singular, [String? pluralForm]) =>
    '$n ${n == 1 ? singular : (pluralForm ?? '${singular}s')}';

/// An optional duration setting, in seconds.
String formatSeconds(int? seconds) =>
    seconds == null ? 'Engine default' : '${seconds}s';
