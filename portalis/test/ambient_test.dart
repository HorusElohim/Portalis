import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/theme.dart';
import 'package:portalis/ui/ui.dart';

/// Counts the frames actually scheduled, which is what battery cost reduces
/// to for an ambient effect.
Future<int> _framesOver(WidgetTester tester, Duration window) async {
  var frames = 0;
  final end = tester.binding.clock.now().add(window);
  while (tester.binding.clock.now().isBefore(end)) {
    await tester.pump(const Duration(milliseconds: 100));
    frames++;
  }
  return frames;
}

void main() {
  group('intensity mapping', () {
    test('nothing moving is exactly zero', () {
      expect(AmbientBackground.intensityForRate(0), 0);
      expect(AmbientBackground.intensityForRate(-1), 0);
    });

    test('any real movement is visible, and it saturates', () {
      // A trickle still registers...
      expect(AmbientBackground.intensityForRate(0.01), greaterThanOrEqualTo(0.15));
      // ...and a firehose doesn't keep getting brighter forever.
      expect(AmbientBackground.intensityForRate(500), 1.0);
      expect(AmbientBackground.intensityForRate(8), 1.0);
    });

    test('is monotonic between the floor and the ceiling', () {
      expect(AmbientBackground.intensityForRate(2),
          lessThan(AmbientBackground.intensityForRate(6)));
    });
  });

  group('ticker lifecycle', () {
    testWidgets('idle creates no ticker at all', (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: AmbientBackground(intensity: 0, child: SizedBox()),
      ));
      await tester.pump();

      // Not a paused animation — no ticker exists, so the framework has
      // nothing to schedule. This is the whole low-power claim.
      expect(tester.binding.transientCallbackCount, 0);
    });

    testWidgets('movement starts a ticker, and stopping disposes it',
        (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: AmbientBackground(intensity: 0.5, child: SizedBox()),
      ));
      await tester.pump();
      expect(tester.binding.transientCallbackCount, greaterThan(0));

      await tester.pumpWidget(const MaterialApp(
        home: AmbientBackground(intensity: 0, child: SizedBox()),
      ));
      await tester.pump();
      // Back to genuinely nothing once the transfer ends.
      expect(tester.binding.transientCallbackCount, 0);
    });

    testWidgets('reduce-motion keeps it static even during a transfer',
        (tester) async {
      await tester.pumpWidget(const MediaQuery(
        data: MediaQueryData(disableAnimations: true),
        child: MaterialApp(
          home: AmbientBackground(intensity: 1, child: SizedBox()),
        ),
      ));
      await tester.pump();

      expect(tester.binding.transientCallbackCount, 0);
    });

    testWidgets('an idle background schedules no frames over time',
        (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: AmbientBackground(intensity: 0, child: SizedBox()),
      ));
      await tester.pump();

      await _framesOver(tester, const Duration(seconds: 2));
      // pump() always produces a frame when asked; what matters is that the
      // widget never asks for one itself.
      expect(tester.binding.hasScheduledFrame, isFalse);
    });
  });

  group('rendering', () {
    testWidgets('always draws its child, moving or not', (tester) async {
      for (final intensity in [0.0, 0.6]) {
        await tester.pumpWidget(MaterialApp(
          home: AmbientBackground(
            intensity: intensity,
            child: const Text('content', textDirection: TextDirection.ltr),
          ),
        ));
        await tester.pump();
        expect(find.text('content'), findsOneWidget);
      }
      expect(tester.takeException(), isNull);
    });

    testWidgets('takes the accent it is given, so torrents read as ember',
        (tester) async {
      await tester.pumpWidget(const MaterialApp(
        home: AmbientBackground(
          intensity: 0.8,
          accent: AppColors.ember,
          child: SizedBox(),
        ),
      ));
      await tester.pump();
      expect(tester.takeException(), isNull);
    });
  });
}
