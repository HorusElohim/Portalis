import 'package:flutter/material.dart';
import '../theme.dart';

/// Circular avatar with initials, matching the accent-800/600 avatar style.
class Avatar extends StatelessWidget {
  const Avatar({super.key, required this.initials, this.size = 30});

  final String initials;
  final double size;

  @override
  Widget build(BuildContext context) {
    return Container(
      width: size,
      height: size,
      alignment: Alignment.center,
      decoration: BoxDecoration(
        color: AppColors.accent800,
        shape: BoxShape.circle,
        border: Border.all(color: AppColors.accent600),
      ),
      child: Text(
        initials,
        style: TextStyle(
          color: AppColors.accent300,
          fontSize: size * 0.4,
          fontWeight: FontWeight.w500,
        ),
      ),
    );
  }
}

/// Pulsing "live copies" indicator dot, used next to a copies label.
class LiveDot extends StatefulWidget {
  const LiveDot({super.key, required this.color, this.size = 8});

  final Color color;
  final double size;

  @override
  State<LiveDot> createState() => _LiveDotState();
}

class _LiveDotState extends State<LiveDot>
    with SingleTickerProviderStateMixin {
  late final AnimationController _controller = AnimationController(
    vsync: this,
    duration: const Duration(seconds: 2),
  )..repeat();

  @override
  void dispose() {
    _controller.dispose();
    super.dispose();
  }

  @override
  Widget build(BuildContext context) {
    return SizedBox(
      width: widget.size,
      height: widget.size,
      child: AnimatedBuilder(
        animation: _controller,
        builder: (context, _) {
          final t = _controller.value;
          return Stack(
            clipBehavior: Clip.none,
            alignment: Alignment.center,
            children: [
              Opacity(
                opacity: (0.55 * (1 - t)).clamp(0.0, 1.0),
                child: Transform.scale(
                  scale: 1 + t * 1.1,
                  child: _dot(widget.color),
                ),
              ),
              _dot(widget.color),
            ],
          );
        },
      ),
    );
  }

  Widget _dot(Color color) => Container(
        decoration: BoxDecoration(color: color, shape: BoxShape.circle),
      );
}

/// Live copies indicator: pulsing dot + colored label.
class CopiesIndicator extends StatelessWidget {
  const CopiesIndicator({
    super.key,
    required this.color,
    required this.label,
    this.fontSize = 11,
  });

  final Color color;
  final String label;
  final double fontSize;

  @override
  Widget build(BuildContext context) {
    return Row(
      children: [
        LiveDot(color: color),
        const SizedBox(width: 7),
        Flexible(
          child: Text(
            label,
            overflow: TextOverflow.ellipsis,
            style: TextStyle(
              color: color,
              fontSize: fontSize,
              fontWeight: FontWeight.w500,
            ),
          ),
        ),
      ],
    );
  }
}

/// Outlined accent pill button, e.g. "＋ Share something".
class PillButton extends StatelessWidget {
  const PillButton({
    super.key,
    required this.label,
    required this.onTap,
    this.icon,
    this.filled = false,
    this.dim = false,
  });

  final String label;
  final VoidCallback? onTap;
  final Widget? icon;
  final bool filled;

  /// Use the dimmer neutral outline instead of the accent outline.
  final bool dim;

  @override
  Widget build(BuildContext context) {
    final color = dim ? AppColors.neutral300 : AppColors.accent300;
    final borderColor = dim ? AppColors.borderStrong : AppColors.accent;
    return Material(
      color: filled ? AppColors.accent : Colors.transparent,
      shape: StadiumBorder(
        side: BorderSide(color: filled ? AppColors.accent : borderColor),
      ),
      child: InkWell(
        customBorder: const StadiumBorder(),
        onTap: onTap,
        child: Padding(
          padding: const EdgeInsets.symmetric(horizontal: 28, vertical: 12),
          child: Row(
            mainAxisSize: MainAxisSize.min,
            children: [
              if (icon != null) ...[icon!, const SizedBox(width: 7)],
              Flexible(
                child: Text(
                  label,
                  overflow: TextOverflow.ellipsis,
                  style: TextStyle(
                    color: filled ? AppColors.bg : color,
                    fontSize: 14,
                    fontWeight: FontWeight.w500,
                  ),
                ),
              ),
            ],
          ),
        ),
      ),
    );
  }
}

/// A thin horizontal strip of colored segments (piece heatmap / availability).
class PieceStrip extends StatelessWidget {
  const PieceStrip({super.key, required this.colors, this.height = 14});

  final List<Color> colors;
  final double height;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(3),
      child: SizedBox(
        height: height,
        child: Row(
          children: [
            for (final c in colors)
              Expanded(
                child: Container(
                  margin: const EdgeInsets.symmetric(horizontal: 0.75),
                  color: c,
                ),
              ),
          ],
        ),
      ),
    );
  }
}

/// Placeholder tile standing in for real thumbnails/covers/media.
class PlaceholderTile extends StatelessWidget {
  const PlaceholderTile({
    super.key,
    this.label,
    this.borderRadius = 0,
  });

  final String? label;
  final double borderRadius;

  @override
  Widget build(BuildContext context) {
    return ClipRRect(
      borderRadius: BorderRadius.circular(borderRadius),
      child: CustomPaint(
        painter: _DiagonalStripePainter(),
        child: Align(
          alignment: Alignment.center,
          child: label == null
              ? null
              : Text(
                  label!,
                  style: const TextStyle(
                    color: AppColors.neutral500,
                    fontSize: 10,
                    fontFamily: 'monospace',
                  ),
                ),
        ),
      ),
    );
  }
}

class _DiagonalStripePainter extends CustomPainter {
  @override
  void paint(Canvas canvas, Size size) {
    final bg = Paint()..color = const Color(0xFF1E2130);
    canvas.drawRect(Offset.zero & size, bg);
    final stripe = Paint()..color = const Color(0xFF232637);
    const gap = 16.0;
    for (double x = -size.height; x < size.width; x += gap) {
      final path = Path()
        ..moveTo(x, size.height)
        ..lineTo(x + size.height, 0)
        ..lineTo(x + size.height + 8, 0)
        ..lineTo(x + 8, size.height)
        ..close();
      canvas.drawPath(path, stripe);
    }
  }

  @override
  bool shouldRepaint(covariant CustomPainter oldDelegate) => false;
}

enum RootTab { collections, user, settings }

/// Bottom tab bar shared by Home / User / Settings.
class RootTabBar extends StatelessWidget {
  const RootTabBar({super.key, required this.current, required this.onSelect});

  final RootTab current;
  final ValueChanged<RootTab> onSelect;

  @override
  Widget build(BuildContext context) {
    return DecoratedBox(
      decoration: const BoxDecoration(
        border: Border(top: BorderSide(color: AppColors.border)),
      ),
      child: SafeArea(
        top: false,
        child: Padding(
          padding: const EdgeInsets.symmetric(vertical: 9),
          child: Row(
            children: [
              _tab(RootTab.collections, Icons.grid_view_rounded, 'Collections'),
              _tab(RootTab.user, Icons.person_rounded, 'User'),
              _tab(RootTab.settings, Icons.settings_rounded, 'Settings'),
            ],
          ),
        ),
      ),
    );
  }

  Widget _tab(RootTab tab, IconData icon, String label) {
    final active = tab == current;
    final color = active ? AppColors.accent300 : AppColors.neutral400;
    return Expanded(
      child: InkWell(
        onTap: () => onSelect(tab),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          children: [
            Icon(icon, size: 18, color: color),
            const SizedBox(height: 3),
            Text(
              label,
              style: TextStyle(
                color: color,
                fontSize: 9.5,
                fontWeight: FontWeight.w500,
              ),
            ),
          ],
        ),
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
    return Text(
      text,
      style: const TextStyle(
        color: AppColors.neutral400,
        fontSize: 9.5,
        fontFamily: 'monospace',
        fontWeight: FontWeight.w500,
        letterSpacing: 1.2,
      ),
    );
  }
}

/// A back-chevron text button, e.g. "‹ Back".
class NavBackButton extends StatelessWidget {
  const NavBackButton({super.key, this.onTap});

  final VoidCallback? onTap;

  @override
  Widget build(BuildContext context) {
    return TextButton(
      onPressed: onTap ?? () => Navigator.of(context).maybePop(),
      style: TextButton.styleFrom(
        foregroundColor: AppColors.accent300,
        padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 8),
      ),
      child: const Text(
        '‹ Back',
        style: TextStyle(fontSize: 14, fontWeight: FontWeight.w500),
      ),
    );
  }
}
