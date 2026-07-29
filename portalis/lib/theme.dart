import 'package:flutter/material.dart';

/// Nocturne design tokens, ported from the SmartShare design exploration.
class AppColors {
  AppColors._();

  static const bg = Color(0xFF161826);
  static const surface = Color(0xFF1C1E2C);
  static const border = Color(0xFF232636);
  static const borderStrong = Color(0xFF34374A);
  static const text = Color(0xFFE9E9ED);

  static const accent = Color(0xFF9184D9);
  static const accent300 = Color(0xFFB3A9E6);
  static const accent600 = Color(0xFF584F8F);
  static const accent800 = Color(0xFF2E2A4A);

  static const neutral300 = Color(0xFFA7ABBE);
  static const neutral400 = Color(0xFF8B8FA3);
  static const neutral500 = Color(0xFF6D7186);

  static const viewerBg = Color(0xFF0B0D15);

  /// Palette used for per-collection/media "live copies" hues and piece
  /// heatmaps, cycled by index.
  static const hues = <Color>[
    Color(0xFF6FCF97),
    Color(0xFF56CCF2),
    Color(0xFFF2C94C),
    Color(0xFFEB5757),
    Color(0xFFBB6BD9),
    Color(0xFF9184D9),
  ];

  static Color hueAt(int index) => hues[index % hues.length];
}

class AppTheme {
  AppTheme._();

  static ThemeData get dark {
    final base = ThemeData(
      brightness: Brightness.dark,
      useMaterial3: true,
      scaffoldBackgroundColor: AppColors.bg,
      fontFamily: 'Inter',
    );
    return base.copyWith(
      colorScheme: base.colorScheme.copyWith(
        surface: AppColors.bg,
        primary: AppColors.accent,
        onPrimary: AppColors.bg,
        secondary: AppColors.accent300,
      ),
      textTheme: base.textTheme.apply(
        bodyColor: AppColors.text,
        displayColor: AppColors.text,
      ),
      dividerColor: AppColors.border,
      splashFactory: InkRipple.splashFactory,
    );
  }
}
