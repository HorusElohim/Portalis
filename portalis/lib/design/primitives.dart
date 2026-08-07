// Cross-feature layout design primitive.

import 'package:flutter/material.dart';

import '../theme.dart';
import 'window.dart';

/// The horizontal gutter every screen's content sits inside.
///
/// One number, because there is one correct answer. Screens used to pick
/// their own — 20 on Settings and the flow screens, 22 on Formats, Storage
/// and Collections, 28 on People and the desktop Collections pane — so two
/// panes of the same window disagreed about where their left edge was, and
/// the desktop Collections pane disagreed with the mobile Collections screen
/// that it is the same screen as.
///
/// Lives here beside [PageBody] because the pair is the whole horizontal
/// story: [PageBody] decides how wide the column is, this decides where its
/// content starts.
const double kScreenGutter = 22;

/// Constrains a screen's content to a comfortable column and centres it.
///
/// Every screen here comes from a phone mockup, and a phone layout stretched
/// across a desktop window is not the same design — full-width action buttons
/// become metre-long bars, single-line rows put their label and value at
/// opposite ends of the screen, and text lines grow far past a readable
/// measure. Constraining to [maxWidth] keeps the intended proportions:
/// identical to today on any phone (where the window is narrower than the
/// cap), a centred column on desktop.
///
/// Every caller used to get exactly [maxWidth] forever, on any window —
/// which is where "all the windows are a different size" came from: one
/// screen (Settings) had grown its own one-off `wide` check to widen past
/// that, and the rest just stayed at a phone's measure no matter how much
/// desktop window there was to use. This is the one place that decision
/// belongs, so every screen that calls [PageBody] gets the same answer:
/// [maxWidth] below [WindowSize.spaciousBreakpoint], [wideMaxWidth] at or
/// above it. A screen with something genuinely different to do with the
/// extra room (Settings splits into two columns) passes its own
/// [wideMaxWidth]; every other screen gets a sensible default for free.
///
/// Wrap the *content*, not the [Scaffold] — the background should still fill
/// the window.
class PageBody extends StatelessWidget {
  const PageBody({
    super.key,
    required this.child,
    this.maxWidth = 560,
    this.wideMaxWidth = defaultWideMaxWidth,
  });

  /// The cap every reading-width screen shares once the window is spacious.
  /// Named so [AppScreen] hands out the same number rather than a second
  /// literal that could drift from this one.
  static const double defaultWideMaxWidth = 720;

  final Widget child;

  /// Roughly a large phone's width plus breathing room. Wide enough that
  /// nothing reflows versus the mockup, narrow enough to stay a readable
  /// measure on a narrow window.
  final double maxWidth;

  /// The cap once the window is spacious. Comfortably wider than [maxWidth]
  /// without going metre-long — see the class doc for why every screen gets
  /// this rather than opting in one at a time.
  final double wideMaxWidth;

  @override
  Widget build(BuildContext context) {
    return WindowBuilder(
      builder: (context, window) => Align(
        // Top, not centre: a short page (settings, an empty collection)
        // should start at the top of the window like every other desktop
        // app, not float in the middle of it.
        alignment: Alignment.topCenter,
        child: ConstrainedBox(
          constraints: BoxConstraints(
            maxWidth: window.isSpacious ? wideMaxWidth : maxWidth,
          ),
          child: child,
        ),
      ),
    );
  }
}

/// Card surface used for rows and panels throughout.
class SurfaceCard extends StatelessWidget {
  SurfaceCard({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(14),
    this.radius = AppRadius.card,
    this.onTap,
    this.borderColor,
    this.gradient,
    this.glow = GlowLevel.none,
    Color? glowColor,
    this.glowIntensity = 0,
  }) : glowColor = glowColor ?? AppColors.signal;

  final Widget child;
  final EdgeInsets padding;
  final double radius;
  final VoidCallback? onTap;
  final Color? borderColor;

  /// Overrides the wash [glow] would otherwise supply. Rarely needed — a
  /// card's fill follows its energy by default, which is what keeps the two
  /// from drifting apart.
  final Gradient? gradient;

  /// Energy, not decoration — see [GlowLevel]. A card that is merely selected
  /// must stay [GlowLevel.none].
  final GlowLevel glow;
  final Color glowColor;

  /// Live throughput behind this card, 0..1 — see [Glow.intensityForRate].
  /// Brightens the wash as bytes actually move.
  final double glowIntensity;

  @override
  Widget build(BuildContext context) {
    final energy =
        Glow.of(glow, color: glowColor, intensity: glowIntensity);
    final fill = gradient ?? energy.gradient;
    final content = Container(
      padding: padding,
      decoration: BoxDecoration(
        color: fill == null ? AppColors.surface : null,
        gradient: fill,
        borderRadius: BorderRadius.circular(radius),
        border: borderColor != null
            ? Border.all(color: borderColor!)
            : energy.border,
        boxShadow: energy.shadows,
      ),
      child: child,
    );
    if (onTap == null) return content;
    return Material(
      color: Colors.transparent,
      child: InkWell(
        borderRadius: BorderRadius.circular(radius),
        onTap: onTap,
        child: content,
      ),
    );
  }
}

/// Small "SECTION HEADER" style label.
class SectionLabel extends StatelessWidget {
  const SectionLabel(this.text, {super.key});

  final String text;

  @override
  Widget build(BuildContext context) {
    return Text(text, style: monoLabel(size: 10, weight: FontWeight.w500));
  }
}

/// A small status pill. The colour is the caller's decision precisely because
/// it is meaningful — mint only when something is moving, ember only for
/// torrents (see [AppColors.signal]).
class StatusBadge extends StatelessWidget {
  const StatusBadge({
    super.key,
    required this.label,
    this.color,
    this.filled = true,
  });

  final String label;

  /// `null` renders the neutral outlined variant, for states that are simply
  /// true rather than active — "SHARING", "PENDING".
  final Color? color;
  final bool filled;

  @override
  Widget build(BuildContext context) {
    final c = color;
    return Container(
      padding: const EdgeInsets.symmetric(horizontal: 9, vertical: 5),
      decoration: BoxDecoration(
        color: c != null && filled ? c.withValues(alpha: 0.12) : null,
        border: c == null ? Border.all(color: AppColors.borderStrong) : null,
        borderRadius: BorderRadius.circular(AppRadius.tight),
      ),
      child: Text(
        label,
        style: monoLabel(
            size: 11, color: c ?? AppColors.textDim, letterSpacing: 0.2),
      ),
    );
  }
}

/// A canvas heading. Uppercases here rather than at every call site, so the
/// casing is a property of the style and not something each screen has to
/// remember — and so a single edit takes it back if it ever stops earning it.
class CanvasTitle extends StatelessWidget {
  CanvasTitle(
    this.text, {
    super.key,
    this.size = 30,
    Color? color,
    this.height,
    this.textAlign,
    this.maxLines,
  }) : color = color ?? AppColors.text;

  final String text;
  final double size;
  final Color color;
  final double? height;
  final TextAlign? textAlign;
  final int? maxLines;

  @override
  Widget build(BuildContext context) {
    return Text(
      text.toUpperCase(),
      textAlign: textAlign,
      maxLines: maxLines,
      overflow: maxLines == null ? null : TextOverflow.ellipsis,
      style: canvasTitle(size: size, color: color, height: height),
    );
  }
}

/// A poster-scale heading with a soft glow and an accent bar beneath it —
/// see [impactTitle]. Reserved for [CanvasTitle]'s bigger sibling: a pane
/// that's a destination in its own right rather than one of several peers.
class ImpactTitle extends StatelessWidget {
  ImpactTitle(
    this.text, {
    super.key,
    this.size = 46,
    Color? accent,
  }) : accent = accent ?? AppColors.signal;

  final String text;
  final double size;
  final Color accent;

  @override
  Widget build(BuildContext context) {
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      mainAxisSize: MainAxisSize.min,
      children: [
        Text(text.toUpperCase(), style: impactTitle(size: size, glow: accent)),
        const SizedBox(height: 10),
        Container(
          width: 44,
          height: 5,
          decoration: BoxDecoration(
            color: accent,
            borderRadius: BorderRadius.circular(AppRadius.pill),
          ),
        ),
      ],
    );
  }
}
