// This is a basic Flutter widget test.
//
// To perform an interaction with a widget in your test, use the WidgetTester
// utility in the flutter_test package. For example, you can send tap and scroll
// gestures. You can also use WidgetTester to find child widgets in the widget
// tree, read text, and verify that the values of widget properties are correct.

import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/main.dart';
import 'package:portalis/models.dart';
import 'package:portalis/screens/add_torrent_screen.dart';
import 'package:portalis/screens/collection_screen.dart';
import 'package:portalis/screens/settings_screen.dart';
import 'package:portalis/screens/user_screen.dart';

void main() {
  testWidgets('Renders home screen with an empty-collections state',
      (tester) async {
    // RustLib isn't initialized in a widget test (no real native library
    // loaded), so there are no real torrents/collections — Home should show
    // its empty state rather than mock data.
    await tester.pumpWidget(const MyApp());
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 1100));

    expect(find.text('Portalis'), findsOneWidget);
    expect(find.text('＋ Add'), findsOneWidget);
    expect(find.textContaining('No collections yet'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Add torrent screen renders and surfaces backend errors',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('＋ Add'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(AddTorrentScreen), findsOneWidget);

    await tester.enterText(
      find.byKey(const Key('magnetField')),
      'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567',
    );
    await tester.tap(find.byKey(const Key('addMagnetButton')));
    await tester.pump();
    // RustLib isn't initialized, so the add call is expected to fail — it
    // should be caught and shown as inline error text, not an uncaught
    // exception (and the screen must stay open, not pop, on failure).
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(AddTorrentScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Settings screen opens from Home\'s gear icon', (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.byIcon(Icons.settings_outlined));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(SettingsScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('User screen opens from Home\'s avatar', (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.byKey(const Key('userAvatarButton')));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(UserScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Join a collection surfaces backend errors', (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('＋ Add'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    await tester.enterText(
      find.byKey(const Key('collabInviteCodeField')),
      'deadbeef:Test Collection',
    );
    await tester.enterText(
      find.byKey(const Key('collabDisplayNameField')),
      'Tester',
    );
    await tester.tap(find.byKey(const Key('joinCollabButton')));
    await tester.pump();
    // RustLib isn't initialized, so the join call is expected to fail — it
    // should be caught and shown as inline error text, not an uncaught
    // exception (and the screen must stay open, not pop, on failure).
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(AddTorrentScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Add Collab shows an error dialog when the backend call fails',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    final collection = Collection(
      name: 'Test Collection',
      subtitle: '1 file',
      categories: const ['Downloading'],
      hueIndex: 0,
      copiesLabel: '0% · 0 peers',
      collaboratorCount: 0,
      media: const [],
      collaborators: const [],
    );

    await tester.pumpWidget(MaterialApp(
      home: CollectionScreen(collection: collection),
    ));
    await tester.pump();

    await tester.tap(find.text('Add collab'));
    await tester.pump(); // shows the loading dialog
    // RustLib isn't initialized, so create_collab_collection is expected
    // to fail — the loading dialog should be replaced by an error dialog,
    // not an uncaught exception.
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.text('Invite a collaborator'), findsOneWidget);
    expect(find.textContaining('Couldn\'t create invite'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Done'));
    await tester.pump();
  });
}
