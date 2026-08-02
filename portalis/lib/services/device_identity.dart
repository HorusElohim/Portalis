import 'package:flutter/foundation.dart';

import '../bridge_generated/device.dart' as bridge;

/// This device's identity, held once for the whole app.
///
/// Three places used to call `deviceIdentity()` for themselves and keep the
/// answer in their own `State`: the You screen, the desktop sidebar's chip,
/// and the join screen. Each therefore held its own copy of the nickname,
/// loaded once, with no way to hear that another had changed it — so renaming
/// yourself left the sidebar addressing you by your old name until the app was
/// restarted. The name is device-level state, so it belongs here rather than
/// in whichever widget happened to ask first.
class DeviceIdentity extends ChangeNotifier {
  DeviceIdentity._();
  static final instance = DeviceIdentity._();

  bridge.DeviceIdentityInfo? _info;
  bridge.DeviceIdentityInfo? get info => _info;

  /// The last failure, so a screen can say why it has no name to show instead
  /// of inventing one.
  String? lastError;

  bool _loading = false;

  String get nickname => _info?.nickname ?? '';

  /// Loads once. Safe to call from every `initState` that needs it — later
  /// callers get the cached value rather than another FFI round trip.
  Future<void> load() async {
    if (_info != null || _loading) return;
    _loading = true;
    // Yield before going near the bridge. This is called from `initState`, and
    // an FFI call that fails *synchronously* — which is exactly what happens
    // whenever `RustLib` isn't initialised — would run the catch, the finally,
    // and `notifyListeners()` in the middle of the build that created the
    // widget, which Flutter rejects outright.
    // A microtask, not a timer: this resumes as soon as the current build
    // finishes rather than on a future frame, and a pending timer would leave
    // every widget test that renders a screen complaining about one.
    await Future<void>.value();
    try {
      _info = await bridge.deviceIdentity();
      lastError = null;
    } catch (e) {
      lastError = '$e';
    } finally {
      _loading = false;
      notifyListeners();
    }
  }

  /// Renames this device everywhere at once. Rust also rewrites this device's
  /// collaborator record in every collection it belongs to (see
  /// `collections.rs::list_collections`), so the new name reaches peers on the
  /// next sync; this makes it reach the rest of the app immediately.
  Future<void> rename(String nickname) async {
    _info = await bridge.setNickname(nickname: nickname);
    lastError = null;
    notifyListeners();
  }
}
