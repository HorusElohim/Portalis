import 'package:flutter/foundation.dart';

import '../bridge_generated/settings.dart' as bridge;
import '../bridge_generated/torrent.dart' as torrent_bridge;

export '../bridge_generated/settings.dart' show EngineSettings;

/// The BitTorrent engine's configuration, as librqbit actually exposes it.
///
/// Every field here maps to a real `SessionOptions` field (see
/// `rust/backend/src/settings.rs`) — there are no app-invented preferences.
/// Rust owns persistence and validation; this is a cache plus change
/// notification.
///
/// Only the two rate limits are adjustable on a live session. Everything else
/// is read by librqbit once, when the session is constructed, so [save]
/// reports whether the change needs a restart rather than pretending it took
/// effect.
class SettingsService extends ChangeNotifier {
  SettingsService._();
  static final instance = SettingsService._();

  bridge.EngineSettings? _settings;
  bridge.EngineSettings? get settings => _settings;

  int storageUsedBytes = 0;
  String? lastError;

  bool get loaded => _settings != null;

  Future<void> load() async {
    try {
      _settings = await bridge.engineSettings();
      lastError = null;
    } catch (e) {
      lastError = '$e';
    }
    await refreshStorageUsage();
    notifyListeners();
  }

  /// Persists and applies. Returns `true` when the change only takes effect
  /// after a restart.
  Future<bool> save(bridge.EngineSettings next) async {
    // Optimistic: show the new value immediately, then reconcile with what
    // Rust actually stored (it validates and may reject).
    _settings = next;
    notifyListeners();
    try {
      final restartRequired = await bridge.setEngineSettings(settings: next);
      _settings = await bridge.engineSettings();
      lastError = null;
      notifyListeners();
      return restartRequired;
    } catch (e) {
      lastError = '$e';
      // Roll back to whatever is genuinely stored, so a rejected value
      // doesn't linger in the UI as though it had been accepted.
      try {
        _settings = await bridge.engineSettings();
      } catch (_) {
        // Leave the optimistic value; the error is already surfaced.
      }
      notifyListeners();
      rethrow;
    }
  }

  Future<bool> resetToDefaults() async =>
      save(await bridge.defaultEngineSettings());

  /// Test seam, mirroring `Collections.debugSeed`: widget tests run without
  /// `RustLib`, so [load] always fails and the screen would only ever render
  /// its loading state.
  @visibleForTesting
  void debugSeed(bridge.EngineSettings settings) {
    _settings = settings;
    lastError = null;
    notifyListeners();
  }

  Future<void> refreshStorageUsage() async {
    try {
      storageUsedBytes = (await torrent_bridge.storageUsageBytes()).toInt();
      notifyListeners();
    } catch (_) {
      // Leave the last-known value in place on failure.
    }
  }
}
