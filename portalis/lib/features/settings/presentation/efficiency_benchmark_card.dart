import 'package:flutter/material.dart';

import '../../../design/design.dart';
import '../../../theme.dart';
import '../application/efficiency_benchmark.dart';

/// A quiet, animated proof that the local settings surface is responsive.
class EfficiencyBenchmarkCard extends StatelessWidget {
  const EfficiencyBenchmarkCard({
    super.key,
    required this.running,
    required this.result,
  });

  final bool running;
  final EfficiencyBenchmarkResult? result;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.fromLTRB(kScreenGutter, 14, kScreenGutter, 0),
        child: AnimatedSwitcher(
          duration: const Duration(milliseconds: 360),
          switchInCurve: Curves.easeOutCubic,
          switchOutCurve: Curves.easeInCubic,
          child: SurfaceCard(
            key: ValueKey((running, result?.checksum)),
            glow: result == null ? GlowLevel.none : GlowLevel.calm,
            glowColor: AppColors.signal,
            padding: const EdgeInsets.all(16),
            child: Row(
              children: [
                _BenchmarkMark(running: running),
                const SizedBox(width: 13),
                Expanded(child: _copy()),
                if (running)
                  const SizedBox(
                    width: 16,
                    height: 16,
                    child: CircularProgressIndicator(
                      strokeWidth: 1.7,
                      color: AppColors.signal,
                    ),
                  )
                else if (result != null)
                  const Icon(
                    Icons.check_circle_outline,
                    size: 21,
                    color: AppColors.signal,
                  ),
              ],
            ),
          ),
        ),
      );

  Widget _copy() {
    if (running || result == null) {
      return Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text('EFFICIENCY CHECK', style: monoLabel(size: 10, letterSpacing: 0.7)),
          const SizedBox(height: 5),
          Text(
            'Measuring local responsiveness…',
            style: AppText.secondary(color: AppColors.textDim),
          ),
        ],
      );
    }
    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Text(
          'ENGINE EFFICIENCY',
          style: monoLabel(
            size: 10,
            color: AppColors.signal,
            letterSpacing: 0.7,
          ),
        ),
        const SizedBox(height: 5),
        Text(
          '${result!.rateLabel} · ${result!.durationLabel}',
          style: AppText.cardTitle(color: AppColors.text),
        ),
        const SizedBox(height: 3),
        Text(
          'Local check complete. Refreshed whenever Settings opens.',
          style: AppText.caption(color: AppColors.textDim),
        ),
      ],
    );
  }
}

class _BenchmarkMark extends StatelessWidget {
  const _BenchmarkMark({required this.running});

  final bool running;

  @override
  Widget build(BuildContext context) => AnimatedContainer(
        duration: const Duration(milliseconds: 360),
        width: 34,
        height: 34,
        decoration: BoxDecoration(
          color: AppColors.signalWash,
          borderRadius: BorderRadius.circular(AppRadius.inner),
          border: Border.all(color: AppColors.signal.withValues(alpha: 0.45)),
        ),
        child: Icon(
          running ? Icons.bolt_outlined : Icons.speed_outlined,
          size: 19,
          color: AppColors.signal,
        ),
      );
}
