import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../bridge_generated/torrent.dart' as bridge;

/// Local app preferences (auto-seed, background sharing, etc.) plus the
/// bandwidth/storage controls that really do live in the Rust engine
/// (`torrent.rs`'s upload-limit and storage-usage functions). There's no
/// backend concept of the four on/off toggles below — they're UI-only
/// behavior this app hasn't wired to real enforcement yet — so those persist
/// via [SharedPreferences] rather than a fabricated backend model.
class SettingsService extends ChangeNotifier {
  SettingsService._();
  static final instance = SettingsService._();

  static const _kAutoSeedWifiOnly = 'autoSeedWifiOnly';
  static const _kBackgroundSharing = 'backgroundSharing';
  static const _kDiscoverable = 'discoverable';
  static const _kMeteredWarning = 'meteredWarning';
  static const _kUploadLimitBps = 'uploadLimitBps'; // 0 == unlimited
  static const _kStorageCapBytes = 'storageCapBytes';

  static const defaultStorageCapBytes = 10 * 1000 * 1000 * 1000; // 10 GB

  SharedPreferences? _prefs;

  bool autoSeedWifiOnly = true;
  bool backgroundSharing = true;
  bool discoverable = false;
  bool meteredWarning = false;

  /// 0 means unlimited.
  int uploadLimitBps = 0;
  int storageCapBytes = defaultStorageCapBytes;
  int storageUsedBytes = 0;

  bool _loaded = false;
  bool get loaded => _loaded;

  Future<void> load() async {
    final prefs = await SharedPreferences.getInstance();
    _prefs = prefs;
    autoSeedWifiOnly = prefs.getBool(_kAutoSeedWifiOnly) ?? true;
    backgroundSharing = prefs.getBool(_kBackgroundSharing) ?? true;
    discoverable = prefs.getBool(_kDiscoverable) ?? false;
    meteredWarning = prefs.getBool(_kMeteredWarning) ?? false;
    uploadLimitBps = prefs.getInt(_kUploadLimitBps) ?? 0;
    storageCapBytes = prefs.getInt(_kStorageCapBytes) ?? defaultStorageCapBytes;

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

  Future<void> setAutoSeedWifiOnly(bool v) async {
    autoSeedWifiOnly = v;
    await _prefs?.setBool(_kAutoSeedWifiOnly, v);
    notifyListeners();
  }

  Future<void> setBackgroundSharing(bool v) async {
    backgroundSharing = v;
    await _prefs?.setBool(_kBackgroundSharing, v);
    notifyListeners();
  }

  Future<void> setDiscoverable(bool v) async {
    discoverable = v;
    await _prefs?.setBool(_kDiscoverable, v);
    notifyListeners();
  }

  Future<void> setMeteredWarning(bool v) async {
    meteredWarning = v;
    await _prefs?.setBool(_kMeteredWarning, v);
    notifyListeners();
  }

  /// [bps] of 0 means unlimited.
  Future<void> setUploadLimitBps(int bps) async {
    uploadLimitBps = bps;
    await _prefs?.setInt(_kUploadLimitBps, bps);
    await bridge.setUploadLimitBps(bytesPerSec: bps == 0 ? null : bps);
    notifyListeners();
  }

  Future<void> setStorageCapBytes(int bytes) async {
    storageCapBytes = bytes;
    await _prefs?.setInt(_kStorageCapBytes, bytes);
    notifyListeners();
  }
}
