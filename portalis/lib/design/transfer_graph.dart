import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:portalis/design/formatters.dart';
import 'package:portalis/theme.dart';

/// A compact live activity graph for the current download and upload rates.
///
/// The backend currently exposes the latest rates, not a sampled history. The
/// graph therefore communicates direction and activity without pretending it
/// has historical measurements that it does not have.
class TransferGraph extends StatelessWidget {
  const TransferGraph({
    super.key,
    required this.downloadMbps,
    required this.uploadMbps,
    this.color = AppColors.signal,
  });

  final double downloadMbps;
  final double uploadMbps;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final moving = downloadMbps > 0 || uploadMbps > 0;
    if (!moving) return const SizedBox.shrink();

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text('LIVE ACTIVITY', style: monoLabel(size: 9, color: AppColors.textDim)),
            const Spacer(),
            _Legend(color: color, label: '↓ ${formatRate(downloadMbps)}'),
            if (uploadMbps > 0) ...[
              const SizedBox(width: 12),
              _Legend(color: AppColors.signalSoft, label: '↑ ${formatRate(uploadMbps)}'),
            ],
          ],
        ),
        const SizedBox(height: 8),
        SizedBox(
          height: 54,
          width: double.infinity,
          child: CustomPaint(
            painter: _TransferGraphPainter(
              download: downloadMbps,
              upload: uploadMbps,
              color: color,
            ),
          ),
        ),
      ],
    );
  }
}

class _Legend extends StatelessWidget {
  const _Legend({required this.color, required this.label});

  final Color color;
  final String label;

  @override
  Widget build(BuildContext context) => Text(
        label,
        style: monoLabel(size: 10, color: color, weight: FontWeight.w700, letterSpacing: 0),
      );
}

class _TransferGraphPainter extends CustomPainter {
  const _TransferGraphPainter({
    required this.download,
    required this.upload,
    required this.color,
  });

  final double download;
  final double upload;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final baseline = Paint()
      ..color = AppColors.borderStrong
      ..strokeWidth = 1;
    canvas.drawLine(Offset(0, size.height - 1), Offset(size.width, size.height - 1), baseline);

    final maxRate = math.max(download, upload).toDouble();
    _drawWave(canvas, size, download / maxRate, color, 0.0);
    if (upload > 0) {
      _drawWave(canvas, size, upload / maxRate, AppColors.signalSoft, math.pi / 2);
    }
  }

  void _drawWave(Canvas canvas, Size size, double level, Color color, double phase) {
    final path = Path();
    for (var index = 0; index <= 24; index++) {
      final x = size.width * index / 24;
      final wave = math.sin(index * 0.75 + phase) * 0.14;
      final amplitude = 8.0 + level.clamp(0.0, 1.0).toDouble() * 20.0;
      final y = size.height * 0.55 - wave * amplitude - amplitude;
      if (index == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }
    final paint = Paint()
      ..color = color.withValues(alpha: 0.92)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.4
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(path, paint);
  }

  @override
  bool shouldRepaint(_TransferGraphPainter oldDelegate) =>
      oldDelegate.download != download ||
      oldDelegate.upload != upload ||
      oldDelegate.color != color;
}
