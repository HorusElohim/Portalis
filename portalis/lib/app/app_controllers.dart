import '../features/appearance/application/theme_controller.dart';
import '../features/collections/application/collections_controller.dart';
import '../features/identity/application/identity_controller.dart';
import '../features/settings/application/settings_controller.dart';

/// Application-owned controller instances shared by the widget tree.
///
/// Features expose controllers, repositories, and domain models; this is the
/// sole place that chooses their production implementations and lifetime.
abstract final class AppControllers {
  static final collections = CollectionsController.production();
  static final identity = IdentityController.production();
  static final settings = SettingsController.production();
  static final theme = ThemeController.instance;
}
