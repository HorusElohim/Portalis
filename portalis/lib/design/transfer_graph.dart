import 'dart:math' as math;

import 'package:flutter/material.dart';
import 'package:portalis/design/formatters.dart';
import 'package:portalis/theme.dart';

/// A point in the transfer history shown by [TransferGraph].
class TransferPoint {
  const TransferPoint({
    required this.at,
    required this.downloadMbps,
    required this.uploadMbps,
  });

  final DateTime at;
  final double downloadMbps;
  final double uploadMbps;
}

/// A real download/upload history sampled by the collections controller.
class TransferGraph extends StatelessWidget {
  const TransferGraph({
    super.key,
    required this.progress,
    required this.downloadMbps,
    required this.uploadMbps,
    this.history = const [],
    this.startedAt,
    this.completedAt,
    this.color = AppColors.signal,
  });

  final double progress;
  final double downloadMbps;
  final double uploadMbps;
  final List<TransferPoint> history;
  final DateTime? startedAt;
  final DateTime? completedAt;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final now = DateTime.now();
    final points = history.isEmpty
        ? [
            TransferPoint(
              at: now,
              downloadMbps: downloadMbps,
              uploadMbps: uploadMbps,
            ),
          ]
        : history;
    final start = startedAt ?? points.first.at;
    final end = completedAt ?? points.last.at;
    final complete = progress >= 1.0 && completedAt != null;

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        Row(
          children: [
            Text(
              'TRANSFER HISTORY',
              style: monoLabel(size: 9, color: AppColors.textDim),
            ),
            const Spacer(),
            _Legend(
              color: color,
              label: 'down ${formatRate(downloadMbps)}',
            ),
            if (uploadMbps > 0) ...[
              const SizedBox(width: 12),
              _Legend(
                color: AppColors.signalSoft,
                label: 'up ${formatRate(uploadMbps)}',
              ),
            ],
          ],
        ),
        const SizedBox(height: 8),
        SizedBox(
          height: 62,
          width: double.infinity,
          child: CustomPaint(
            painter: _TransferGraphPainter(
              history: points,
              startedAt: start,
              color: color,
            ),
          ),
        ),
        Row(
          mainAxisAlignment: MainAxisAlignment.spaceBetween,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            _TimelineLabel(title: 'START', value: start),
            _TimelineLabel(title: complete ? 'END' : 'CURRENT', value: end),
          ],
        ),
      ],
    );
  }
}

class _TimelineLabel extends StatelessWidget {
  const _TimelineLabel({required this.title, required this.value});

  final String title;
  final DateTime value;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Text(title, style: monoLabel(size: 8, color: AppColors.textGhost)),
          const SizedBox(height: 2),
          Text(
            _dateTimeLabel(value),
            style: monoLabel(size: 9, color: AppColors.textDim),
          ),
        ],
      );

  String _dateTimeLabel(DateTime value) =>
      '${value.day.toString().padLeft(2, '0')}/'
      '${value.month.toString().padLeft(2, '0')}/'
      '${value.year}  '
      '${value.hour.toString().padLeft(2, '0')}:'
      '${value.minute.toString().padLeft(2, '0')}:'
      '${value.second.toString().padLeft(2, '0')}';
}

class _Legend extends StatelessWidget {
  const _Legend({required this.color, required this.label});

  final Color color;
  final String label;

  @override
  Widget build(BuildContext context) => Text(
        label,
        style: monoLabel(
          size: 10,
          color: color,
          weight: FontWeight.w700,
          letterSpacing: 0,
        ),
      );
}

class _TransferGraphPainter extends CustomPainter {
  const _TransferGraphPainter({
    required this.history,
    required this.startedAt,
    required this.color,
  });

  final List<TransferPoint> history;
  final DateTime startedAt;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    final baseline = Paint()
      ..color = AppColors.borderStrong
      ..strokeWidth = 1;
    final baselineY = size.height - 8;
    canvas.drawLine(
      Offset(0, baselineY),
      Offset(size.width, baselineY),
      baseline,
    );
    canvas.drawLine(
      Offset(0, size.height * 0.5),
      Offset(size.width, size.height * 0.5),
      baseline..color = AppColors.border,
    );

    final maxRate = history.fold<double>(
      0,
      (max, point) => math.max(
        max,
        math.max(point.downloadMbps, point.uploadMbps),
      ).toDouble(),
    );
    final scale = maxRate <= 0 ? 1.0 : maxRate;
    final start = startedAt;
    final finish = history.last.at;
    final duration = finish.difference(start).inMicroseconds;
    final span = duration <= 0 ? 1 : duration;

    _drawSeries(
      canvas,
      size,
      history,
      scale,
      span,
      start,
      color,
      (point) => point.downloadMbps,
    );
    if (history.any((point) => point.uploadMbps > 0)) {
      _drawSeries(
        canvas,
        size,
        history,
        scale,
        span,
        start,
        AppColors.signalSoft,
        (point) => point.uploadMbps,
      );
    }
  }

  void _drawSeries(
    Canvas canvas,
    Size size,
    List<TransferPoint> points,
    double scale,
    int span,
    DateTime start,
    Color color,
    double Function(TransferPoint point) rate,
  ) {
    final path = Path();
    for (var index = 0; index < points.length; index++) {
      final point = points[index];
      final elapsed = point.at.difference(start).inMicroseconds;
      final x = points.length == 1 ? size.width : size.width * elapsed / span;
      final normalized = (rate(point) / scale).clamp(0.0, 1.0).toDouble();
      final y = size.height - 8 - normalized * (size.height - 18);
      if (index == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }

    final paint = Paint()
      ..color = color.withValues(alpha: 0.86)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(path, paint);

    final last = points.last;
    final lastRate = (rate(last) / scale).clamp(0.0, 1.0).toDouble();
    final lastY = size.height - 8 - lastRate * (size.height - 18);
    canvas.drawCircle(
      Offset(size.width, lastY),
      3,
      Paint()..color = color,
    );
  }

  @override
  bool shouldRepaint(_TransferGraphPainter oldDelegate) =>
      oldDelegate.history != history ||
      oldDelegate.startedAt != startedAt ||
      oldDelegate.color != color;
}
