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

/// The type scale — five steps of body text, plus [monoLabel] for machine
/// output and [canvasTitle]/[impactTitle] for headings.
///
/// Five, because the app had fifteen: 9, 10, 10.5, 11, 11.5, 12, 12.5, 13,
/// 13.5, 14, 14.5, 15, 16, 20 and 25, each picked at the call site that
/// needed it. Nothing could be restyled without finding every literal, and
/// two screens showing the same kind of text routinely disagreed by half a
/// point — which is invisible on its own and exactly why it kept happening.
///
/// The steps are roles, not sizes: ask for what the text *is* and the scale
/// decides how big it is. Restyling the app's text is then this class and
/// nothing else — the same way [AppColors] is already the only place the
/// palette lives, and [AppRadius] the only place corners do.
class AppText {
  AppText._();

  /// The label on a button that completes something — see `ScreenAction`,
  /// and the one line of a preview that names what you are about to add.
  static TextStyle action({Color color = AppColors.text, double? height}) =>
      _body(16, color, FontWeight.w500, height);

  /// The heading inside a card or a row: "File formats", "People".
  static TextStyle cardTitle({Color color = AppColors.text}) =>
      _body(14.5, color, FontWeight.w600);

  /// Default reading text, and the label half of a settings row.
  static TextStyle body({
    Color color = AppColors.text,
    double? height,
    FontWeight weight = FontWeight.w400,
  }) =>
      _body(13.5, color, weight, height);

  /// Supporting copy under a title — the explanation, not the thing.
  static TextStyle secondary({
    Color color = AppColors.textFaint,
    double? height,
  }) =>
      _body(12.5, color, FontWeight.w400, height);

  /// The smallest step: hints under an action, helper text under a field,
  /// and asides. [weight] because a caption occasionally has to carry a
  /// name — a linked collection, a selected tab — and emphasis is the only
  /// thing left to say it with at this size.
  static TextStyle caption({
    Color color = AppColors.textGhost,
    double? height,
    FontWeight weight = FontWeight.w400,
  }) =>
      _body(11.5, color, weight, height);

  static TextStyle _body(
    double size,
    Color color,
    FontWeight weight, [
    double? height,
  ]) =>
      TextStyle(
        fontFamily: AppFonts.body,
        fontSize: size,
        color: color,
        fontWeight: weight,
        height: height,
      );
}

/// Corner radii, as roles rather than numbers.
///
/// Five, replacing the twelve the app had reached — 6, 7, 8, 10, 11, 12, 14,
/// 15, 20, 22, 99 and 999 — where the difference between a 10 and an 11 was
/// never a decision anyone made.
class AppRadius {
  AppRadius._();

  /// Fully round: pills, bars, progress tracks.
  static const pill = 99.0;

  /// A card or panel — see `SurfaceCard`.
  static const card = 20.0;

  /// Buttons and text fields.
  static const control = 14.0;

  /// Something nested inside a card: a chip, a thumbnail, an inner tile.
  static const inner = 11.0;

  /// The smallest corner — a badge or a tiny glyph tile.
  static const tight = 7.0;
}

/// Shorthand for the recurring mono label style (uppercase, tracked out).
///
/// The mono step of [AppText]'s scale — kept a standalone function because
/// its callers vary letter-spacing and size far more than body text does.
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

/// The big titles on a canvas — screen headings and the one name a screen is
/// about. Uppercase, heavy, and tracked *in* rather than out, so a heading
/// reads as a block rather than a sentence.
///
/// The tall/condensed look this is after really wants a condensed family;
/// none is bundled, and a font the app doesn't ship would silently fall back
/// to the system one — which is the bug this project already had once, when
/// the theme named 'Inter' and nothing was ever loading it. So it is built
/// from Space Grotesk at its heaviest, with the tracking and line height doing
/// the rest. Drop a condensed `.ttf` in `fonts/` and this is the one place to
/// point at it.
TextStyle canvasTitle({
  double size = 30,
  Color color = AppColors.text,
  double? height,
}) =>
    TextStyle(
      fontFamily: AppFonts.display,
      fontSize: size,
      color: color,
      fontWeight: FontWeight.w700,
      // Proportional, not fixed: the same visual tightness at 20pt and 40pt.
      letterSpacing: size * -0.025,
      height: height ?? 0.98,
    );

/// Poster-scale heading for a pane that's a destination in its own right —
/// People, Settings — rather than one of several reached from the same nav
/// (see [canvasTitle] for those). The extra presence comes from scale and a
/// soft glow rather than a heavier cut: Space Grotesk's variable range tops
/// out at the same 700 [canvasTitle] already uses.
TextStyle impactTitle({
  double size = 46,
  Color color = AppColors.text,
  Color glow = AppColors.signal,
}) =>
    TextStyle(
      fontFamily: AppFonts.display,
      fontSize: size,
      color: color,
      fontWeight: FontWeight.w700,
      letterSpacing: size * -0.035,
      height: 0.92,
      shadows: [
        Shadow(color: glow.withValues(alpha: 0.4), blurRadius: size * 0.5),
      ],
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
      snackBarTheme: SnackBarThemeData(
        backgroundColor: AppColors.surface,
        contentTextStyle: AppText.body(),
      ),
      dialogTheme: const DialogThemeData(backgroundColor: AppColors.surface),
    );
  }
}

/// Glow — the "positive energy" layer of the theme.
///
/// A scalable set rather than a one-off effect: every highlight in the app
/// asks for a [GlowLevel] and gets a consistent border tint, shadow spread and
/// opacity back. Adding a new level, or retuning the whole app's energy, is a
/// change to this one table.
///
/// The same discipline as the rest of the palette applies: glow marks
/// something *alive* — sharing, receiving, connected — never mere selection or
/// decoration. A screen where everything glows says nothing.
enum GlowLevel {
  /// No energy. Settled, idle, or purely structural.
  none,

  /// Present and healthy, but not transferring — a collection that is being
  /// shared and simply has no taker right now.
  calm,

  /// Actively doing something.
  active,

  /// Doing something at full tilt.
  vivid,
}

/// The resolved appearance of a [GlowLevel] in a given colour.
@immutable
class Glow {
  const Glow({
    required this.level,
    required this.color,
    required this.borderOpacity,
    required this.blur,
    required this.spread,
    required this.shadowOpacity,
  });

  final GlowLevel level;
  final Color color;
  final double borderOpacity;
  final double blur;
  final double spread;
  final double shadowOpacity;

  bool get isVisible => level != GlowLevel.none;

  Border get border => Border.all(
        color: isVisible
            ? color.withValues(alpha: borderOpacity)
            : AppColors.border,
        width: level == GlowLevel.vivid ? 1.4 : 1,
      );

  List<BoxShadow> get shadows => isVisible
      ? [
          BoxShadow(
            color: color.withValues(alpha: shadowOpacity),
            blurRadius: blur,
            spreadRadius: spread,
          ),
        ]
      : const [];

  /// Looks up the tuned appearance for a level. One table, so retuning the
  /// app's energy is a single edit.
  static Glow of(GlowLevel level, {Color color = AppColors.signal}) =>
      switch (level) {
        GlowLevel.none => Glow(
            level: level,
            color: color,
            borderOpacity: 0,
            blur: 0,
            spread: 0,
            shadowOpacity: 0,
          ),
        GlowLevel.calm => Glow(
            level: level,
            color: color,
            borderOpacity: 0.26,
            blur: 14,
            spread: -4,
            shadowOpacity: 0.10,
          ),
        GlowLevel.active => Glow(
            level: level,
            color: color,
            borderOpacity: 0.40,
            blur: 22,
            spread: -2,
            shadowOpacity: 0.18,
          ),
        GlowLevel.vivid => Glow(
            level: level,
            color: color,
            borderOpacity: 0.58,
            blur: 30,
            spread: 0,
            shadowOpacity: 0.26,
          ),
      };
}
