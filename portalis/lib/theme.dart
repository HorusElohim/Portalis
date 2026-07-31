import 'package:flutter/material.dart';

/// "Signal" design tokens.
///
/// The governing rule: [signal] means **data is moving**, and nothing else.
/// A collection that is idle, complete, or merely selected must not be mint —
/// otherwise the colour stops carrying information and the live-transfer card
/// no longer reads at a glance. [ember] is reserved just as strictly for
/// torrent-sourced content, so the two content types stay distinguishable
/// without reading a label.
class AppColors {
  AppColors._();

  /// App background — the darkest surface.
  static const bg = Color(0xFF07090A);

  /// Screen body, one step up from [bg].
  static const surfaceDeep = Color(0xFF0B1110);

  /// Cards and list rows.
  static const surface = Color(0xFF121A18);

  /// Inputs, search fields, and anything that should read as recessed.
  static const surfaceRaised = Color(0xFF141D1B);

  /// Sidebar / secondary panes on desktop.
  static const surfaceSunken = Color(0xFF0A0F0E);

  static const border = Color(0x12FFFFFF);
  static const borderStrong = Color(0x1FFFFFFF);

  static const text = Color(0xFFE6EDEA);
  static const textDim = Color(0xFF8B9A95);
  static const textFaint = Color(0xFF7C8B86);
  static const textGhost = Color(0xFF63736E);

  /// **Data is moving.** Never decorative.
  static const signal = Color(0xFF5CE7A3);

  /// Gradient partner for [signal] — the darker end of a live progress bar.
  static const signalDim = Color(0xFF2FA97A);

  /// Text/icon mint that has to sit on a tinted fill.
  static const signalSoft = Color(0xFF9FE9C6);

  /// Muted mint for supporting metrics beside a live figure.
  static const signalMuted = Color(0xFF7F9E92);

  /// Tinted fill behind [signal] content.
  static const signalWash = Color(0x1F5CE7A3);

  /// Ink for text placed *on* a solid [signal] fill.
  static const onSignal = Color(0xFF06120D);

  /// Solid dark-mint fill — collaborator avatars, badges. Distinct from
  /// [signalWash], which is translucent and sits over arbitrary backgrounds.
  static const signalDeep = Color(0xFF2E4A41);

  /// **Torrent-sourced.** Reserved as strictly as [signal].
  static const ember = Color(0xFFF0B357);
  static const emberWash = Color(0x1FF0B357);
  static const onEmber = Color(0xFF1A1206);

  static const danger = Color(0xFFEB5757);

  /// Full-bleed media viewer backdrop.
  static const viewerBg = Color(0xFF07090A);

  /// Per-collection accents, cycled by index. Deliberately excludes [signal]
  /// and [ember]: a collection's identity colour must never be mistakable for
  /// "transferring" or "torrent".
  static const hues = <Color>[
    Color(0xFF6FCF97),
    Color(0xFF56CCF2),
    Color(0xFFBB6BD9),
    Color(0xFF7E8CE0),
    Color(0xFF4FC3A1),
    Color(0xFF9B8FE8),
  ];

  static Color hueAt(int index) => hues[index % hues.length];
}

/// Font families, bundled in `fonts/` — see pubspec.yaml.
class AppFonts {
  AppFonts._();

  /// Headings, collection names, and any large number.
  static const display = 'Space Grotesk';

  /// Body copy.
  static const body = 'Instrument Sans';

  /// Labels, metrics, hashes, addresses — anything that should read as
  /// machine output.
  static const mono = 'JetBrains Mono';
}

/// Shorthand for the recurring mono label style (uppercase, tracked out).
TextStyle monoLabel({
  double size = 10,
  Color color = AppColors.textFaint,
  double letterSpacing = 1.2,
  FontWeight weight = FontWeight.w400,
}) =>
    TextStyle(
      fontFamily: AppFonts.mono,
      fontSize: size,
      color: color,
      letterSpacing: letterSpacing,
      fontWeight: weight,
    );

/// Shorthand for display/heading text.
TextStyle displayText({
  double size = 20,
  Color color = AppColors.text,
  FontWeight weight = FontWeight.w600,
  double letterSpacing = -0.4,
  double? height,
}) =>
    TextStyle(
      fontFamily: AppFonts.display,
      fontSize: size,
      color: color,
      fontWeight: weight,
      letterSpacing: letterSpacing,
      height: height,
    );

class AppTheme {
  AppTheme._();

  static ThemeData get dark {
    final base = ThemeData(
      brightness: Brightness.dark,
      useMaterial3: true,
      scaffoldBackgroundColor: AppColors.surfaceDeep,
      fontFamily: AppFonts.body,
    );
    return base.copyWith(
      colorScheme: base.colorScheme.copyWith(
        surface: AppColors.surfaceDeep,
        primary: AppColors.signal,
        onPrimary: AppColors.onSignal,
        secondary: AppColors.signalSoft,
        error: AppColors.danger,
      ),
      textTheme: base.textTheme.apply(
        bodyColor: AppColors.text,
        displayColor: AppColors.text,
      ),
      dividerColor: AppColors.border,
      splashFactory: InkRipple.splashFactory,
      snackBarTheme: const SnackBarThemeData(
        backgroundColor: AppColors.surface,
        contentTextStyle: TextStyle(color: AppColors.text, fontSize: 13),
      ),
      dialogTheme: const DialogThemeData(backgroundColor: AppColors.surface),
    );
  }
}
