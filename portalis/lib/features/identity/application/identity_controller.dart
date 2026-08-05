import 'package:flutter/foundation.dart';

import '../data/identity_repository.dart';
import '../domain/device_profile.dart';

/// Caches the one device-level identity and notifies every interested surface
/// after a rename. It owns state only; native communication is delegated to
/// [IdentityRepository].
class IdentityController extends ChangeNotifier {
  IdentityController({required IdentityRepository repository})
      : _repository = repository;

  final IdentityRepository _repository;

  factory IdentityController.production() =>
      IdentityController(repository: const FrbIdentityRepository());
  DeviceProfile? _info;
  DeviceProfile? get info => _info;
  String? lastError;
  bool _loading = false;

  String get nickname => _info?.nickname ?? '';

  Future<void> load() async {
    if (_info != null || _loading) return;
    _loading = true;
    await Future<void>.value();
    try {
      _info = await _repository.load();
      lastError = null;
    } catch (error) {
      lastError = '$error';
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  Future<void> rename(String nickname) async {
    _info = await _repository.rename(nickname);
    lastError = null;
    notifyListeners();
  }
}
