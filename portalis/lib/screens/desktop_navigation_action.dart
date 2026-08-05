import 'package:flutter/material.dart';
import 'package:portalis/theme.dart';

/// A compact destination control for the desktop top bar.
class DesktopNavigationAction extends StatelessWidget {
  const DesktopNavigationAction({
    super.key,
    required this.icon,
    required this.tooltip,
    required this.selected,
    required this.onTap,
    this.badge,
  });

  final IconData icon;
  final String tooltip;
  final bool selected;
  final VoidCallback onTap;
  final String? badge;

  @override
  Widget build(BuildContext context) => Tooltip(
        message: tooltip,
        child: Material(
          color: selected ? AppColors.surfaceRaised : Colors.transparent,
          borderRadius: BorderRadius.circular(AppRadius.inner),
          child: InkWell(
            key: Key('header${tooltip}Button'),
            borderRadius: BorderRadius.circular(AppRadius.inner),
            onTap: onTap,
            child: Padding(
              padding: const EdgeInsets.symmetric(horizontal: 10, vertical: 9),
              child: Row(
                mainAxisSize: MainAxisSize.min,
                children: [
                  Icon(
                    icon,
                    size: 20,
                    color: selected ? AppColors.text : AppColors.textDim,
                  ),
                  if (badge != null) ...[
                    const SizedBox(width: 5),
                    Text(badge!, style: monoLabel(size: 10.5, letterSpacing: 0)),
                  ],
                ],
              ),
            ),
          ),
        ),
      );
}
