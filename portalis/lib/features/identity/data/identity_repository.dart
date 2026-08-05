import '../../../bridge_generated/device.dart' as bridge;
import '../domain/device_profile.dart';

abstract interface class IdentityRepository {
  Future<DeviceProfile> load();
  Future<DeviceProfile> rename(String nickname);
}

/// The sole identity adapter that understands generated native DTOs.
class FrbIdentityRepository implements IdentityRepository {
  const FrbIdentityRepository();

  @override
  Future<DeviceProfile> load() async => _map(await bridge.deviceIdentity());

  @override
  Future<DeviceProfile> rename(String nickname) async =>
      _map(await bridge.setNickname(nickname: nickname));

  DeviceProfile _map(bridge.DeviceIdentityInfo info) => DeviceProfile(
        deviceId: info.deviceId,
        nickname: info.nickname,
      );
}
