import 'package:flutter/material.dart';

/// A full set of the app's color roles — one instance per theme.
///
/// [AppColors] (in `theme.dart`) is the only thing call sites ever read;
/// this class exists solely so there can be more than one of it. Adding a
/// third theme is: add a field-for-field twin of [nature] or [future] below,
/// nothing else.
@immutable
class AppPalette {
  const AppPalette({
    required this.bg,
    required this.surfaceDeep,
    required this.surface,
    required this.surfaceRaised,
    required this.surfaceSunken,
    required this.border,
    required this.borderStrong,
    required this.text,
    required this.textDim,
    required this.textFaint,
    required this.textGhost,
    required this.signal,
    required this.signalDim,
    required this.signalSoft,
    required this.signalMuted,
    required this.signalWash,
    required this.onSignal,
    required this.signalDeep,
    required this.ember,
    required this.emberWash,
    required this.onEmber,
    required this.danger,
    required this.viewerBg,
  });

  final Color bg;
  final Color surfaceDeep;
  final Color surface;
  final Color surfaceRaised;
  final Color surfaceSunken;
  final Color border;
  final Color borderStrong;
  final Color text;
  final Color textDim;
  final Color textFaint;
  final Color textGhost;
  final Color signal;
  final Color signalDim;
  final Color signalSoft;
  final Color signalMuted;
  final Color signalWash;
  final Color onSignal;
  final Color signalDeep;
  final Color ember;
  final Color emberWash;
  final Color onEmber;
  final Color danger;
  final Color viewerBg;

  /// The original palette. Mint [signal], amber [ember].
  static const nature = AppPalette(
    bg: Color(0xFF07090A),
    surfaceDeep: Color(0xFF0B1110),
    surface: Color(0xFF121A18),
    surfaceRaised: Color(0xFF141D1B),
    surfaceSunken: Color(0xFF0A0F0E),
    border: Color(0x12FFFFFF),
    borderStrong: Color(0x1FFFFFFF),
    text: Color(0xFFE6EDEA),
    textDim: Color(0xFF8B9A95),
    textFaint: Color(0xFF7C8B86),
    textGhost: Color(0xFF63736E),
    signal: Color(0xFF5CE7A3),
    signalDim: Color(0xFF2FA97A),
    signalSoft: Color(0xFF9FE9C6),
    signalMuted: Color(0xFF7F9E92),
    signalWash: Color(0x1F5CE7A3),
    onSignal: Color(0xFF06120D),
    signalDeep: Color(0xFF2E4A41),
    ember: Color(0xFFF0B357),
    emberWash: Color(0x1FF0B357),
    onEmber: Color(0xFF1A1206),
    danger: Color(0xFFEB5757),
    viewerBg: Color(0xFF07090A),
  );

  /// Drawn from `assets/PortalisFuture.png`: electric cyan [signal], the
  /// icon's cyan/blue side; violet-magenta [ember], its opposite side — the
  /// same hue separation the mint/amber pair keeps in [nature].
  static const future = AppPalette(
    bg: Color(0xFF06070D),
    surfaceDeep: Color(0xFF0A0C18),
    surface: Color(0xFF121629),
    surfaceRaised: Color(0xFF141A30),
    surfaceSunken: Color(0xFF090B1A),
    border: Color(0x14FFFFFF),
    borderStrong: Color(0x22FFFFFF),
    text: Color(0xFFE8ECFB),
    textDim: Color(0xFF8D93B8),
    textFaint: Color(0xFF7A80A3),
    textGhost: Color(0xFF5F6488),
    signal: Color(0xFF22D3EE),
    signalDim: Color(0xFF0B7EC4),
    signalSoft: Color(0xFF9FE9FF),
    signalMuted: Color(0xFF7FA3B8),
    signalWash: Color(0x1F22D3EE),
    onSignal: Color(0xFF00181D),
    signalDeep: Color(0xFF123B4A),
    ember: Color(0xFFE85CF0),
    emberWash: Color(0x1FE85CF0),
    onEmber: Color(0xFF1A0620),
    danger: Color(0xFFEB5757),
    viewerBg: Color(0xFF06070D),
  );
}
