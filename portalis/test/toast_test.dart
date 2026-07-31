import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/theme.dart';
import 'package:portalis/ui/ui.dart';

/// Pumps a trivial app and hands back a context sitting under an Overlay.
Future<BuildContext> _host(WidgetTester tester) async {
  late BuildContext ctx;
  await tester.pumpWidget(MaterialApp(
    builder: (context, child) =>
        ToastScope(child: child ?? const SizedBox.shrink()),
    home: Scaffold(
      body: Builder(builder: (context) {
        ctx = context;
        return const SizedBox.expand();
      }),
    ),
  ));
  return ctx;
}

/// Dismisses a toast and drains its exit fully.
///
/// `_leave()` awaits the reverse animation, so the removal lands a microtask
/// after the controller finishes — one extra frame past the duration.
Future<void> _dismiss(WidgetTester tester, String text) async {
  await tester.tap(find.text(text));
  // Stepped rather than one long pump: the exit awaits its controller, so the
  // removal lands a microtask after the animation ends and needs real frames
  // to flush. pumpAndSettle is unusable here — the idle drift repeats
  // forever and would never settle.
  for (var i = 0; i < 20; i++) {
    await tester.pump(const Duration(milliseconds: 50));
  }
}

void main() {
  group('showing', () {
    testWidgets('a toast appears and then leaves on its own', (tester) async {
      final ctx = await _host(tester);

      showToast(ctx, 'Invite code copied');
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));
      expect(find.text('Invite code copied'), findsOneWidget);

      // It clears itself without anyone dismissing it.
      await tester.pump(const Duration(seconds: 8));
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text('Invite code copied'), findsNothing);
    });

    testWidgets('tapping dismisses early', (tester) async {
      final ctx = await _host(tester);

      showToast(ctx, 'Tap me away');
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));

      await _dismiss(tester, 'Tap me away');
      expect(find.text('Tap me away'), findsNothing);
    });

    testWidgets('a longer message is given longer to read', (tester) async {
      final ctx = await _host(tester);
      const long =
          'Couldn\'t load your collections: PanicException(failed printing '
          'to stderr: Broken pipe (os error 32))';

      showToast(ctx, long, severity: ToastSeverity.error);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));

      // A short toast would be gone by now; an error this long must not be.
      await tester.pump(const Duration(milliseconds: 3000));
      expect(find.text(long), findsOneWidget);

      await tester.pump(const Duration(seconds: 8));
      await tester.pump(const Duration(milliseconds: 400));
      expect(find.text(long), findsNothing);
    });
  });

  group('severity', () {
    testWidgets('colours the balloon by how much it matters', (tester) async {
      for (final (severity, expected, icon) in [
        (ToastSeverity.success, AppColors.signal, Icons.check_circle_outline),
        (ToastSeverity.warning, AppColors.ember, Icons.error_outline),
        (ToastSeverity.error, AppColors.danger, Icons.cancel_outlined),
        (ToastSeverity.info, AppColors.textDim, Icons.info_outline),
      ]) {
        final ctx = await _host(tester);
        showToast(ctx, 'msg', severity: severity);
        await tester.pump();
        await tester.pump(const Duration(milliseconds: 500));

        // Icon and colour come from the same switch, so checking the icon's
        // colour pins both the glyph and the tint for this severity.
        expect(find.byIcon(icon), findsOneWidget, reason: '$severity glyph');
        expect(tester.widget<Icon>(find.byIcon(icon)).color, expected,
            reason: '$severity tint');

        await _dismiss(tester, 'msg');
      }
    });
  });

  group('stacking', () {
    testWidgets('several toasts stack, oldest dropping past three',
        (tester) async {
      final ctx = await _host(tester);

      for (final m in ['first', 'second', 'third', 'fourth']) {
        showToast(ctx, m);
        await tester.pump(const Duration(milliseconds: 60));
      }
      await tester.pump(const Duration(milliseconds: 500));

      // A burst shouldn't bury the screen — the earliest drifts away.
      expect(find.text('first'), findsNothing);
      expect(find.text('second'), findsOneWidget);
      expect(find.text('third'), findsOneWidget);
      expect(find.text('fourth'), findsOneWidget);

      await tester.pump(const Duration(seconds: 8));
      await tester.pump(const Duration(milliseconds: 400));
      expect(tester.takeException(), isNull);
    });
  });

  group('motion', () {
    testWidgets('reduce-motion still shows the message, without drifting',
        (tester) async {
      late BuildContext ctx;
      await tester.pumpWidget(MediaQuery(
        data: const MediaQueryData(disableAnimations: true),
        child: MaterialApp(
          builder: (context, child) =>
              ToastScope(child: child ?? const SizedBox.shrink()),
          home: Scaffold(
            body: Builder(builder: (context) {
              ctx = context;
              return const SizedBox.expand();
            }),
          ),
        ),
      ));

      showToast(ctx, 'no drift', severity: ToastSeverity.success);
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 500));

      expect(find.text('no drift'), findsOneWidget);
      await tester.tap(find.text('no drift'));
      await tester.pump();
      await tester.pump(const Duration(milliseconds: 400));
      expect(tester.takeException(), isNull);
    });

    testWidgets('no toast means no scheduled frames', (tester) async {
      await _host(tester);
      await tester.pump(const Duration(seconds: 1));

      // The host holds no ticker until something is actually shown.
      expect(tester.binding.hasScheduledFrame, isFalse);
    });
  });

  group('robustness', () {
    testWidgets('a context with no Overlay is a no-op, not a crash',
        (tester) async {
      // Error paths call showToast; it must never be the thing that throws.
      late BuildContext ctx;
      await tester.pumpWidget(Builder(builder: (context) {
        ctx = context;
        return const Directionality(
          textDirection: TextDirection.ltr,
          child: SizedBox.expand(),
        );
      }));

      showToast(ctx, 'nowhere to go');
      await tester.pump();
      expect(tester.takeException(), isNull);
    });
  });
}
