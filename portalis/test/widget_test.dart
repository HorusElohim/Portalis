// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/main.dart';
import 'package:portalis/screens/collection_screen.dart';
import 'package:portalis/screens/media_viewer_screen.dart';
import 'package:portalis/screens/peer_screen.dart';
import 'package:portalis/screens/share_screens.dart';
import 'package:portalis/screens/swarm_screen.dart';
import 'package:portalis/widgets/common.dart';

void main() {
  testWidgets('Renders home screen with collections', (tester) async {
    await tester.pumpWidget(const MyApp());
    await tester.pump();

    expect(find.text('SmartShare'), findsOneWidget);
    expect(find.text('Your collections'), findsOneWidget);
    expect(find.text('＋ Share something'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Full collection -> media -> swarm -> peer flow, no overflow',
      (tester) async {
    // Matches the phone canvas size in the design exploration (iPhone-class).
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('Iceland 2024'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(CollectionScreen), findsOneWidget);
    expect(find.text('Add collab'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.descendant(
      of: find.byType(CollectionScreen),
      matching: find.text('IMG_1000'),
    ));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(MediaViewerScreen), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Details'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(SwarmScreen), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.descendant(
      of: find.byType(SwarmScreen),
      matching: find.text('Maya'),
    ));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(PeerScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Media viewer does not overflow on a wide desktop window',
      (tester) async {
    // Reproduces the layout that overflowed before the AspectRatio fix:
    // a wide macOS desktop window rather than a narrow phone canvas.
    await tester.binding.setSurfaceSize(const Size(1800, 1169));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('Iceland 2024'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    await tester.tap(find.descendant(
      of: find.byType(CollectionScreen),
      matching: find.text('IMG_1000'),
    ));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(MediaViewerScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Share flow: pick media -> invite -> back to home',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('＋ Share something'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(ShareStep1Screen), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.descendant(
      of: find.byType(ShareStep1Screen),
      matching: find.byType(PlaceholderTile),
    ).first);
    await tester.pump();
    expect(find.text('Continue · 1 selected'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Continue · 1 selected'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(ShareStep2Screen), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Start sharing'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('SmartShare'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('User and Settings tabs render without overflow',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('User'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Maya'), findsWidgets);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Settings'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.text('Settings'), findsWidgets);
    expect(tester.takeException(), isNull);
  });
}
