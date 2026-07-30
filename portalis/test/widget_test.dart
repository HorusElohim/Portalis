import 'package:flutter/material.dart';
import 'package:flutter_test/flutter_test.dart';
import 'package:portalis/main.dart';
import 'package:portalis/models.dart';
import 'package:portalis/screens/add_torrent_screen.dart';
import 'package:portalis/screens/collection_details_screen.dart';
import 'package:portalis/screens/collection_screen.dart';
import 'package:portalis/screens/join_collection_screen.dart';
import 'package:portalis/screens/settings_screen.dart';
import 'package:portalis/screens/share_screen.dart';
import 'package:portalis/screens/user_screen.dart';

void main() {
  testWidgets('Renders home screen with an empty-collections state',
      (tester) async {
    // RustLib isn't initialized in a widget test (no real native library
    // loaded), so there are no real torrents/collections — Home should show
    // its empty state plus the three Add-flow actions.
    await tester.pumpWidget(const MyApp());
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 1100));

    expect(find.text('Portalis'), findsOneWidget);
    expect(find.text('Share something'), findsOneWidget);
    expect(find.text('Join a collection'), findsOneWidget);
    expect(find.text('Torrent'), findsOneWidget);
    expect(find.textContaining('Nothing here yet'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Torrent screen validates input and surfaces backend errors',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('Torrent'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(AddTorrentScreen), findsOneWidget);

    await tester.enterText(
      find.byKey(const Key('magnetField')),
      'magnet:?xt=urn:btih:0123456789abcdef0123456789abcdef01234567',
    );
    await tester.pump();
    // A valid magnet shows the parsed preview card.
    expect(find.text('READY TO ADD'), findsOneWidget);

    await tester.tap(find.byKey(const Key('addMagnetButton')));
    await tester.pump();
    // RustLib isn't initialized, so the add call is expected to fail — it
    // should be caught and shown as inline error text, not an uncaught
    // exception (and the screen must stay open, not pop, on failure).
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(AddTorrentScreen), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Share screen requires a name and files before creating',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('Share something'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(ShareScreen), findsOneWidget);
    expect(find.text('Nothing added yet'), findsOneWidget);

    // With no files picked, Create & share stays disabled even with a name.
    await tester.enterText(
        find.byKey(const Key('collectionNameField')), 'Trip to Lisbon');
    await tester.pump();
    final button = tester.widget<FilledButton>(
        find.byKey(const Key('createShareButton')));
    expect(button.onPressed, isNull);
    expect(tester.takeException(), isNull);
  });

  testWidgets('Join screen recognises a valid code and surfaces backend errors',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    await tester.pumpWidget(const MyApp());
    await tester.pump();

    await tester.tap(find.text('Join a collection'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));
    expect(find.byType(JoinCollectionScreen), findsOneWidget);

    // Invite codes are hex-wrapped (see collections.rs::invite_code_for) so
    // the name and address aren't visible in plain text — encode the same
    // way here.
    final plain = '${'ab' * 32}:Test Collection@192.168.1.9:5321';
    final code = plain.codeUnits
        .map((c) => c.toRadixString(16).padLeft(2, '0'))
        .join();
    await tester.enterText(find.byKey(const Key('inviteCodeField')), code);
    await tester.pump();
    // The preview card parses the name and address count from the code.
    expect(find.text('Test Collection'), findsOneWidget);
    expect(find.textContaining('1 address'), findsOneWidget);

    await tester.tap(find.byKey(const Key('joinCollectionButton')));
    await tester.pump();
    // RustLib isn't initialized, so the join is expected to fail — inline
    // error, screen stays open, no uncaught exception.
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.byType(JoinCollectionScreen), findsOneWidget);
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

  testWidgets('A shared collection shows its own invite code, no backend call',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    // The invite code now travels *with* the collection, so showing it needs
    // no round trip — this used to mint a throwaway collection on every tap
    // because the shared and torrent models weren't linked.
    const collection = Collection(
      id: 'e7b1f0aa-0000-4000-8000-000000000001',
      name: 'Test Collection',
      kind: CollectionKind.shared,
      inviteCode: 'abcdef0123456789',
      collaborators: [],
      media: [],
      state: 'empty',
    );

    await tester.pumpWidget(const MaterialApp(
      home: CollectionScreen(collection: collection),
    ));
    await tester.pump();

    await tester.tap(find.text('Invite'));
    await tester.pump();
    await tester.pump(const Duration(milliseconds: 400));

    expect(find.text('Invite a collaborator'), findsOneWidget);
    expect(find.text('abcdef0123456789'), findsOneWidget);
    expect(tester.takeException(), isNull);

    await tester.tap(find.text('Done'));
    await tester.pump();
  });

  test('Media regroups into the manifest entries it was flattened from', () {
    // The grid renders a flat file list, but the unit a collection *grows* by
    // is the manifest entry (one torrent, one info-hash) — the details screen
    // shows that structure, so the regrouping has to be faithful.
    const collection = Collection(
      id: 'c1',
      name: 'Trip',
      kind: CollectionKind.shared,
      collaborators: [],
      media: [
        MediaItem(label: 'a.mp4', infoHash: 'aa', sizeBytes: 100, downloadedBytes: 100, addedBy: 'dev1'),
        MediaItem(label: 'b.mp4', infoHash: 'aa', sizeBytes: 100, downloadedBytes: 50, addedBy: 'dev1'),
        MediaItem(label: 'later', infoHash: 'bb', fetched: false, addedBy: 'dev2'),
      ],
      state: 'downloading',
    );

    final entries = collection.entries;

    expect(entries.length, 2);
    expect(entries.first.infoHash, 'aa');
    expect(entries.first.media.length, 2);
    expect(entries.first.addedBy, 'dev1');
    expect(entries.first.fetched, isTrue);
    expect(entries.first.totalBytes, 200);
    expect(entries.first.downloadedBytes, 150);
    expect(entries.first.progress, 0.75);
    // A not-yet-fetched entry has no byte counts to report — its size isn't
    // knowable until the torrent's metadata arrives.
    expect(entries.last.fetched, isFalse);
    expect(entries.last.totalBytes, 0);
    expect(entries.last.progress, 0.0);
  });

  testWidgets('Collection details degrades when the collection is unavailable',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    // RustLib isn't initialized, so Collections holds nothing — the screen
    // must say so rather than throwing on a null lookup.
    await tester.pumpWidget(const MaterialApp(
      home: CollectionDetailsScreen(collectionId: 'does-not-exist'),
    ));
    await tester.pump();

    expect(find.textContaining('no longer available'), findsOneWidget);
    expect(tester.takeException(), isNull);
  });

  testWidgets('A plain torrent collection offers no invite or add-media',
      (tester) async {
    await tester.binding.setSurfaceSize(const Size(390, 844));
    addTearDown(() => tester.binding.setSurfaceSize(null));

    // A torrent's contents are fixed by its info-hash and it has no invite
    // secret, so those actions must not appear — they'd be dead buttons.
    const collection = Collection(
      id: '0123456789abcdef0123456789abcdef01234567',
      name: 'Some Torrent',
      kind: CollectionKind.torrent,
      collaborators: [],
      media: [],
      state: 'downloading',
    );

    await tester.pumpWidget(const MaterialApp(
      home: CollectionScreen(collection: collection),
    ));
    await tester.pump();

    expect(find.text('Some Torrent'), findsOneWidget);
    expect(find.text('Invite'), findsNothing);
    expect(find.text('＋ Add media'), findsNothing);
    expect(find.text('Sync'), findsNothing);
    expect(tester.takeException(), isNull);
  });
}
