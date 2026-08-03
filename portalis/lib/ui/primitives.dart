// Part of the Portalis UI kit — see ui.dart.

import 'package:flutter/material.dart';

import '../theme.dart';

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
/// Wrap the *content*, not the [Scaffold] — the background should still fill
/// the window.
class PageBody extends StatelessWidget {
  const PageBody({super.key, required this.child, this.maxWidth = 560});

  final Widget child;

  /// Roughly a large phone's width plus breathing room. Wide enough that
  /// nothing reflows versus the mockup, narrow enough to stay a readable
  /// measure on a full-screen desktop window.
  final double maxWidth;

  @override
  Widget build(BuildContext context) {
    return Align(
      // Top, not centre: a short page (settings, an empty collection) should
      // start at the top of the window like every other desktop app, not
      // float in the middle of it.
      alignment: Alignment.topCenter,
      child: ConstrainedBox(
        constraints: BoxConstraints(maxWidth: maxWidth),
        child: child,
      ),
    );
  }
}

/// Card surface used for rows and panels throughout.
class SurfaceCard extends StatelessWidget {
  const SurfaceCard({
    super.key,
    required this.child,
    this.padding = const EdgeInsets.all(14),
    this.radius = 20,
    this.onTap,
    this.borderColor,
    this.gradient,
    this.glow = GlowLevel.none,
    this.glowColor = AppColors.signal,
  });

  final Widget child;
  final EdgeInsets padding;
  final double radius;
  final VoidCallback? onTap;
  final Color? borderColor;
  final Gradient? gradient;

  /// Energy, not decoration — see [GlowLevel]. A card that is merely selected
  /// must stay [GlowLevel.none].
  final GlowLevel glow;
  final Color glowColor;

  @override
  Widget build(BuildContext context) {
    final energy = Glow.of(glow, color: glowColor);
    final content = Container(
      padding: padding,
      decoration: BoxDecoration(
        color: gradient == null ? AppColors.surface : null,
        gradient: gradient,
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
        borderRadius: BorderRadius.circular(8),
      ),
      child: Text(
        label,
        style: monoLabel(size: 11, color: c ?? AppColors.textDim, letterSpacing: 0.2),
      ),
    );
  }
}

/// A canvas heading. Uppercases here rather than at every call site, so the
/// casing is a property of the style and not something each screen has to
/// remember — and so a single edit takes it back if it ever stops earning it.
class CanvasTitle extends StatelessWidget {
  const CanvasTitle(
    this.text, {
    super.key,
    this.size = 30,
    this.color = AppColors.text,
    this.height,
    this.textAlign,
    this.maxLines,
  });

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
  const ImpactTitle(
    this.text, {
    super.key,
    this.size = 46,
    this.accent = AppColors.signal,
  });

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
            borderRadius: BorderRadius.circular(99),
          ),
        ),
      ],
    );
  }
}
