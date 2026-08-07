import 'package:flutter/foundation.dart';
import 'package:shared_preferences/shared_preferences.dart';

import '../../../theme_palette.dart';

enum AppThemeId {
  nature,
  future;

  AppPalette get palette =>
      this == AppThemeId.future ? AppPalette.future : AppPalette.nature;
}

/// The one live palette choice, read by every [AppColors] getter.
///
/// A singleton rather than an [InheritedWidget]: `theme.dart`'s design
/// tokens are read as static properties from hundreds of call sites with no
/// [BuildContext] in reach, so the source of truth has to be reachable the
/// same way. [portalis_app.dart] still listens for the rebuild; this just
/// holds the value.
class ThemeController extends ChangeNotifier {
  ThemeController._();

  static final instance = ThemeController._();

  static const _prefsKey = 'app.theme.v1';

  AppThemeId _id = AppThemeId.nature;
  AppThemeId get id => _id;
  AppPalette get palette => _id.palette;

  bool _loaded = false;
  bool get loaded => _loaded;

  /// Reads the persisted choice. Awaited before `runApp` in bootstrap so the
  /// first frame already paints the right theme — no flash of Nature before
  /// a stored Future preference lands.
  Future<void> load() async {
    if (_loaded) return;
    final preferences = await SharedPreferences.getInstance();
    final stored = preferences.getString(_prefsKey);
    _id = AppThemeId.values.firstWhere(
      (value) => value.name == stored,
      orElse: () => AppThemeId.nature,
    );
    _loaded = true;
    notifyListeners();
  }

  Future<void> setTheme(AppThemeId id) async {
    if (id == _id) return;
    _id = id;
    notifyListeners();
    final preferences = await SharedPreferences.getInstance();
    await preferences.setString(_prefsKey, id.name);
  }
}
