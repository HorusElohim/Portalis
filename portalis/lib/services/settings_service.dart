import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../bridge_generated/torrent.dart' as bridge;

/// The settings the Rust engine actually enforces: the upload limit
/// (`torrent.rs::set_upload_limit_bps`, applied to the live session) and the
/// reported storage usage (`torrent.rs::storage_usage_bytes`).
///
/// Deliberately nothing else. The design mockup's on/off toggles (auto-seed
/// on Wi-Fi only, background sharing, discoverable, metered-connection
/// warning) and a storage *cap* were removed rather than kept as switches
/// that persisted a preference no code path ever read — a setting that
/// silently does nothing is worse than an absent one. Re-add each when the
/// behaviour behind it exists.
class SettingsService extends ChangeNotifier {
  SettingsService._();
  static final instance = SettingsService._();

  static const _kUploadLimitBps = 'uploadLimitBps'; // 0 == unlimited

  SharedPreferences? _prefs;

  /// 0 means unlimited.
  int uploadLimitBps = 0;
  int storageUsedBytes = 0;

  bool _loaded = false;
  bool get loaded => _loaded;

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    _prefs = prefs;
    uploadLimitBps = prefs.getInt(_kUploadLimitBps) ?? 0;

    // Apply the persisted upload limit to the running session — the engine
    // itself doesn't remember this across restarts.
    try {
      await bridge.setUploadLimitBps(
        bytesPerSec: uploadLimitBps == 0 ? null : uploadLimitBps,
      );
    } catch (_) {
      // Non-fatal — session may not be up yet; the value is still saved
      // and will be re-applied on the next successful load().
    }

    await refreshStorageUsage();
    _loaded = true;
    notifyListeners();
  }

  Future<void> refreshStorageUsage() async {
    try {
      storageUsedBytes = (await bridge.storageUsageBytes()).toInt();
      notifyListeners();
    } catch (_) {
      // Leave the last-known value in place on failure.
    }
  }

  /// [bps] of 0 means unlimited.
  Future<void> setUploadLimitBps(int bps) async {
    uploadLimitBps = bps;
    await _prefs?.setInt(_kUploadLimitBps, bps);
    await bridge.setUploadLimitBps(bytesPerSec: bps == 0 ? null : bps);
    notifyListeners();
  }
}
