/// The local device identity safe to show in Flutter. The signing key remains
/// exclusively in Rust and never crosses the Flutter-Rust boundary.
class DeviceProfile {
  const DeviceProfile({required this.deviceId, required this.nickname});

  final String deviceId;
  final String nickname;
}
