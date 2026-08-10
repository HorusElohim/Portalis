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
  TransferGraph({
    super.key,
    required this.progress,
    required this.downloadMbps,
    required this.uploadMbps,
    this.history = const [],
    this.startedAt,
    this.completedAt,
    this.showHeader = true,
    Color? color,
  }) : color = color ?? AppColors.signal;

  final double progress;
  final double downloadMbps;
  final double uploadMbps;
  final List<TransferPoint> history;
  final DateTime? startedAt;
  final DateTime? completedAt;
  final bool showHeader;
  final Color color;

  @override
  Widget build(BuildContext context) {
    final graph = _TransferGraphState.from(
      progress: progress,
      downloadMbps: downloadMbps,
      uploadMbps: uploadMbps,
      history: history,
      startedAt: startedAt,
      completedAt: completedAt,
    );

    return Column(
      crossAxisAlignment: CrossAxisAlignment.start,
      children: [
        if (showHeader) ...[
          _TransferGraphHeading(graph: graph, color: color),
          const SizedBox(height: 10),
        ],
        if (graph.maxRate <= 0)
          Padding(
            padding: const EdgeInsets.symmetric(vertical: 12),
            child: Row(
              children: [
                Container(width: 26, height: 2, color: color),
                const SizedBox(width: 12),
                Expanded(
                  child: Text(
                    graph.complete
                        ? 'No non-zero download-session samples were recorded.'
                        : 'Waiting for the first non-zero speed sample.',
                    style: monoLabel(size: 10, color: AppColors.textDim),
                  ),
                ),
              ],
            ),
          )
        else
          Semantics(
            label: 'Transfer speed on a logarithmic scale from '
                '${_dateTimeLabel(graph.start)} to '
                '${_dateTimeLabel(graph.end)}. Peak download '
                '${formatRate(graph.peakDownload)}${graph.hasUpload ? ', peak upload ${formatRate(graph.peakUpload)}' : ''}.',
            child: SizedBox(
              height: 86,
              width: double.infinity,
              child: Row(
                crossAxisAlignment: CrossAxisAlignment.stretch,
                children: [
                  SizedBox(
                    width: 66,
                    child: _RateAxis(
                      maxRate: graph.maxRate,
                      minPositiveRate: graph.minPositiveRate,
                    ),
                  ),
                  const SizedBox(width: 8),
                  Expanded(
                    child: CustomPaint(
                      painter: _TransferGraphPainter(
                        history: graph.points,
                        startedAt: graph.start,
                        endedAt: graph.end,
                        maxRate: graph.maxRate,
                        minPositiveRate: graph.minPositiveRate,
                        color: color,
                      ),
                    ),
                  ),
                ],
              ),
            ),
          ),
        const SizedBox(height: 7),
        LayoutBuilder(
          builder: (context, constraints) {
            final compact = constraints.maxWidth < 520;
            return Row(
              crossAxisAlignment: CrossAxisAlignment.start,
              children: [
                Expanded(
                  child: _TimelineLabel(
                    title: 'START',
                    value: graph.start,
                    compact: compact,
                  ),
                ),
                Expanded(
                  child: _TimelineLabel(
                    title: graph.complete ? 'END' : 'LATEST',
                    value: graph.end,
                    compact: compact,
                    alignEnd: true,
                  ),
                ),
              ],
            );
          },
        ),
      ],
    );
  }

  String _dateTimeLabel(DateTime value) =>
      '${value.day.toString().padLeft(2, '0')}/'
      '${value.month.toString().padLeft(2, '0')}/'
      '${value.year} '
      '${value.hour.toString().padLeft(2, '0')}:'
      '${value.minute.toString().padLeft(2, '0')}:'
      '${value.second.toString().padLeft(2, '0')}';
}

/// The live speed facts that can sit beside a collection identity while the
/// chart itself remains below. [TransferGraph] shows the same heading by
/// default when it is used on its own.
class TransferGraphHeader extends StatelessWidget {
  TransferGraphHeader({
    super.key,
    required this.progress,
    required this.downloadMbps,
    required this.uploadMbps,
    this.history = const [],
    this.startedAt,
    this.completedAt,
    Color? color,
  }) : color = color ?? AppColors.signal;

  final double progress;
  final double downloadMbps;
  final double uploadMbps;
  final List<TransferPoint> history;
  final DateTime? startedAt;
  final DateTime? completedAt;
  final Color color;

  @override
  Widget build(BuildContext context) => _TransferGraphHeading(
        graph: _TransferGraphState.from(
          progress: progress,
          downloadMbps: downloadMbps,
          uploadMbps: uploadMbps,
          history: history,
          startedAt: startedAt,
          completedAt: completedAt,
        ),
        color: color,
      );
}

class _TransferGraphHeading extends StatelessWidget {
  const _TransferGraphHeading({required this.graph, required this.color});

  final _TransferGraphState graph;
  final Color color;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment: CrossAxisAlignment.start,
        children: [
          Row(
            children: [
              Text(
                graph.complete ? 'DOWNLOAD SESSION' : 'TRANSFER SPEED',
                style: monoLabel(
                  size: 9,
                  color: AppColors.textDim,
                  weight: FontWeight.w700,
                ),
              ),
              const Spacer(),
              Text(
                graph.complete
                    ? 'COMPLETED IN ${graph.spanLabel}'
                    : 'LIVE · ${graph.spanLabel}',
                style: monoLabel(size: 9, color: AppColors.textGhost),
              ),
            ],
          ),
          const SizedBox(height: 10),
          Wrap(
            spacing: 8,
            runSpacing: 8,
            children: [
              _SeriesSummary(
                color: color,
                label: 'DOWNLOAD',
                value: graph.complete
                    ? 'peak ${formatRate(graph.peakDownload)}'
                    : 'now ${formatRate(graph.downloadMbps)} · peak ${formatRate(graph.peakDownload)}',
              ),
              if (graph.hasUpload)
                _SeriesSummary(
                  color: AppColors.signalSoft,
                  label: 'UPLOAD',
                  value: graph.uploadMbps > 0
                      ? 'now ${formatRate(graph.uploadMbps)} · peak ${formatRate(graph.peakUpload)}'
                      : 'peak ${formatRate(graph.peakUpload)}',
                ),
            ],
          ),
        ],
      );
}

class _TransferGraphState {
  const _TransferGraphState({
    required this.complete,
    required this.downloadMbps,
    required this.uploadMbps,
    required this.points,
    required this.start,
    required this.end,
    required this.peakDownload,
    required this.peakUpload,
    required this.minPositiveRate,
  });

  factory _TransferGraphState.from({
    required double progress,
    required double downloadMbps,
    required double uploadMbps,
    required List<TransferPoint> history,
    DateTime? startedAt,
    DateTime? completedAt,
  }) {
    final now = DateTime.now();
    // A completed download remains completed even when it predates persisted
    // transfer metadata. In that case the final sample is the best truthful
    // end time available; treating it as "latest" suggests a live transfer.
    final complete = progress >= 1.0;
    final current = TransferPoint(
      at: now,
      downloadMbps: downloadMbps,
      uploadMbps: uploadMbps,
    );
    final points = history.isEmpty
        ? [current]
        : complete
            ? history
            : [...history, current];
    final start = startedAt ?? points.first.at;
    final lastSampleAt = points.last.at;
    final requestedEnd = completedAt ?? lastSampleAt;
    final end =
        requestedEnd.isBefore(lastSampleAt) ? lastSampleAt : requestedEnd;
    final peakDownload = points.fold<double>(
      0,
      (peak, point) => math.max(peak, point.downloadMbps),
    );
    final peakUpload = points.fold<double>(
      0,
      (peak, point) => math.max(peak, point.uploadMbps),
    );
    final positiveRates = [
      for (final point in points) ...[
        if (point.downloadMbps > 0) point.downloadMbps,
        if (point.uploadMbps > 0) point.uploadMbps,
      ],
    ];
    final minPositiveRate =
        positiveRates.isEmpty ? 0.0 : positiveRates.reduce(math.min).toDouble();
    return _TransferGraphState(
      complete: complete,
      downloadMbps: downloadMbps,
      uploadMbps: uploadMbps,
      points: points,
      start: start,
      end: end,
      peakDownload: peakDownload,
      peakUpload: peakUpload,
      minPositiveRate: minPositiveRate,
    );
  }

  final bool complete;
  final double downloadMbps;
  final double uploadMbps;
  final List<TransferPoint> points;
  final DateTime start;
  final DateTime end;
  final double peakDownload;
  final double peakUpload;
  final double minPositiveRate;

  double get maxRate => math.max(peakDownload, peakUpload).toDouble();
  bool get hasUpload => peakUpload > 0 || uploadMbps > 0;
  String get spanLabel => _durationLabel(end.difference(start));
}

String _durationLabel(Duration duration) {
  final seconds = math.max(0, duration.inSeconds);
  final hours = seconds ~/ 3600;
  final minutes = (seconds % 3600) ~/ 60;
  if (hours > 0) return '${hours}h ${minutes}m';
  if (minutes > 0) return '${minutes}m ${seconds % 60}s';
  return '${seconds}s';
}

class _TimelineLabel extends StatelessWidget {
  const _TimelineLabel({
    required this.title,
    required this.value,
    this.compact = false,
    this.alignEnd = false,
  });

  final String title;
  final DateTime value;
  final bool compact;
  final bool alignEnd;

  @override
  Widget build(BuildContext context) => Column(
        crossAxisAlignment:
            alignEnd ? CrossAxisAlignment.end : CrossAxisAlignment.start,
        children: [
          Text(title, style: monoLabel(size: 8, color: AppColors.textGhost)),
          const SizedBox(height: 2),
          Text(
            compact ? _compactDateTimeLabel(value) : _dateTimeLabel(value),
            maxLines: 1,
            overflow: TextOverflow.ellipsis,
            textAlign: alignEnd ? TextAlign.end : TextAlign.start,
            style: monoLabel(size: 9, color: AppColors.textDim),
          ),
        ],
      );

  String _compactDateTimeLabel(DateTime value) =>
      '${value.day.toString().padLeft(2, '0')}/'
      '${value.month.toString().padLeft(2, '0')}  '
      '${value.hour.toString().padLeft(2, '0')}:'
      '${value.minute.toString().padLeft(2, '0')}:'
      '${value.second.toString().padLeft(2, '0')}';

  String _dateTimeLabel(DateTime value) =>
      '${value.day.toString().padLeft(2, '0')}/'
      '${value.month.toString().padLeft(2, '0')}/'
      '${value.year}  '
      '${value.hour.toString().padLeft(2, '0')}:'
      '${value.minute.toString().padLeft(2, '0')}:'
      '${value.second.toString().padLeft(2, '0')}';
}

class _SeriesSummary extends StatelessWidget {
  const _SeriesSummary({
    required this.color,
    required this.label,
    required this.value,
  });

  final Color color;
  final String label;
  final String value;

  @override
  Widget build(BuildContext context) => Padding(
        padding: const EdgeInsets.symmetric(vertical: 2),
        child: Column(
          mainAxisSize: MainAxisSize.min,
          crossAxisAlignment: CrossAxisAlignment.start,
          children: [
            Row(
              mainAxisSize: MainAxisSize.min,
              children: [
                Container(
                  width: 7,
                  height: 7,
                  decoration:
                      BoxDecoration(shape: BoxShape.circle, color: color),
                ),
                const SizedBox(width: 6),
                Text(
                  label,
                  style: monoLabel(
                    size: 9,
                    color: color,
                    weight: FontWeight.w700,
                  ),
                ),
              ],
            ),
            const SizedBox(height: 3),
            Text(
              value,
              style: monoLabel(
                size: 9,
                color: AppColors.text,
                letterSpacing: 0,
              ),
            ),
          ],
        ),
      );
}

class _RateAxis extends StatelessWidget {
  const _RateAxis({
    required this.maxRate,
    required this.minPositiveRate,
  });

  final double maxRate;
  final double minPositiveRate;

  @override
  Widget build(BuildContext context) => Column(
        mainAxisAlignment: MainAxisAlignment.spaceBetween,
        crossAxisAlignment: CrossAxisAlignment.end,
        children: [
          Text(
            formatRate(maxRate),
            style: monoLabel(size: 8, color: AppColors.textDim),
          ),
          Text(
            formatRate(
              _inverseLogarithmicRate(
                0.5,
                maxRate: maxRate,
                minPositiveRate: minPositiveRate,
              ),
            ),
            style: monoLabel(size: 8, color: AppColors.textGhost),
          ),
          Text(
            '0 MB/s',
            style: monoLabel(size: 8, color: AppColors.textGhost),
          ),
        ],
      );
}

class _TransferGraphPainter extends CustomPainter {
  const _TransferGraphPainter({
    required this.history,
    required this.startedAt,
    required this.endedAt,
    required this.maxRate,
    required this.minPositiveRate,
    required this.color,
  });

  final List<TransferPoint> history;
  final DateTime startedAt;
  final DateTime endedAt;
  final double maxRate;
  final double minPositiveRate;
  final Color color;

  @override
  void paint(Canvas canvas, Size size) {
    const top = 4.0;
    final bottom = size.height - 4;
    final grid = Paint()
      ..color = AppColors.border
      ..strokeWidth = 1;
    for (final y in [top, (top + bottom) / 2, bottom]) {
      canvas.drawLine(Offset(0, y), Offset(size.width, y), grid);
    }

    final duration = endedAt.difference(startedAt).inMicroseconds;
    final span = duration <= 0 ? 1 : duration;

    if (history.any((point) => point.downloadMbps > 0)) {
      _drawSeries(
        canvas,
        size,
        history,
        maxRate,
        span,
        startedAt,
        top,
        bottom,
        color,
        (point) => point.downloadMbps,
        fill: true,
      );
    }
    if (history.any((point) => point.uploadMbps > 0)) {
      _drawSeries(
        canvas,
        size,
        history,
        maxRate,
        span,
        startedAt,
        top,
        bottom,
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
    double top,
    double bottom,
    Color color,
    double Function(TransferPoint point) rate, {
    bool fill = false,
  }) {
    final path = Path();
    final offsets = <Offset>[];
    for (var index = 0; index < points.length; index++) {
      final point = points[index];
      final elapsed = point.at.difference(start).inMicroseconds;
      final x = points.length == 1
          ? size.width
          : (size.width * elapsed / span).clamp(0.0, size.width);
      final normalized = _logarithmicRate(
        rate(point),
        maxRate: scale,
        minPositiveRate: minPositiveRate,
      );
      final y = bottom - normalized * (bottom - top);
      offsets.add(Offset(x, y));
      if (index == 0) {
        path.moveTo(x, y);
      } else {
        path.lineTo(x, y);
      }
    }

    if (fill && offsets.isNotEmpty) {
      final area = Path()
        ..moveTo(offsets.first.dx, bottom)
        ..lineTo(offsets.first.dx, offsets.first.dy);
      for (final point in offsets.skip(1)) {
        area.lineTo(point.dx, point.dy);
      }
      area
        ..lineTo(offsets.last.dx, bottom)
        ..close();
      canvas.drawPath(
        area,
        Paint()
          ..shader = LinearGradient(
            begin: Alignment.topCenter,
            end: Alignment.bottomCenter,
            colors: [
              color.withValues(alpha: 0.16),
              color.withValues(alpha: 0.01),
            ],
          ).createShader(Rect.fromLTWH(0, top, size.width, bottom - top)),
      );
    }

    final paint = Paint()
      ..color = color.withValues(alpha: 0.86)
      ..style = PaintingStyle.stroke
      ..strokeWidth = 2.2
      ..strokeCap = StrokeCap.round
      ..strokeJoin = StrokeJoin.round;
    canvas.drawPath(path, paint);

    final last = points.last;
    final lastElapsed = last.at.difference(start).inMicroseconds;
    final lastX = points.length == 1
        ? size.width
        : (size.width * lastElapsed / span).clamp(0.0, size.width);
    final lastRate = _logarithmicRate(
      rate(last),
      maxRate: scale,
      minPositiveRate: minPositiveRate,
    );
    final lastY = bottom - lastRate * (bottom - top);
    canvas.drawCircle(
      Offset(lastX, lastY),
      3,
      Paint()..color = color,
    );
  }

  @override
  bool shouldRepaint(_TransferGraphPainter oldDelegate) =>
      oldDelegate.history != history ||
      oldDelegate.startedAt != startedAt ||
      oldDelegate.endedAt != endedAt ||
      oldDelegate.maxRate != maxRate ||
      oldDelegate.minPositiveRate != minPositiveRate ||
      oldDelegate.color != color;
}

double _logarithmicRate(
  double rate, {
  required double maxRate,
  required double minPositiveRate,
}) {
  if (rate <= 0 || maxRate <= 0 || minPositiveRate <= 0) return 0;
  final denominator = math.log(1 + maxRate / minPositiveRate);
  if (denominator <= 0) return 1;
  // A small positive floor keeps a genuinely active direction visible even
  // when the other direction is orders of magnitude faster.
  return (math.log(1 + rate / minPositiveRate) / denominator)
      .clamp(0.06, 1.0)
      .toDouble();
}

double _inverseLogarithmicRate(
  double normalized, {
  required double maxRate,
  required double minPositiveRate,
}) {
  if (maxRate <= 0 || minPositiveRate <= 0) return 0;
  final logarithmicMax = math.log(1 + maxRate / minPositiveRate);
  return minPositiveRate * (math.exp(logarithmicMax * normalized) - 1);
}
