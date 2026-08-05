import 'package:flutter/foundation.dart';

import '../data/settings_repository.dart';
import '../domain/engine_settings.dart';
import '../domain/storage_entry.dart';

/// UI state and commands for engine configuration and download storage.
class SettingsController extends ChangeNotifier {
  SettingsController({required SettingsRepository repository})
      : _repository = repository;

  final SettingsRepository _repository;

  factory SettingsController.production() =>
      SettingsController(repository: const FrbSettingsRepository());

  EngineSettings? _settings;
  EngineSettings? get settings => _settings;
  int storageUsedBytes = 0;
  String? lastError;

  bool get loaded => _settings != null;

  Future<void> load() async {
    try {
      _settings = await _repository.load();
      lastError = null;
    } catch (error) {
      lastError = '$error';
    }
    await refreshStorageUsage();
    notifyListeners();
  }

  /// Persists changes and reports whether a restart is needed.
  Future<bool> save(EngineSettings next) async {
    _settings = next;
    notifyListeners();
    try {
      final restartRequired = await _repository.save(next);
      _settings = await _repository.load();
      lastError = null;
      notifyListeners();
      return restartRequired;
    } catch (error) {
      lastError = '$error';
      try {
        _settings = await _repository.load();
      } catch (_) {
        // Preserve the optimistic value while the error remains visible.
      }
      notifyListeners();
      rethrow;
    }
  }

  Future<bool> resetToDefaults() async => save(await _repository.defaults());

  Future<void> refreshStorageUsage() async {
    try {
      storageUsedBytes = await _repository.storageUsageBytes();
      notifyListeners();
    } catch (_) {
      // Storage is auxiliary information; retain the last known value.
    }
  }

  Future<List<StorageEntry>> storageBreakdown() => _repository.storageBreakdown();

  @visibleForTesting
  void debugSeed(EngineSettings settings) {
    _settings = settings;
    lastError = null;
    notifyListeners();
  }
}
