import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../../../theme_palette.dart';
import '../../appearance/application/theme_controller.dart';

/// Two tappable swatches — Nature and Future — for the Settings appearance
/// section. Each shows the palette's own `signal → ember` gradient rather
/// than a description, so the choice is legible without reading either name.
class ThemePickerRow extends StatelessWidget {
  const ThemePickerRow({super.key});

  @override
  Widget build(BuildContext context) {
    return ListenableBuilder(
      listenable: ThemeController.instance,
      builder: (context, _) => Row(
        children: const [
          Expanded(
            child: _ThemeSwatch(
              id: AppThemeId.nature,
              label: 'Nature',
              palette: AppPalette.nature,
            ),
          ),
          SizedBox(width: 12),
          Expanded(
            child: _ThemeSwatch(
              id: AppThemeId.future,
              label: 'Future',
              palette: AppPalette.future,
            ),
          ),
        ],
      ),
    );
  }
}

class _ThemeSwatch extends StatelessWidget {
  const _ThemeSwatch({
    required this.id,
    required this.label,
    required this.palette,
  });

  final AppThemeId id;
  final String label;
  final AppPalette palette;

  @override
  Widget build(BuildContext context) {
    final selected = ThemeController.instance.id == id;
    return SurfaceCard(
      onTap: () => ThemeController.instance.setTheme(id),
      borderColor: selected ? palette.signal : AppColors.border,
      child: Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Container(
            height: 32,
            decoration: BoxDecoration(
              gradient: LinearGradient(
                colors: [palette.signal, palette.ember],
              ),
              borderRadius: BorderRadius.circular(AppRadius.inner),
            ),
          ),
          const SizedBox(height: 10),
          Row(
            children: [
              Expanded(child: Text(label, style: AppText.cardTitle())),
              if (selected)
                Icon(Icons.check_circle, size: 16, color: palette.signal),
            ],
          ),
        ],
      ),
    );
  }
}
